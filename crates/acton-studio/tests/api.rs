use std::collections::BTreeSet;

use acton_studio::{
    STUDIO_HEALTH_PATH, STUDIO_INFO_PATH, STUDIO_OPENAPI_PATH, StudioServer, StudioServerConfig,
    StudioWorkspace,
};
use axum::body::to_bytes;
use axum::http::Request;
use expect_test::expect;
use tower::ServiceExt;

fn server() -> StudioServer {
    StudioServer::new(
        StudioServerConfig::new("test-version").with_workspace(
            StudioWorkspace::new("counter", "/private/workspaces/counter")
                .with_wallet_names(vec!["deployer".to_owned(), "treasury".to_owned()]),
        ),
    )
}

#[tokio::test]
async fn info_contract_does_not_expose_host_paths_or_deployment_mode() {
    let response = server()
        .router()
        .oneshot(
            Request::get(STUDIO_INFO_PATH)
                .body(axum::body::Body::empty())
                .expect("info request must be valid"),
        )
        .await
        .expect("info request must succeed");
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("info body must be readable");
    let actual = format!("status: {status}\nbody: {}", String::from_utf8_lossy(&body));

    expect![[r#"status: 200 OK
body: {"protocolVersion":1,"serverVersion":"test-version","workspace":{"name":"counter","walletNames":["deployer","treasury"]}}"#]]
    .assert_eq(&actual);
}

#[tokio::test]
async fn health_contract_is_minimal() {
    let response = server()
        .router()
        .oneshot(
            Request::get(STUDIO_HEALTH_PATH)
                .body(axum::body::Body::empty())
                .expect("health request must be valid"),
        )
        .await
        .expect("health request must succeed");
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("health body must be readable");
    let actual = format!("status: {status}\nbody: {}", String::from_utf8_lossy(&body));

    expect![[r"status: 204 No Content
body: "]]
    .assert_eq(&actual);
}

#[tokio::test]
async fn openapi_contract_lists_every_studio_operation_and_resolves_schema_references() {
    let response = server()
        .router()
        .oneshot(
            Request::get(STUDIO_OPENAPI_PATH)
                .body(axum::body::Body::empty())
                .expect("OpenAPI request must be valid"),
        )
        .await
        .expect("OpenAPI request must succeed");
    let status = response.status();
    let content_type = response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("<missing>")
        .to_owned();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("OpenAPI body must be readable");
    let document: serde_json::Value =
        serde_json::from_slice(&body).expect("OpenAPI body must be valid JSON");

    let methods = [
        "delete", "get", "head", "options", "patch", "post", "put", "trace",
    ];
    let mut operations = Vec::new();
    for (path, item) in document["paths"]
        .as_object()
        .expect("OpenAPI paths must be an object")
    {
        let item = item
            .as_object()
            .expect("OpenAPI path item must be an object");
        for method in methods {
            if item.contains_key(method) {
                operations.push(format!("{} {path}", method.to_ascii_uppercase()));
            }
        }
    }
    operations.sort();

    let schemas = document["components"]["schemas"]
        .as_object()
        .expect("OpenAPI schemas must be an object")
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut references = BTreeSet::new();
    collect_schema_references(&document, &mut references);
    let missing_references = references
        .iter()
        .filter_map(|reference| {
            reference
                .strip_prefix("#/components/schemas/")
                .filter(|name| !schemas.contains(*name))
        })
        .collect::<Vec<_>>();

    let actual = format!(
        "status: {status}\ncontent-type: {content_type}\nopenapi: {}\ntitle: {}\nversion: {}\noperations: {}\n{}\nschemas: {}\nmissing schema references: {}",
        document["openapi"].as_str().unwrap_or("<missing>"),
        document["info"]["title"].as_str().unwrap_or("<missing>"),
        document["info"]["version"].as_str().unwrap_or("<missing>"),
        operations.len(),
        operations.join("\n"),
        schemas.len(),
        if missing_references.is_empty() {
            "none".to_owned()
        } else {
            missing_references.join(", ")
        }
    );

    expect![[r"
        status: 200 OK
        content-type: application/json
        openapi: 3.1.0
        title: Acton Studio API
        version: 1.0.0
        operations: 51
        DELETE /api/v1/environments/{environment_id}
        DELETE /api/v1/environments/{environment_id}/snapshots/{snapshot_id}
        GET /api/v1/environments
        GET /api/v1/environments/{environment_id}
        GET /api/v1/environments/{environment_id}/api-calls
        GET /api/v1/environments/{environment_id}/rpc/acton_getAddressName
        GET /api/v1/environments/{environment_id}/rpc/acton_getCompilerAbi
        GET /api/v1/environments/{environment_id}/rpc/acton_getRegisteredVerifiedSource
        GET /api/v1/environments/{environment_id}/rpc/acton_listCompilerAbis
        GET /api/v1/environments/{environment_id}/rpc/acton_listContracts
        GET /api/v1/environments/{environment_id}/rpc/acton_listVerifiedSources
        GET /api/v1/environments/{environment_id}/rpc/{path}
        GET /api/v1/environments/{environment_id}/snapshot-operation
        GET /api/v1/environments/{environment_id}/snapshots
        GET /api/v1/environments/{environment_id}/wallets
        GET /api/v1/health
        GET /api/v1/info
        GET /api/v1/openapi.json
        GET /api/v1/test-runs
        GET /api/v1/test-runs/events
        GET /api/v1/test-runs/{run_id}
        GET /api/v1/test-runs/{run_id}/artifacts/config
        GET /api/v1/test-runs/{run_id}/artifacts/contract/{name}
        GET /api/v1/test-runs/{run_id}/artifacts/coverage.lcov
        GET /api/v1/test-runs/{run_id}/artifacts/file
        GET /api/v1/test-runs/{run_id}/artifacts/gas-profile
        GET /api/v1/test-runs/{run_id}/artifacts/health
        GET /api/v1/test-runs/{run_id}/artifacts/reports
        GET /api/v1/test-runs/{run_id}/artifacts/test-logs
        GET /api/v1/test-runs/{run_id}/artifacts/trace/{name}
        GET /api/v1/test-runs/{run_id}/output
        PATCH /api/v1/environments/{environment_id}
        POST /api/v1/environments
        POST /api/v1/environments/{environment_id}/restart
        POST /api/v1/environments/{environment_id}/rpc
        POST /api/v1/environments/{environment_id}/rpc/acton_deleteCompilerAbi
        POST /api/v1/environments/{environment_id}/rpc/acton_deleteContract
        POST /api/v1/environments/{environment_id}/rpc/acton_deleteVerifiedSource
        POST /api/v1/environments/{environment_id}/rpc/acton_deleteVerifiedSourceArtifact
        POST /api/v1/environments/{environment_id}/rpc/acton_registerCompilerAbis
        POST /api/v1/environments/{environment_id}/rpc/acton_registerContract
        POST /api/v1/environments/{environment_id}/rpc/acton_registerVerifiedSources
        POST /api/v1/environments/{environment_id}/rpc/acton_setAddressName
        POST /api/v1/environments/{environment_id}/rpc/{path}
        POST /api/v1/environments/{environment_id}/snapshots
        POST /api/v1/environments/{environment_id}/snapshots/{snapshot_id}/restore
        POST /api/v1/environments/{environment_id}/stop
        POST /api/v1/environments/{environment_id}/wallets/{wallet_name}/sign
        POST /api/v1/test-runs
        POST /api/v1/test-runs/{run_id}/cancel
        POST /api/v1/test-runs/{run_id}/events
        schemas: 71
        missing schema references: none"]]
    .assert_eq(&actual);
}

fn collect_schema_references(value: &serde_json::Value, references: &mut BTreeSet<String>) {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                collect_schema_references(value, references);
            }
        }
        serde_json::Value::Object(object) => {
            if let Some(reference) = object.get("$ref").and_then(serde_json::Value::as_str) {
                references.insert(reference.to_owned());
            }
            for value in object.values() {
                collect_schema_references(value, references);
            }
        }
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => {}
    }
}

#[tokio::test]
async fn unknown_api_routes_never_fall_back_to_the_studio_ui() {
    let response = server()
        .router()
        .oneshot(
            Request::get("/api/v2/unknown")
                .body(axum::body::Body::empty())
                .expect("unknown API request must be valid"),
        )
        .await
        .expect("unknown API request must succeed");
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("unknown API body must be readable");
    let actual = format!("status: {status}\nbody: {}", String::from_utf8_lossy(&body));

    expect![[r"status: 404 Not Found
body: "]]
    .assert_eq(&actual);
}
