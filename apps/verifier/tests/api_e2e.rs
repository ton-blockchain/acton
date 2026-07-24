mod support;

use axum::{
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
};
use serde::Deserialize;
use serde_json::{Value, json};
use tower::ServiceExt;
use verifier::app;
use verifier::compilers::CompileGeneratedSource;
use verifier::source_storage::SourceMapData;

use support::{
    app_state, failing_compiler_app_state, failing_source_storage_app_state, file_part, get,
    post_verify, recording_app_state, recording_source_storage_app_state,
    recording_source_storage_app_state_with_generated_sources,
    recording_source_storage_app_state_with_source_map_data, response_json, text_part,
    unverified_app_state,
};

const ADDRESS_ONE: &str = "EQD0000000000000000000000000000000000000000000000";
const ADDRESS_TWO: &str = "EQD1111111111111111111111111111111111111111111111";
const CODE_HASH_ONE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const CODE_HASH_ONE_BASE64: &str = "qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqo=";
const CODE_HASH_TWO: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const COMPILE_PARAMS_TOLK: &str = r#"{"compiler_version":"1.4.1"}"#;
const COMPILE_PARAMS_TOLK_WITH_IMPORT_MAPPINGS: &str =
    r#"{"compiler_version":"1.4.1","import_mappings":{"@contracts":"contracts"}}"#;
const COMPILE_PARAMS_FUNC: &str = r#"{"compiler_version":"0.4.6"}"#;
const EMPTY_COMPILE_PARAMS: &str = "{}";
const SOURCES_MAIN: &str = r#"[{"path":"main.tolk","is_entrypoint":true}]"#;
const SOURCES_FUNC_MAIN: &str =
    r#"[{"path":"main.fc","is_entrypoint":true,"include_in_command":true}]"#;
const SOURCES_TACT_PKG: &str = r#"[{"path":"contract.pkg","is_entrypoint":true}]"#;
const TACT_PKG_1_6_13: &str = r#"{"compiler":{"version":"1.6.13","parameters":"{\"entrypoint\":\"./contract.tact\",\"options\":{}}"}}"#;
const SOURCES_TWO_FILES: &str = r#"[
  {"path":"main.tolk","is_entrypoint":true},
  {"path":"imports/lib.tolk","is_entrypoint":false}
]"#;
const SOURCES_ALIASED_FILES: &str = r#"[
  {"path":"main.tolk","is_entrypoint":true},
  {"path":"contracts/lib.tolk","is_entrypoint":false}
]"#;

#[tokio::test]
async fn healthz_returns_ok() {
    let response = get(app_state(&[], CODE_HASH_ONE), "/healthz").await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<Value>(response).await;
    assert_eq!(body, json!({"ok": true}));
}

#[tokio::test]
async fn robots_txt_disallows_crawling() {
    let response = get(app_state(&[], CODE_HASH_ONE), "/robots.txt").await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE),
        Some(&header::HeaderValue::from_static(
            "text/plain; charset=utf-8"
        ))
    );

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("robots.txt response body should be readable");
    assert_eq!(body.as_ref(), b"User-agent: *\nDisallow: /\n");
}

#[tokio::test]
async fn version_returns_long_version() {
    let response = get(app_state(&[], CODE_HASH_ONE), "/version").await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("version response body should be readable");
    assert_eq!(body.as_ref(), env!("VERIFIER_LONG_VERSION").as_bytes());
}

#[tokio::test]
async fn openapi_json_documents_verifier_api() {
    let response = get(app_state(&[], CODE_HASH_ONE), "/api/v1/openapi.json").await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<Value>(response).await;
    assert_eq!(body["openapi"], "3.1.0");
    assert!(body["paths"]["/api/v1/verify"].is_object());
    assert!(body["paths"]["/api/v1/last_verified"].is_object());
    assert!(body["paths"]["/api/v1/abi"].is_object());
    assert!(body["paths"]["/api/v1/verification/status"].is_object());
    assert!(body["paths"]["/api/v1/verification/source"].is_object());
    assert!(body["components"]["schemas"]["VerifyResponse"].is_object());
    assert!(body["components"]["schemas"]["VerificationSourceResponse"].is_object());
    assert!(body["components"]["schemas"]["SourceFileResponse"].is_object());
}

#[tokio::test]
async fn api_routes_allow_browser_cors() {
    let state = app_state(&[], CODE_HASH_ONE);
    let preflight_request = Request::builder()
        .method(Method::OPTIONS)
        .uri(format!("/api/v1/abi?code_hash={CODE_HASH_ONE}"))
        .header(header::ORIGIN, "https://actonscan.com")
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
        .body(Body::empty())
        .expect("preflight request should be valid");

    let preflight_response = app::router_with_state(state.clone())
        .oneshot(preflight_request)
        .await
        .expect("router should handle CORS preflight");

    assert_eq!(preflight_response.status(), StatusCode::OK);
    assert_eq!(
        preflight_response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .and_then(|value| value.to_str().ok()),
        Some("*")
    );

    let get_request = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/abi?code_hash={CODE_HASH_ONE}"))
        .header(header::ORIGIN, "https://actonscan.com")
        .body(Body::empty())
        .expect("GET request should be valid");

    let get_response = app::router_with_state(state)
        .oneshot(get_request)
        .await
        .expect("router should handle browser GET request");

    assert_eq!(get_response.status(), StatusCode::OK);
    assert_eq!(
        get_response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .and_then(|value| value.to_str().ok()),
        Some("*")
    );
}

#[tokio::test]
async fn last_verified_returns_latest_verified_contracts() {
    let state = app_state(&[], CODE_HASH_ONE);
    let verify_response = post_verify(
        state.clone(),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;
    assert_eq!(verify_response.status(), StatusCode::OK);
    let verified = response_json::<VerifyResponse>(verify_response).await;
    let source_bundle_hash = verified
        .source_bundle_hash
        .as_deref()
        .expect("verify response should include source bundle hash");

    let response = get(state.clone(), "/api/v1/last_verified?limit=500&offset=0").await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<LastVerifiedResponse>(response).await;
    assert_eq!(body.total, 1);
    assert_eq!(body.items.len(), 1);
    assert_eq!(body.items[0].code_hash, CODE_HASH_ONE);
    assert_eq!(body.items[0].source_bundle_hash, source_bundle_hash);
    assert_eq!(body.items[0].entrypoint, "main.tolk");
    assert_eq!(body.items[0].compiler.language, "tolk");
    assert_eq!(body.items[0].file_count, 1);
    assert!(!body.items[0].has_tolk_abi);
    assert_eq!(body.items[0].abi_name, None);

    let response = get(state, "/api/v1/last_verified?offset=500").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json::<LastVerifiedResponse>(response).await;
    assert_eq!(body.total, 1);
    assert!(body.items.is_empty());
}

#[tokio::test]
async fn abi_returns_indexed_tolk_abi_records_with_code_hash() {
    let state = recording_source_storage_app_state_with_generated_sources(
        &[],
        CODE_HASH_ONE,
        vec![CompileGeneratedSource {
            path: "output/main.abi.json".to_owned(),
            content: r#"{"contract_name":"Smoke","abi_schema_version":"1.0"}"#.to_owned(),
        }],
    )
    .0;
    let verify_response = post_verify(
        state.clone(),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;
    assert_eq!(verify_response.status(), StatusCode::OK);
    let verified = response_json::<VerifyResponse>(verify_response).await;
    assert!(verified.source_bundle_hash.is_some());

    let response = get(state.clone(), "/api/v1/abi").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json::<AbiContractsResponse>(response).await;
    assert_eq!(body.items.len(), 1);
    assert_eq!(body.items[0].code_hash, CODE_HASH_ONE);
    assert_eq!(body.items[0].abi["contract_name"].as_str(), Some("Smoke"));

    let response = get(
        state.clone(),
        &format!("/api/v1/abi?code_hash={CODE_HASH_ONE}"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json::<AbiContractsResponse>(response).await;
    assert_eq!(body.items[0].code_hash, CODE_HASH_ONE);
    assert_eq!(body.items[0].abi["contract_name"].as_str(), Some("Smoke"));

    let response = get(
        state.clone(),
        &format!("/api/v1/abi?code_hash={CODE_HASH_ONE_BASE64}"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json::<AbiContractsResponse>(response).await;
    assert_eq!(body.items[0].code_hash, CODE_HASH_ONE);
    assert_eq!(body.items[0].abi["contract_name"].as_str(), Some("Smoke"));

    let response = get(state.clone(), "/api/v1/last_verified").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json::<LastVerifiedResponse>(response).await;
    assert_eq!(body.items[0].entrypoint, "main.tolk");
    assert_eq!(body.items[0].abi_name.as_deref(), Some("Smoke"));

    let response = get(state, "/api/v1/abi?offset=500").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json::<AbiContractsResponse>(response).await;
    assert!(body.items.is_empty());
}

#[tokio::test]
async fn verification_status_reports_unverified_code_hash_without_stored_bundle() {
    let response = get(
        app_state(&[], CODE_HASH_ONE),
        &format!("/api/v1/verification/status?code_hash={CODE_HASH_ONE}"),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<VerificationStatusResponse>(response).await;
    assert_eq!(body.code_hash, CODE_HASH_ONE);
    assert!(!body.verified);
}

#[tokio::test]
async fn verification_status_reports_verified_after_successful_verify() {
    let state = app_state(&[], CODE_HASH_ONE);
    let verify_response = post_verify(
        state.clone(),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;
    assert_eq!(verify_response.status(), StatusCode::OK);

    let response = get(
        state,
        &format!("/api/v1/verification/status?code_hash={CODE_HASH_ONE}"),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<VerificationStatusResponse>(response).await;
    assert_eq!(body.code_hash, CODE_HASH_ONE);
    assert!(body.verified);
}

#[tokio::test]
async fn verification_status_resolves_code_hash_from_address() {
    let state = app_state(&[(ADDRESS_ONE, CODE_HASH_ONE)], CODE_HASH_ONE);
    let verify_response = post_verify(
        state.clone(),
        vec![
            text_part("address", ADDRESS_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;
    assert_eq!(verify_response.status(), StatusCode::OK);

    let response = get(
        state,
        &format!("/api/v1/verification/status?address={ADDRESS_ONE}"),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<VerificationStatusResponse>(response).await;
    assert_eq!(body.code_hash, CODE_HASH_ONE);
    assert!(body.verified);
}

#[tokio::test]
async fn verification_status_reports_unverified_contract() {
    let response = get(
        unverified_app_state(&[], CODE_HASH_ONE),
        &format!("/api/v1/verification/status?code_hash={CODE_HASH_ONE}"),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<VerificationStatusResponse>(response).await;
    assert_eq!(body.code_hash, CODE_HASH_ONE);
    assert!(!body.verified);
}

#[tokio::test]
async fn verification_status_rejects_missing_target() {
    let response = get(app_state(&[], CODE_HASH_ONE), "/api/v1/verification/status").await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_error_contains(response, "address or code_hash").await;
}

#[tokio::test]
async fn verification_status_returns_not_found_when_address_has_no_code_hash() {
    let response = get(
        app_state(&[], CODE_HASH_ONE),
        &format!("/api/v1/verification/status?address={ADDRESS_ONE}"),
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_error_contains(response, "code_hash was not found").await;
}

#[tokio::test]
async fn verification_source_returns_verified_bundle_files() {
    let (state, recorded_requests) = recording_source_storage_app_state(&[], CODE_HASH_ONE);
    let source_path = format!("/api/v1/verification/source?code_hash={CODE_HASH_ONE}");
    let unverified_response = get(state.clone(), &source_path).await;
    assert_eq!(unverified_response.status(), StatusCode::OK);
    let unverified = response_json::<VerificationSourceResponse>(unverified_response).await;
    assert!(!unverified.verified);
    assert!(unverified.bundle.is_none());

    let verify_response = post_verify(
        state.clone(),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;
    assert_eq!(verify_response.status(), StatusCode::OK);

    let verified = response_json::<VerifyResponse>(verify_response).await;
    let source_bundle_hash = verified
        .source_bundle_hash
        .as_deref()
        .expect("verify response should include source bundle hash");
    let response = get(state.clone(), &source_path).await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<VerificationSourceResponse>(response).await;
    assert_eq!(body.code_hash, CODE_HASH_ONE);
    assert!(body.verified);
    let bundle = body
        .bundle
        .expect("verified source should include a bundle");
    assert_eq!(bundle.source_bundle_hash, source_bundle_hash);
    assert_eq!(bundle.verified_at, 1_700_000_000);
    assert_eq!(bundle.compiler.language, "tolk");
    assert_eq!(bundle.compiler.version, "1.4.1");
    assert_eq!(bundle.entrypoint, "main.tolk");
    assert_eq!(bundle.files.len(), 1);
    assert_eq!(bundle.files[0].path, "main.tolk");
    assert_eq!(bundle.files[0].content, "fun main() {}");

    let repeated_response = post_verify(
        state.clone(),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part(
                "files",
                "main.tolk",
                "text/plain",
                "fun main() { throw 1; }",
            ),
        ],
    )
    .await;
    assert_eq!(repeated_response.status(), StatusCode::OK);
    let repeated = response_json::<VerifyResponse>(repeated_response).await;
    let repeated_hash = repeated
        .source_bundle_hash
        .expect("repeat response should include the stored source bundle hash");
    assert_eq!(repeated.verification_result, "already_verified");
    assert_eq!(repeated.compiled_code_hash, None);
    assert_eq!(repeated_hash, source_bundle_hash);
    assert_eq!(
        recorded_requests
            .lock()
            .expect("recorded source storage requests mutex should not be poisoned")
            .len(),
        1,
        "an already-verified code hash must not be stored again"
    );

    let response = get(
        state.clone(),
        &format!("/api/v1/verification/source?code_hash={CODE_HASH_ONE}"),
    )
    .await;
    let body = response_json::<VerificationSourceResponse>(response).await;
    let bundle = body.bundle.expect("verified bundle should still exist");
    assert_eq!(bundle.source_bundle_hash, source_bundle_hash);
    assert_eq!(bundle.files.len(), 1);
    assert_eq!(bundle.files[0].content, "fun main() {}");

    let response = get(state, "/api/v1/last_verified").await;
    let body = response_json::<LastVerifiedResponse>(response).await;
    assert_eq!(body.items.len(), 1);
    assert_eq!(body.items[0].source_bundle_hash, source_bundle_hash);
}

#[tokio::test]
async fn verification_source_returns_stored_source_map_data() {
    let source_map = source_map_data_fixture();
    let (state, recorded_requests) = recording_source_storage_app_state_with_source_map_data(
        &[],
        CODE_HASH_ONE,
        source_map.clone(),
    );
    let verify_response = post_verify(
        state.clone(),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;
    assert_eq!(verify_response.status(), StatusCode::OK);

    let recorded_source_map_data = {
        let recorded_requests = recorded_requests
            .lock()
            .expect("recorded source storage requests mutex should not be poisoned");
        assert_eq!(recorded_requests.len(), 1);
        recorded_requests[0].source_map.clone()
    };
    assert_eq!(recorded_source_map_data.as_ref(), Some(&source_map));

    let response = get(
        state,
        &format!("/api/v1/verification/source?code_hash={CODE_HASH_ONE}"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<VerificationSourceResponse>(response).await;
    assert_eq!(
        body.bundle
            .as_ref()
            .and_then(|bundle| bundle.source_map.as_ref()),
        Some(&source_map)
    );
}

#[tokio::test]
async fn verification_source_returns_not_found_when_address_has_no_code_hash() {
    let response = get(
        app_state(&[], CODE_HASH_ONE),
        &format!("/api/v1/verification/source?address={ADDRESS_ONE}"),
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_error_contains(response, "code_hash was not found").await;
}

#[tokio::test]
async fn verify_resolves_code_hash_from_address_with_mock_blockchain() {
    let response = post_verify(
        app_state(&[(ADDRESS_ONE, CODE_HASH_ONE)], CODE_HASH_ONE),
        vec![
            text_part("address", ADDRESS_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<VerifyResponse>(response).await;
    assert_eq!(body.code_hash, CODE_HASH_ONE);
    assert_eq!(body.compiled_code_hash.as_deref(), Some(CODE_HASH_ONE));
    assert_eq!(body.verification_result, "match");
    assert!(body.source_bundle_hash.is_some());
    assert_eq!(body.storage_revision.as_deref(), Some("mock-revision"));
}

#[tokio::test]
async fn verify_accepts_valid_multipart_request_with_multiple_files() {
    let response = post_verify(
        app_state(&[(ADDRESS_TWO, CODE_HASH_TWO)], CODE_HASH_TWO),
        vec![
            text_part("address", ADDRESS_TWO),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_TWO_FILES),
            file_part(
                "files",
                "main.tolk",
                "text/plain",
                "import \"imports/lib.tolk\";",
            ),
            file_part("files", "imports/lib.tolk", "text/plain", "fun helper() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<VerifyResponse>(response).await;
    assert_eq!(body.code_hash, CODE_HASH_TWO);
    assert_eq!(body.compiled_code_hash.as_deref(), Some(CODE_HASH_TWO));
    assert_eq!(body.verification_result, "match");
    assert!(body.source_bundle_hash.is_some());
    assert_eq!(body.storage_revision.as_deref(), Some("mock-revision"));
}

#[tokio::test]
async fn verify_passes_uploaded_file_contents_to_compiler() {
    let (state, recorded_requests) = recording_app_state(&[], CODE_HASH_ONE);
    let response = post_verify(
        state,
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_TWO_FILES),
            file_part(
                "files",
                "main.tolk",
                "text/plain",
                "import \"imports/lib.tolk\";",
            ),
            file_part("files", "imports/lib.tolk", "text/plain", "fun helper() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let (language, compiler_version, entrypoint, sources) = {
        let recorded_requests = recorded_requests
            .lock()
            .expect("recorded compiler requests mutex should not be poisoned");
        assert_eq!(recorded_requests.len(), 1);

        let request = &recorded_requests[0];
        let snapshot = (
            request.language.clone(),
            request.compiler_version.clone(),
            request.entrypoint.clone(),
            request
                .sources
                .iter()
                .map(|source| (source.path.clone(), source.content.clone()))
                .collect::<Vec<_>>(),
        );
        drop(recorded_requests);
        snapshot
    };

    assert_eq!(language, "tolk");
    assert_eq!(compiler_version, "1.4.1");
    assert_eq!(entrypoint, "main.tolk");
    assert_eq!(sources.len(), 2);
    assert_eq!(sources[0].0, "main.tolk");
    assert_eq!(sources[0].1, "import \"imports/lib.tolk\";");
    assert_eq!(sources[1].0, "imports/lib.tolk");
    assert_eq!(sources[1].1, "fun helper() {}");
}

#[tokio::test]
async fn verify_passes_import_mappings_to_compiler() {
    let (state, recorded_requests) = recording_app_state(&[], CODE_HASH_ONE);
    let response = post_verify(
        state,
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK_WITH_IMPORT_MAPPINGS),
            text_part("sources", SOURCES_ALIASED_FILES),
            file_part(
                "files",
                "main.tolk",
                "text/plain",
                "import \"@contracts/lib\";",
            ),
            file_part(
                "files",
                "contracts/lib.tolk",
                "text/plain",
                "fun helper() {}",
            ),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let contracts_mapping = {
        let recorded_requests = recorded_requests
            .lock()
            .expect("recorded compiler requests mutex should not be poisoned");
        assert_eq!(recorded_requests.len(), 1);

        recorded_requests[0]
            .import_mappings
            .get("@contracts")
            .cloned()
    };
    assert_eq!(contracts_mapping.as_deref(), Some("contracts"));
}

#[tokio::test]
async fn verify_accepts_func_and_passes_compile_metadata_to_compiler() {
    let (state, recorded_requests) = recording_app_state(&[], CODE_HASH_ONE);
    let response = post_verify(
        state,
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "func"),
            text_part("compile_params", COMPILE_PARAMS_FUNC),
            text_part("sources", SOURCES_FUNC_MAIN),
            file_part("files", "main.fc", "text/plain", "() main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let snapshot = {
        let recorded_requests = recorded_requests
            .lock()
            .expect("recorded compiler requests mutex should not be poisoned");
        assert_eq!(recorded_requests.len(), 1);

        let request = &recorded_requests[0];
        let snapshot = (
            request.language.clone(),
            request.compiler_version.clone(),
            request.entrypoint.clone(),
            request.sources[0].path.clone(),
            request.sources[0].include_in_command,
        );
        drop(recorded_requests);
        snapshot
    };
    assert_eq!(snapshot.0, "func");
    assert_eq!(snapshot.1, "0.4.6");
    assert_eq!(snapshot.2, "main.fc");
    assert_eq!(snapshot.3, "main.fc");
    assert_eq!(snapshot.4, Some(true));
}

#[tokio::test]
async fn verify_accepts_tact_and_reads_compiler_version_from_pkg() {
    let (state, recorded_requests) = recording_app_state(&[], CODE_HASH_ONE);
    let response = post_verify(
        state,
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tact"),
            text_part("compile_params", EMPTY_COMPILE_PARAMS),
            text_part("sources", SOURCES_TACT_PKG),
            file_part("files", "contract.pkg", "application/json", TACT_PKG_1_6_13),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let snapshot = {
        let recorded_requests = recorded_requests
            .lock()
            .expect("recorded compiler requests mutex should not be poisoned");
        assert_eq!(recorded_requests.len(), 1);

        let request = &recorded_requests[0];
        let snapshot = (
            request.language.clone(),
            request.compiler_version.clone(),
            request.entrypoint.clone(),
        );
        drop(recorded_requests);
        snapshot
    };
    assert_eq!(snapshot.0, "tact");
    assert_eq!(snapshot.1, "1.6.13");
    assert_eq!(snapshot.2, "contract.tact");
}

#[tokio::test]
async fn verify_accepts_code_hash_without_address() {
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<VerifyResponse>(response).await;
    assert_eq!(body.code_hash, CODE_HASH_ONE);
    assert_eq!(body.compiled_code_hash.as_deref(), Some(CODE_HASH_ONE));
    assert_eq!(body.verification_result, "match");
}

#[tokio::test]
async fn verify_accepts_address_and_code_hash_together() {
    let response = post_verify(
        app_state(&[(ADDRESS_ONE, CODE_HASH_ONE)], CODE_HASH_ONE),
        vec![
            text_part("address", ADDRESS_ONE),
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<VerifyResponse>(response).await;
    assert_eq!(body.code_hash, CODE_HASH_ONE);
    assert_eq!(body.compiled_code_hash.as_deref(), Some(CODE_HASH_ONE));
    assert_eq!(body.verification_result, "match");
}

#[tokio::test]
async fn verify_normalizes_base64_code_hash_input() {
    let response = post_verify(
        app_state(&[(ADDRESS_ONE, CODE_HASH_ONE)], CODE_HASH_ONE),
        vec![
            text_part("address", ADDRESS_ONE),
            text_part("code_hash", CODE_HASH_ONE_BASE64),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<VerifyResponse>(response).await;
    assert_eq!(body.code_hash, CODE_HASH_ONE);
    assert_eq!(body.compiled_code_hash.as_deref(), Some(CODE_HASH_ONE));
    assert_eq!(body.verification_result, "match");
}

#[tokio::test]
async fn verify_returns_mismatch_when_compiled_hash_differs_from_target() {
    let response = post_verify(
        app_state(&[], CODE_HASH_TWO),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<VerifyResponse>(response).await;
    assert_eq!(body.code_hash, CODE_HASH_ONE);
    assert_eq!(body.compiled_code_hash.as_deref(), Some(CODE_HASH_TWO));
    assert_eq!(body.verification_result, "mismatch");
    assert_eq!(body.source_bundle_hash, None);
    assert!(body.storage_revision.is_none());
}

#[tokio::test]
async fn verify_returns_source_bundle_hash_on_hash_match() {
    let (state, recorded_requests) = recording_source_storage_app_state(&[], CODE_HASH_ONE);
    let response = post_verify(
        state,
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<VerifyResponse>(response).await;
    let source_bundle_hash = body
        .source_bundle_hash
        .as_deref()
        .expect("matched verification should return source bundle hash");
    assert_eq!(body.storage_revision.as_deref(), Some("mock-revision"));

    let recorded_snapshot = {
        let recorded_requests = recorded_requests
            .lock()
            .expect("recorded source storage requests mutex should not be poisoned");
        let snapshot = recorded_requests.clone();
        drop(recorded_requests);
        snapshot
    };
    assert_eq!(recorded_snapshot.len(), 1);
    assert_eq!(recorded_snapshot[0].code_hash, CODE_HASH_ONE);
    assert_eq!(recorded_snapshot[0].source_bundle_hash, source_bundle_hash);
    assert_eq!(recorded_snapshot[0].files.len(), 1);
    assert_eq!(recorded_snapshot[0].files[0].0, "main.tolk");
    assert_eq!(recorded_snapshot[0].files[0].1, "fun main() {}");
}

#[tokio::test]
async fn verify_stores_generated_sources_on_hash_match() {
    let (state, recorded_requests) = recording_source_storage_app_state_with_generated_sources(
        &[],
        CODE_HASH_ONE,
        vec![CompileGeneratedSource {
            path: "contract.abi".to_owned(),
            content: r#"{"name":"Contract"}"#.to_owned(),
        }],
    );
    let response = post_verify(
        state,
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tact"),
            text_part("compile_params", EMPTY_COMPILE_PARAMS),
            text_part("sources", SOURCES_TACT_PKG),
            file_part("files", "contract.pkg", "application/json", TACT_PKG_1_6_13),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let recorded_snapshot = {
        let recorded_requests = recorded_requests
            .lock()
            .expect("recorded source storage requests mutex should not be poisoned");
        let snapshot = recorded_requests.clone();
        drop(recorded_requests);
        snapshot
    };
    assert_eq!(recorded_snapshot.len(), 1);
    assert_eq!(recorded_snapshot[0].files.len(), 2);
    assert_eq!(recorded_snapshot[0].files[0].0, "contract.abi");
    assert_eq!(recorded_snapshot[0].files[0].1, r#"{"name":"Contract"}"#);
    assert_eq!(recorded_snapshot[0].files[1].0, "contract.pkg");
}

#[tokio::test]
async fn verify_stores_source_bundle_on_hash_match() {
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<VerifyResponse>(response).await;
    let source_bundle_hash = body
        .source_bundle_hash
        .as_deref()
        .expect("matched verification should return source bundle hash");
    assert_eq!(source_bundle_hash.len(), 64);
    assert!(
        source_bundle_hash
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    );
    assert_eq!(body.storage_revision.as_deref(), Some("mock-revision"));
}

#[tokio::test]
async fn verify_does_not_store_source_bundle_on_hash_mismatch() {
    let response = post_verify(
        app_state(&[], CODE_HASH_TWO),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<VerifyResponse>(response).await;
    assert_eq!(body.verification_result, "mismatch");
    assert!(body.storage_revision.is_none());
}

#[tokio::test]
async fn verify_returns_bad_gateway_when_source_storage_fails() {
    let response = post_verify(
        failing_source_storage_app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    assert_error_contains(response, "source storage failed").await;
}

#[tokio::test]
async fn verify_returns_bad_request_when_compilation_fails() {
    let response = post_verify(
        failing_compiler_app_state(&[], "Tolk syntax error at main.tolk:1:5"),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun broken("),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response_json::<Value>(response).await;
    let error = body["error"].as_str().unwrap_or_default();
    assert!(
        error.contains("Tolk syntax error at main.tolk:1:5"),
        "expected error to contain compiler details, got {body}"
    );
}

#[tokio::test]
async fn verify_rejects_address_and_code_hash_mismatch() {
    let response = post_verify(
        app_state(&[(ADDRESS_ONE, CODE_HASH_TWO)], CODE_HASH_ONE),
        vec![
            text_part("address", ADDRESS_ONE),
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_error_contains(response, "mismatch").await;
}

#[tokio::test]
async fn verify_rejects_address_without_current_code_hash() {
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("address", ADDRESS_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_error_contains(response, "not found").await;
}

#[tokio::test]
async fn verify_rejects_missing_verification_target() {
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_error_contains(response, "address or code_hash").await;
}

#[tokio::test]
async fn verify_rejects_missing_language() {
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("address", ADDRESS_ONE),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_error_contains(response, "language").await;
}

#[tokio::test]
async fn verify_rejects_missing_files() {
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("address", ADDRESS_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_error_contains(response, "files").await;
}

#[tokio::test]
async fn verify_rejects_invalid_compile_params_json() {
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("address", ADDRESS_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", "{not json"),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_error_contains(response, "compile_params").await;
}

#[tokio::test]
async fn verify_rejects_missing_sources() {
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_error_contains(response, "sources").await;
}

#[tokio::test]
async fn verify_rejects_missing_entrypoint_source() {
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", r#"[{"path":"main.tolk","is_entrypoint":false}]"#),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_error_contains(response, "entrypoint").await;
}

#[tokio::test]
async fn verify_rejects_multiple_entrypoint_sources() {
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part(
                "sources",
                r#"[
                  {"path":"main.tolk","is_entrypoint":true},
                  {"path":"other.tolk","is_entrypoint":true}
                ]"#,
            ),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
            file_part("files", "other.tolk", "text/plain", "fun other() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_error_contains(response, "multiple entrypoint").await;
}

#[tokio::test]
async fn verify_rejects_uploaded_file_without_source_metadata() {
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
            file_part("files", "extra.tolk", "text/plain", "fun extra() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_error_contains(response, "no source metadata").await;
}

#[tokio::test]
async fn verify_rejects_source_metadata_without_uploaded_file() {
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_TWO_FILES),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_error_contains(response, "no uploaded file").await;
}

#[tokio::test]
async fn verify_rejects_invalid_source_path() {
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part(
                "sources",
                r#"[{"path":"../main.tolk","is_entrypoint":true}]"#,
            ),
            file_part("files", "../main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_error_contains(response, "invalid component").await;
}

#[tokio::test]
async fn verify_rejects_backslash_source_path() {
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part(
                "sources",
                r#"[{"path":"imports\\lib.tolk","is_entrypoint":true}]"#,
            ),
            file_part("files", "imports\\lib.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_error_contains(response, "separators").await;
}

#[tokio::test]
async fn verify_rejects_unsupported_language() {
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "fift"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_error_contains(response, "unsupported language").await;
}

#[tokio::test]
async fn verify_rejects_missing_tolk_version() {
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", EMPTY_COMPILE_PARAMS),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_error_contains(response, "missing compiler version for tolk").await;
}

async fn assert_error_contains(response: axum::response::Response, expected: &str) {
    let body = response_json::<Value>(response).await;
    assert!(
        body["error"]
            .as_str()
            .unwrap_or_default()
            .contains(expected),
        "expected error to contain {expected}, got {body}"
    );
}

#[derive(Debug, Deserialize)]
struct VerifyResponse {
    code_hash: String,
    compiled_code_hash: Option<String>,
    verification_result: String,
    source_bundle_hash: Option<String>,
    storage_revision: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VerificationStatusResponse {
    code_hash: String,
    verified: bool,
}

#[derive(Debug, Deserialize)]
struct VerificationSourceResponse {
    code_hash: String,
    verified: bool,
    bundle: Option<VerifiedSourceBundle>,
}

#[derive(Debug, Deserialize)]
struct VerifiedSourceBundle {
    source_bundle_hash: String,
    verified_at: u64,
    entrypoint: String,
    compiler: VerifiedCompiler,
    source_map: Option<SourceMapData>,
    files: Vec<VerifiedSourceFile>,
}

fn source_map_data_fixture() -> SourceMapData {
    SourceMapData {
        code_boc64: "te6cckEBAQEAAgAAAEysuc0=".to_owned(),
        symbol_types_json: json!([{ "name": "main" }]),
        debug_marks_json: json!([{ "offset": 0 }]),
        debug_marks_base64: "te6cckEBAQEAAgAAAEysuc0=".to_owned(),
    }
}

#[derive(Debug, Deserialize)]
struct VerifiedCompiler {
    language: String,
    version: String,
}

#[derive(Debug, Deserialize)]
struct VerifiedSourceFile {
    path: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct LastVerifiedResponse {
    items: Vec<LastVerifiedItem>,
    total: usize,
}

#[derive(Debug, Deserialize)]
struct LastVerifiedItem {
    code_hash: String,
    source_bundle_hash: String,
    entrypoint: String,
    compiler: VerifiedCompiler,
    file_count: usize,
    has_tolk_abi: bool,
    abi_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AbiContractsResponse {
    items: Vec<AbiContract>,
}

#[derive(Debug, Deserialize)]
struct AbiContract {
    code_hash: String,
    abi: Value,
}
