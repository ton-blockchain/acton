mod support;

use std::sync::Arc;

use axum::{
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
};
use serde::Deserialize;
use serde_json::{Value, json};
use tower::ServiceExt;
use verifier::app;
use verifier::compilers::CompileGeneratedSource;
use verifier::payment::{
    OnchainPaymentVerifier, PaymentAttemptOutcome, PaymentError, PaymentLedger, PaymentVerifier,
};
use verifier::source_storage::SourceMapData;

use support::{
    PAYMENT_ADDRESS, PAYMENT_TX_HASH, StaticPaymentBlockchainClient, app_state,
    app_state_with_api_key, fail_once_source_storage_app_state, failing_compiler_app_state,
    failing_compiler_app_state_with_payment_outcomes, failing_source_storage_app_state,
    failing_source_storage_app_state_with_payment_outcomes, file_part, get,
    mapped_compiler_app_state, owned_file_part, owned_text_part, payment_error_app_state,
    payment_transaction, post_verify, post_verify_with_api_key, post_verify_without_payment,
    recording_app_state, recording_payment_app_state, recording_source_storage_app_state,
    recording_source_storage_app_state_with_generated_sources,
    recording_source_storage_app_state_with_source_map_data, recovering_payment_app_state,
    response_json, text_part, timing_out_compiler_app_state_with_payment_outcomes,
    unverified_app_state,
};

const ADDRESS_ONE: &str = "EQD0000000000000000000000000000000000000000000000";
const ADDRESS_TWO: &str = "EQD1111111111111111111111111111111111111111111111";
const CODE_HASH_ONE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const CODE_HASH_ONE_BASE64: &str = "qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqo=";
const CODE_HASH_TWO: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const CODE_HASH_THREE: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const API_KEY: &str = "migration-api-key";
const ORIGINAL_VERIFIED_AT: &str = "1678647600000";
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

async fn post_take_ticket(
    state: verifier::state::AppState,
    code_hash: &str,
) -> axum::response::Response {
    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/take-ticket")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({"code_hash": code_hash}).to_string()))
        .expect("POST /api/v1/take-ticket request should be valid");

    app::router_with_state(state)
        .oneshot(request)
        .await
        .expect("router should handle POST /api/v1/take-ticket request")
}

fn valid_verify_parts() -> Vec<support::MultipartPart> {
    vec![
        text_part("code_hash", CODE_HASH_ONE),
        text_part("language", "tolk"),
        text_part("compile_params", COMPILE_PARAMS_TOLK),
        text_part("sources", SOURCES_MAIN),
        file_part("files", "main.tolk", "text/plain", "fun main() {}"),
    ]
}

fn response_statuses(operation: &Value) -> Vec<String> {
    let mut statuses = operation["responses"]
        .as_object()
        .expect("OpenAPI operation responses should be an object")
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    statuses.sort();
    statuses
}

#[tokio::test]
async fn healthz_returns_ok() {
    let response = get(app_state(&[], CODE_HASH_ONE), "/healthz").await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<Value>(response).await;
    assert_eq!(body, json!({"ok": true}));
}

#[tokio::test]
async fn healthz_reports_payment_history_recovery() {
    let response = get(recovering_payment_app_state(CODE_HASH_ONE), "/healthz").await;

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        response_json::<Value>(response).await,
        json!({"ok": false, "payment_recovery": "rebuilding"})
    );
}

#[tokio::test]
async fn take_ticket_returns_a_testnet_payment_bound_to_the_code_hash() {
    let response = post_take_ticket(app_state(&[], CODE_HASH_ONE), CODE_HASH_ONE_BASE64).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response_json::<Value>(response).await,
        json!({
            "status": "payment_required",
            "code_hash": CODE_HASH_ONE,
            "payment_address": "0:1111111111111111111111111111111111111111111111111111111111111111",
            "amount_nano": "10000000",
            "comment": format!("acton-verify:v1:{CODE_HASH_ONE}")
        })
    );
}

#[tokio::test]
async fn take_ticket_rejects_an_invalid_code_hash() {
    let response = post_take_ticket(app_state(&[], CODE_HASH_ONE), "not-a-code-hash").await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json::<Value>(response).await,
        json!({"error": "code_hash must contain exactly 64 hexadecimal characters"})
    );
}

#[tokio::test]
async fn take_ticket_waits_for_payment_history_recovery() {
    let response =
        post_take_ticket(recovering_payment_app_state(CODE_HASH_ONE), CODE_HASH_ONE).await;

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        response_json::<Value>(response).await,
        json!({"error": "payment_recovery_in_progress: payment history is still being recovered"})
    );
}

#[tokio::test]
async fn verify_requires_a_payment_for_unverified_code() {
    let response = post_verify_without_payment(
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

    assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
    assert_eq!(
        response_json::<Value>(response).await,
        json!({"error": "missing required field: tx_hash"})
    );
}

#[tokio::test]
async fn verify_rejects_an_invalid_payment_transaction_hash() {
    let response = post_verify_without_payment(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            text_part("tx_hash", "123"),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json::<Value>(response).await,
        json!({
            "error": "payment_tx_hash_invalid: transaction hash must be 64 hexadecimal characters or a 32-byte base64 value"
        })
    );
}

#[tokio::test]
async fn verify_rejects_an_invalid_direct_code_hash_before_claiming_payment() {
    let response = post_verify_without_payment(
        payment_error_app_state(CODE_HASH_ONE, PaymentError::AlreadyUsed),
        vec![
            text_part("code_hash", "not-a-code-hash"),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            text_part("tx_hash", PAYMENT_TX_HASH),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json::<Value>(response).await,
        json!({"error": "code_hash must contain exactly 64 hexadecimal characters"})
    );
}

#[tokio::test]
async fn verify_maps_payment_failures_to_stable_http_contracts() {
    let cases = [
        (
            PaymentError::TransactionNotFound,
            StatusCode::PAYMENT_REQUIRED,
            "payment_not_found: transaction was not found on TON testnet".to_owned(),
        ),
        (
            PaymentError::InvalidTransaction,
            StatusCode::PAYMENT_REQUIRED,
            "payment_invalid: transaction is not a finalized incoming payment".to_owned(),
        ),
        (
            PaymentError::InsufficientAmount {
                expected: 1_000_000,
                actual: 999_999,
            },
            StatusCode::PAYMENT_REQUIRED,
            "payment_insufficient: expected at least 1000000 nanoGRAM, received 999999".to_owned(),
        ),
        (
            PaymentError::CodeHashMismatch,
            StatusCode::PAYMENT_REQUIRED,
            "payment_code_hash_mismatch: transaction comment does not match the requested code hash"
                .to_owned(),
        ),
        (
            PaymentError::AlreadyUsed,
            StatusCode::CONFLICT,
            "payment_used: transaction has already been used".to_owned(),
        ),
        (
            PaymentError::InProgress,
            StatusCode::CONFLICT,
            "payment_in_progress: transaction is already being processed".to_owned(),
        ),
    ];

    for (payment_error, expected_status, expected_error) in cases {
        let response = post_verify(
            payment_error_app_state(CODE_HASH_ONE, payment_error),
            valid_verify_parts(),
        )
        .await;

        assert_eq!(response.status(), expected_status, "{expected_error}");
        assert_eq!(
            response_json::<Value>(response).await,
            json!({"error": expected_error})
        );
    }
}

#[tokio::test]
async fn take_ticket_skips_payment_for_already_verified_code() {
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

    let response = post_take_ticket(state, CODE_HASH_ONE).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json::<Value>(response).await;
    assert_eq!(body["status"], "already_verified");
    assert_eq!(body["code_hash"], CODE_HASH_ONE);
    assert!(body["source_bundle_hash"].is_string());
    assert!(body["storage_revision"].is_string());
    assert!(body.get("payment_address").is_none());
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
    assert!(body["paths"]["/api/v1/take-ticket"].is_object());
    assert!(body["paths"]["/api/v1/verify"].is_object());
    assert!(body["paths"]["/api/v1/last_verified"].is_object());
    assert!(body["paths"]["/api/v1/statistics"].is_object());
    assert!(body["paths"]["/api/v1/statistics/history"].is_object());
    assert!(body["paths"]["/api/v1/abi"].is_object());
    assert!(body["paths"]["/api/v1/verification/status"].is_object());
    assert!(body["paths"]["/api/v1/verification/source"].is_object());
    assert!(body["components"]["schemas"]["VerifyResponse"].is_object());
    assert!(body["components"]["schemas"]["VerificationSourceResponse"].is_object());
    assert!(body["components"]["schemas"]["VerificationStatisticsResponse"].is_object());
    assert!(body["components"]["schemas"]["VerificationStatisticsHistoryResponse"].is_object());
    assert!(body["components"]["schemas"]["SourceFileResponse"].is_object());

    let take_ticket = &body["paths"]["/api/v1/take-ticket"]["post"];
    let verify = &body["paths"]["/api/v1/verify"]["post"];
    assert_eq!(take_ticket["operationId"], "take_ticket");
    assert_eq!(verify["operationId"], "verify");
    assert_eq!(response_statuses(take_ticket), ["200", "400", "502", "503"]);
    assert_eq!(
        response_statuses(verify),
        ["200", "400", "401", "402", "404", "409", "502", "503"]
    );
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
async fn statistics_returns_counts_by_language_and_compiler_version() {
    let state = mapped_compiler_app_state(&[
        ("tolk", "1.4.1", CODE_HASH_ONE),
        ("tolk", "1.5.0", CODE_HASH_TWO),
        ("func", "0.4.6", CODE_HASH_THREE),
    ]);

    for (code_hash, compiler_version) in [(CODE_HASH_ONE, "1.4.1"), (CODE_HASH_TWO, "1.5.0")] {
        let compile_params = json!({"compiler_version": compiler_version}).to_string();
        let response = post_verify(
            state.clone(),
            vec![
                text_part("code_hash", code_hash),
                text_part("language", "tolk"),
                owned_text_part("compile_params", compile_params),
                text_part("sources", SOURCES_MAIN),
                file_part("files", "main.tolk", "text/plain", "fun main() {}"),
            ],
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    let response = post_verify(
        state.clone(),
        vec![
            text_part("code_hash", CODE_HASH_THREE),
            text_part("language", "func"),
            text_part("compile_params", COMPILE_PARAMS_FUNC),
            text_part("sources", SOURCES_FUNC_MAIN),
            file_part("files", "main.fc", "text/plain", "() recv_internal() {}"),
        ],
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let response = get(state, "/api/v1/statistics").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json::<Value>(response).await;
    assert_eq!(
        body,
        json!({
            "total": 3,
            "languages": [
                {
                    "language": "func",
                    "total": 1,
                    "versions": [
                        {"version": "0.4.6", "total": 1}
                    ]
                },
                {
                    "language": "tolk",
                    "total": 2,
                    "versions": [
                        {"version": "1.4.1", "total": 1},
                        {"version": "1.5.0", "total": 1}
                    ]
                }
            ]
        })
    );
}

#[tokio::test]
async fn statistics_history_returns_timestamp_compiler_and_version_for_every_verification() {
    let state = mapped_compiler_app_state(&[
        ("tolk", "1.4.1", CODE_HASH_ONE),
        ("func", "0.4.6", CODE_HASH_TWO),
    ]);

    for (code_hash, language, compile_params, sources, file_name, source) in [
        (
            CODE_HASH_ONE,
            "tolk",
            COMPILE_PARAMS_TOLK,
            SOURCES_MAIN,
            "main.tolk",
            "fun main() {}",
        ),
        (
            CODE_HASH_TWO,
            "func",
            COMPILE_PARAMS_FUNC,
            SOURCES_FUNC_MAIN,
            "main.fc",
            "() recv_internal() {}",
        ),
    ] {
        let response = post_verify(
            state.clone(),
            vec![
                text_part("code_hash", code_hash),
                text_part("language", language),
                text_part("compile_params", compile_params),
                text_part("sources", sources),
                file_part("files", file_name, "text/plain", source),
            ],
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    let response = get(state, "/api/v1/statistics/history").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json::<Value>(response).await;
    assert_eq!(
        body,
        json!({
            "items": [
                {
                    "timestamp": 1_700_000_000_u64,
                    "compiler": "func",
                    "version": "0.4.6"
                },
                {
                    "timestamp": 1_700_000_000_u64,
                    "compiler": "tolk",
                    "version": "1.4.1"
                }
            ]
        })
    );
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
async fn verify_accepts_original_verified_at_with_api_key() {
    let state = app_state_with_api_key(&[], CODE_HASH_ONE, API_KEY);
    let response = post_verify_with_api_key(
        state.clone(),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            text_part("verified_at", ORIGINAL_VERIFIED_AT),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
        API_KEY,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let response = get(
        state,
        &format!("/api/v1/verification/source?code_hash={CODE_HASH_ONE}"),
    )
    .await;
    let body = response_json::<VerificationSourceResponse>(response).await;
    let bundle = body.bundle.expect("verified bundle should exist");
    assert_eq!(bundle.verified_at, 1_678_647_600);
}

#[tokio::test]
async fn verify_rejects_verified_at_without_valid_api_key() {
    let state = app_state_with_api_key(&[], CODE_HASH_ONE, API_KEY);
    let parts = || {
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            text_part("verified_at", ORIGINAL_VERIFIED_AT),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ]
    };

    let response = post_verify(state.clone(), parts()).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_error_contains(response, "valid API key").await;

    let response = post_verify_with_api_key(state, parts(), "wrong-api-key").await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_error_contains(response, "valid API key").await;

    let response = post_verify_with_api_key(app_state(&[], CODE_HASH_ONE), parts(), API_KEY).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_error_contains(response, "valid API key").await;
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
async fn verify_treats_link_multipart_parts_as_regular_file_contents() {
    let state = app_state(&[], CODE_HASH_ONE);
    let response = post_verify(
        state.clone(),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part(
                "sources",
                r#"[
                  {"path":"symlink.tolk","is_entrypoint":true},
                  {"path":"hardlink.tolk","is_entrypoint":false}
                ]"#,
            ),
            file_part("files", "symlink.tolk", "inode/symlink", "../target.tolk"),
            file_part(
                "files",
                "hardlink.tolk",
                "application/x-hardlink",
                "symlink.tolk",
            ),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let response = get(
        state,
        &format!("/api/v1/verification/source?code_hash={CODE_HASH_ONE}"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<VerificationSourceResponse>(response).await;
    let bundle = body
        .bundle
        .expect("verified source should include a bundle");
    assert_eq!(bundle.files.len(), 2);
    assert_eq!(bundle.files[0].path, "hardlink.tolk");
    assert_eq!(bundle.files[0].content, "symlink.tolk");
    assert_eq!(bundle.files[1].path, "symlink.tolk");
    assert_eq!(bundle.files[1].content, "../target.tolk");
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
async fn verify_accepts_func_source_with_func_extension() {
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "func"),
            text_part("compile_params", COMPILE_PARAMS_FUNC),
            text_part("sources", r#"[{"path":"main.func","is_entrypoint":true}]"#),
            file_part("files", "main.func", "text/plain", "() main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
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
    let state = app_state(&[], CODE_HASH_ONE);
    let response = post_verify(
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

    let source_response = get(
        state,
        &format!("/api/v1/verification/source?code_hash={CODE_HASH_ONE}"),
    )
    .await;
    assert_eq!(source_response.status(), StatusCode::OK);
    let source = response_json::<VerificationSourceResponse>(source_response).await;
    assert_eq!(
        source
            .bundle
            .and_then(|bundle| bundle.payment_tx_hash)
            .as_deref(),
        Some("a07d951a702b910d5f65b710ca8ce9667bd0f3d803cf848e01f75744a08d394b")
    );
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
async fn deterministic_mismatch_consumes_the_payment() {
    let (state, outcomes) = recording_payment_app_state(CODE_HASH_TWO);
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
    assert_eq!(
        *outcomes
            .lock()
            .expect("payment outcomes mutex should not be poisoned"),
        [PaymentAttemptOutcome::Consumed]
    );
}

#[tokio::test]
async fn verify_hides_nonretryable_source_storage_failure() {
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
    assert_eq!(
        response_json::<Value>(response).await,
        json!({"error": "internal verifier error"})
    );
}

#[tokio::test]
async fn transient_source_storage_failure_keeps_the_payment_retryable() {
    let (state, outcomes) = failing_source_storage_app_state_with_payment_outcomes(CODE_HASH_ONE);
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

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = response_json::<Value>(response).await;
    assert_eq!(
        body,
        json!({"error": "verification_retryable: source storage is temporarily unavailable"})
    );
    assert!(!body.to_string().contains("source storage failed"));
    assert_eq!(
        *outcomes
            .lock()
            .expect("payment outcomes mutex should not be poisoned"),
        [PaymentAttemptOutcome::Retryable]
    );
}

#[tokio::test]
async fn retryable_storage_failure_allows_a_second_request_with_the_same_payment() {
    let (state, recorded_requests) =
        fail_once_source_storage_app_state(CODE_HASH_ONE, CODE_HASH_ONE).await;

    let first_response = post_verify(state.clone(), valid_verify_parts()).await;
    assert_eq!(first_response.status(), StatusCode::BAD_GATEWAY);
    let first_body = response_json::<Value>(first_response).await;
    assert_eq!(
        first_body,
        json!({"error": "verification_retryable: source storage is temporarily unavailable"})
    );
    assert!(
        !first_body
            .to_string()
            .contains("source storage internal test details")
    );

    let second_response = post_verify(state, valid_verify_parts()).await;
    assert_eq!(second_response.status(), StatusCode::OK);
    assert_eq!(
        response_json::<VerifyResponse>(second_response)
            .await
            .verification_result,
        "match"
    );
    assert_eq!(
        recorded_requests
            .lock()
            .expect("recorded source storage requests mutex should not be poisoned")
            .len(),
        2
    );
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
async fn deterministic_compiler_failure_consumes_the_payment() {
    let (state, outcomes) =
        failing_compiler_app_state_with_payment_outcomes("Tolk syntax error at main.tolk:1:5");
    let response = post_verify(state, valid_verify_parts()).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json::<Value>(response).await,
        json!({"error": "Tolk syntax error at main.tolk:1:5"})
    );
    assert_eq!(
        *outcomes
            .lock()
            .expect("payment outcomes mutex should not be poisoned"),
        [PaymentAttemptOutcome::Consumed]
    );
}

#[tokio::test]
async fn internal_compiler_failure_is_hidden_and_consumes_the_payment() {
    let (state, outcomes) = timing_out_compiler_app_state_with_payment_outcomes(12_345);
    let response = post_verify(state, valid_verify_parts()).await;

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = response_json::<Value>(response).await;
    assert_eq!(body, json!({"error": "internal verifier error"}));
    assert!(!body.to_string().contains("12_345"));
    assert!(!body.to_string().contains("12345"));
    assert_eq!(
        *outcomes
            .lock()
            .expect("payment outcomes mutex should not be poisoned"),
        [PaymentAttemptOutcome::Consumed]
    );
}

#[tokio::test]
async fn restart_rebuilds_consumed_payments_from_file_backed_history() {
    let directory = tempfile::tempdir().expect("temporary ledger directory should be created");
    let ledger_path = directory.path().join("payments.sqlite3");
    let transaction = payment_transaction(PAYMENT_TX_HASH, CODE_HASH_ONE);

    let first_server = OnchainPaymentVerifier::new(
        Arc::new(StaticPaymentBlockchainClient::new(None, Vec::new())),
        PaymentLedger::open(&ledger_path).expect("file-backed payment ledger should open"),
        PAYMENT_ADDRESS.to_owned(),
        1_000_000,
    );
    first_server
        .recover()
        .await
        .expect("initial empty recovery should succeed");
    drop(first_server);

    let restarted_server = OnchainPaymentVerifier::new(
        Arc::new(StaticPaymentBlockchainClient::new(
            Some(transaction.clone()),
            vec![transaction],
        )),
        PaymentLedger::open(&ledger_path).expect("file-backed payment ledger should reopen"),
        PAYMENT_ADDRESS.to_owned(),
        1_000_000,
    );
    restarted_server
        .recover()
        .await
        .expect("restart recovery should rebuild payment history");

    assert!(matches!(
        restarted_server.claim(PAYMENT_TX_HASH, CODE_HASH_ONE).await,
        Err(PaymentError::AlreadyUsed)
    ));
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
async fn verify_compares_source_path_duplicates_case_insensitively() {
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part(
                "sources",
                r#"[
                  {"path":"Contracts/Aa.tolk","is_entrypoint":true},
                  {"path":"contracts/aa.tolk","is_entrypoint":false},
                  {"path":"CONTRACTS/aA.TOLK","is_entrypoint":false},
                  {"path":"CoNtRaCtS/AA.ToLk","is_entrypoint":false}
                ]"#,
            ),
            file_part("files", "Contracts/Aa.tolk", "text/plain", "source one"),
            file_part("files", "contracts/aa.tolk", "text/plain", "source two"),
            file_part("files", "CONTRACTS/aA.TOLK", "text/plain", "source three"),
            file_part("files", "CoNtRaCtS/AA.ToLk", "text/plain", "source four"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_error_contains(response, "duplicate source paths").await;

    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part(
                "sources",
                r#"[
                  {"path":"one/main.tolk","is_entrypoint":true},
                  {"path":"two/main.tolk","is_entrypoint":false}
                ]"#,
            ),
            file_part("files", "one/main.tolk", "text/plain", "source one"),
            file_part("files", "two/main.tolk", "text/plain", "source two"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
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
async fn verify_accepts_source_path_at_length_limit() {
    let path = format!("path/to/large{}/file.tolk", "a".repeat(105));
    assert_eq!(path.chars().count(), 128);
    let sources = serde_json::to_string(&json!([{
        "path": path,
        "is_entrypoint": true,
    }]))
    .expect("source metadata should serialize");
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            owned_text_part("sources", sources),
            owned_file_part("files", path, "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn verify_rejects_source_path_over_length_limit() {
    let path = format!("path/to/large{}/file.tolk", "a".repeat(106));
    assert_eq!(path.chars().count(), 129);
    let sources = serde_json::to_string(&json!([{
        "path": path,
        "is_entrypoint": true,
    }]))
    .expect("source metadata should serialize");
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            owned_text_part("sources", sources),
            owned_file_part("files", path, "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_error_contains(response, "no longer than 128 characters").await;
}

#[tokio::test]
async fn verify_rejects_non_ascii_source_paths() {
    for path in [
        concat!("ca", "f", "\u{e9}.tolk"),
        concat!("ca", "f", "e\u{301}.tolk"),
        "contracts/🚀.tolk",
        "contracts/👩‍💻.tolk",
        "contracts/👍🏽.tolk",
        "contracts/✈️.tolk",
        "contracts/\u{fb01}.tolk",
        "contracts/Ａ.tolk",
        "contracts/контракт.tolk",
        "contracts/foo\u{a0}bar.tolk",
    ] {
        let sources = serde_json::to_string(&json!([{
            "path": path,
            "is_entrypoint": true,
        }]))
        .expect("source metadata should serialize");
        let response = post_verify(
            app_state(&[], CODE_HASH_ONE),
            vec![
                text_part("code_hash", CODE_HASH_ONE),
                text_part("language", "tolk"),
                text_part("compile_params", COMPILE_PARAMS_TOLK),
                owned_text_part("sources", sources),
                file_part("files", path, "text/plain", "fun main() {}"),
            ],
        )
        .await;
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "non-ASCII path should be rejected: {path:?}"
        );
        assert_error_contains(response, "only ASCII letters").await;
    }
}

#[tokio::test]
async fn verify_rejects_control_characters_in_source_paths() {
    for path in [
        "contracts/file\0.tolk",
        "contracts/file\n.tolk",
        "contracts/file\r.tolk",
        "contracts/file\t.tolk",
        "contracts/file\u{7f}.tolk",
    ] {
        let sources = serde_json::to_string(&json!([{
            "path": path,
            "is_entrypoint": true,
        }]))
        .expect("source metadata should serialize");
        let response = post_verify(
            app_state(&[], CODE_HASH_ONE),
            vec![
                text_part("code_hash", CODE_HASH_ONE),
                text_part("language", "tolk"),
                text_part("compile_params", COMPILE_PARAMS_TOLK),
                owned_text_part("sources", sources),
                file_part("files", path, "text/plain", "fun main() {}"),
            ],
        )
        .await;

        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "control character should be rejected: {path:?}"
        );
    }
}

#[tokio::test]
async fn verify_rejects_unsafe_source_paths() {
    let cases = [
        ("../main.tolk", "invalid component"),
        ("imports/../../main.tolk", "invalid component"),
        ("/main.tolk", "must be relative"),
        ("~/main.tolk", "must not start with '~'"),
        ("~user/main.tolk", "must not start with '~'"),
        ("C:/main.tolk", "Windows drive prefix"),
        ("./main.tolk", "invalid component"),
        ("imports/./main.tolk", "invalid component"),
        ("imports//main.tolk", "empty component"),
        ("imports/main.tolk/", "empty component"),
        (".git/main.tolk", "reserved '.git' component"),
        (" main.tolk", "leading or trailing whitespace"),
        ("main.tolk ", "leading or trailing whitespace"),
        ("main.tolk.", "must not end with '.'"),
        ("contracts./main.tolk", "must not end with '.'"),
        ("contracts/file name.tolk", "only ASCII letters"),
        ("contracts/file@name.tolk", "only ASCII letters"),
        ("contracts/file+name.tolk", "only ASCII letters"),
        ("contracts/file=name.tolk", "only ASCII letters"),
        ("contracts/file,name.tolk", "only ASCII letters"),
        ("contracts/file:name.tolk", "only ASCII letters"),
        ("contracts/file<name>.tolk", "only ASCII letters"),
        ("contracts/file|name.tolk", "only ASCII letters"),
        ("contracts/file?name.tolk", "only ASCII letters"),
        ("contracts/file*name.tolk", "only ASCII letters"),
    ];

    for (path, expected_error) in cases {
        let sources = serde_json::to_string(&json!([{
            "path": path,
            "is_entrypoint": true,
        }]))
        .expect("source metadata should serialize");
        let response = post_verify(
            app_state(&[], CODE_HASH_ONE),
            vec![
                text_part("code_hash", CODE_HASH_ONE),
                text_part("language", "tolk"),
                text_part("compile_params", COMPILE_PARAMS_TOLK),
                owned_text_part("sources", sources),
                owned_file_part("files", path, "text/plain", "fun main() {}"),
            ],
        )
        .await;

        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "path should be rejected: {path}"
        );
        assert_error_contains(response, expected_error).await;
    }
}

#[tokio::test]
async fn verify_accepts_portable_ascii_source_path() {
    let path = "Contracts_123/lib-name.v1.tolk";
    let sources = serde_json::to_string(&json!([{
        "path": path,
        "is_entrypoint": true,
    }]))
    .expect("source metadata should serialize");
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            owned_text_part("sources", sources),
            file_part("files", path, "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn verify_rejects_git_control_paths() {
    for path in [
        ".git",
        ".git/config",
        ".git/main.tolk",
        ".gitignore",
        ".gitattributes",
        ".gitmodules",
        "contracts/.gitignore",
        "contracts/.gitattributes",
        "contracts/.gitmodules",
        ".mailmap",
        ".gitconfig",
    ] {
        let sources = serde_json::to_string(&json!([{
            "path": path,
            "is_entrypoint": true,
        }]))
        .expect("source metadata should serialize");
        let response = post_verify(
            app_state(&[], CODE_HASH_ONE),
            vec![
                text_part("code_hash", CODE_HASH_ONE),
                text_part("language", "tolk"),
                text_part("compile_params", COMPILE_PARAMS_TOLK),
                owned_text_part("sources", sources),
                owned_file_part("files", path, "text/plain", "fun main() {}"),
            ],
        )
        .await;

        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "Git control path should be rejected: {path}"
        );
    }
}

#[tokio::test]
async fn verify_rejects_source_in_output_directory() {
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part(
                "sources",
                r#"[{"path":"output/main.tolk","is_entrypoint":true}]"#,
            ),
            file_part("files", "output/main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_error_contains(response, "reserved output directory").await;
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
async fn verify_rejects_source_extensions_that_do_not_match_language() {
    let cases = [
        (
            "tolk",
            COMPILE_PARAMS_TOLK,
            r#"[{"path":"main.fc","is_entrypoint":true}]"#,
            "main.fc",
            ".tolk",
        ),
        (
            "func",
            COMPILE_PARAMS_FUNC,
            r#"[{"path":"main.tolk","is_entrypoint":true}]"#,
            "main.tolk",
            ".fc, .func",
        ),
        (
            "tact",
            EMPTY_COMPILE_PARAMS,
            r#"[{"path":"contract.json","is_entrypoint":true}]"#,
            "contract.json",
            ".pkg, .tact",
        ),
    ];

    for (language, compile_params, sources, file_name, expected_extensions) in cases {
        let response = post_verify(
            app_state(&[], CODE_HASH_ONE),
            vec![
                text_part("code_hash", CODE_HASH_ONE),
                text_part("language", language),
                text_part("compile_params", compile_params),
                text_part("sources", sources),
                file_part("files", file_name, "text/plain", "source"),
            ],
        )
        .await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_error_contains(response, expected_extensions).await;
    }
}

#[tokio::test]
async fn verify_rejects_multiple_source_extensions() {
    for (language, compile_params, path) in [
        ("func", COMPILE_PARAMS_FUNC, "main.tolk.fc"),
        ("func", COMPILE_PARAMS_FUNC, "main.func.fc"),
        ("tolk", COMPILE_PARAMS_TOLK, "main.fc.tolk"),
        ("tact", EMPTY_COMPILE_PARAMS, "contract.tact.pkg"),
        ("tolk", COMPILE_PARAMS_TOLK, "main.FC.ToLk"),
    ] {
        let sources = serde_json::to_string(&json!([{
            "path": path,
            "is_entrypoint": true,
        }]))
        .expect("source metadata should serialize");
        let response = post_verify(
            app_state(&[], CODE_HASH_ONE),
            vec![
                text_part("code_hash", CODE_HASH_ONE),
                text_part("language", language),
                text_part("compile_params", compile_params),
                owned_text_part("sources", sources),
                file_part("files", path, "text/plain", "source"),
            ],
        )
        .await;

        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "multiple source extensions should be rejected: {path}"
        );
        assert_error_contains(response, "multiple source extensions").await;
    }

    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part(
                "sources",
                r#"[{"path":"contracts/my.contract.tolk","is_entrypoint":true}]"#,
            ),
            file_part(
                "files",
                "contracts/my.contract.tolk",
                "text/plain",
                "fun main() {}",
            ),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
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
    payment_tx_hash: Option<String>,
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
