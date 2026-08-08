//! Integration tests for HTTP routers and backend calls.
//!
//! Tests bind ephemeral loopback ports and exercise real HTTP requests. They
//! cover endpoint discovery, CORS/PNA headers, V2 forwarding, faucet input
//! validation, BoC submission, and detection of the confirmed internal message.

use std::{
    collections::BTreeMap,
    net::Ipv4Addr,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use axum::{
    Json, Router,
    extract::Query,
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    middleware,
    routing::{get, post},
};
use expect_test::expect;
use serde_json::{Value, json};
use tokio::{net::TcpListener, task::JoinHandle};

use crate::{
    operations::wallets,
    storage::{RuntimeState, Settings},
};

use super::{FUND_ACCOUNT_PATH, admin, config, cors, faucet, proxy};

async fn serve_test_router(app: Router) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{address}"), task)
}

fn faucet_router(state: faucet::State) -> Router {
    Router::new()
        .route(
            FUND_ACCOUNT_PATH,
            post(faucet::fund_account_handler).options(cors::preflight),
        )
        .layer(middleware::from_fn(cors::browser_headers))
        .with_state(state)
}

#[test]
fn root_document_lists_enabled_service_endpoints() {
    let mut settings = Settings::default();
    settings.services.ton_http_api.enabled = true;
    let mut runtime = RuntimeState::new();
    runtime.ready = true;
    runtime.masterchain_seqno = Some(42);

    let document = config::root_document(&settings, &runtime);

    assert_eq!(document.service, "localton");
    assert!(document.ready);
    assert_eq!(document.masterchain_seqno, Some(42));
    assert_eq!(
        document.endpoints.global_config,
        "http://127.0.0.1:18000/localhost.global.config.json"
    );
    assert_eq!(
        document.endpoints.fund_account.as_deref(),
        Some("http://127.0.0.1:18001/acton_fundAccount")
    );
    assert_eq!(
        document.endpoints.ton_http_api.as_deref(),
        Some("http://127.0.0.1:18002/api/v2")
    );
}

#[test]
fn openapi_documents_config_and_admin_routes() {
    let config_document = serde_json::to_value(config::openapi()).unwrap();
    assert_eq!(config_document["openapi"], "3.1.0");
    for path in [
        "/",
        "/localhost.global.config.json",
        "/config",
        "/live",
        "/healthz",
        "/add-validator",
    ] {
        assert!(
            config_document["paths"][path].is_object(),
            "configuration OpenAPI is missing {path}"
        );
    }
    assert!(config_document["components"]["schemas"]["ConfigDocument"].is_object());

    let admin_document = serde_json::to_value(admin::openapi()).unwrap();
    assert_eq!(admin_document["openapi"], "3.1.0");
    for path in [
        "/v1/status",
        "/v1/settings",
        "/v1/wallets",
        "/v1/processes",
        "/v1/nodes/{name}/start",
        "/v1/nodes/{name}/stop",
        "/acton_fundAccount",
    ] {
        assert!(
            admin_document["paths"][path].is_object(),
            "administrative OpenAPI is missing {path}"
        );
    }
    for schema in [
        "RuntimeState",
        "NodeRuntime",
        "ServiceRuntime",
        "Settings",
        "NetworkSettings",
        "NodeSettings",
        "ServiceSettings",
        "HttpServiceSettings",
        "TonHttpApiSettings",
        "ValidationSettings",
        "MonitoringSettings",
        "PublicWallet",
        "StoredWalletVersion",
        "ProcessInfo",
        "FundAccountRequest",
        "FundAccountResponse",
        "FundAccountErrorResponse",
        "ErrorResponse",
    ] {
        assert!(
            admin_document["components"]["schemas"][schema].is_object(),
            "administrative OpenAPI is missing {schema}"
        );
    }

    let documented_operations = [
        ("CONFIG /", &config_document["paths"]["/"]["get"]),
        (
            "CONFIG /add-validator",
            &config_document["paths"]["/add-validator"]["get"],
        ),
        (
            "ADMIN /v1/status",
            &admin_document["paths"]["/v1/status"]["get"],
        ),
        (
            "ADMIN /v1/nodes/{name}/start",
            &admin_document["paths"]["/v1/nodes/{name}/start"]["post"],
        ),
        (
            "ADMIN /acton_fundAccount",
            &admin_document["paths"]["/acton_fundAccount"]["post"],
        ),
    ]
    .map(|(name, operation)| {
        format!(
            "{name}\nsummary: {}\ndescription: {}",
            operation["summary"].as_str().unwrap_or("<missing>"),
            operation["description"].as_str().unwrap_or("<missing>"),
        )
    })
    .join("\n\n");
    let documented_fields = format!(
        "ConfigDocument.ready: {}\nRuntimeState.ready: {}\nFundAccountRequest.address: {}",
        config_document["components"]["schemas"]["ConfigDocument"]["properties"]["ready"]
            ["description"]
            .as_str()
            .unwrap_or("<missing>"),
        admin_document["components"]["schemas"]["RuntimeState"]["properties"]["ready"]
            ["description"]
            .as_str()
            .unwrap_or("<missing>"),
        admin_document["components"]["schemas"]["FundAccountRequest"]["properties"]["address"]
            ["description"]
            .as_str()
            .unwrap_or("<missing>"),
    );

    expect![[r#"
        CONFIG /
        summary: Get network status and service URLs
        description: Use this endpoint to find the enabled Localton services

        CONFIG /add-validator
        summary: Create and start a validator node
        description: By default, the new validator enters elections automatically

        ADMIN /v1/status
        summary: Get the current launcher and network state
        description: The response shows readiness, the latest masterchain block, node states, and service states

        ADMIN /v1/nodes/{name}/start
        summary: Start a configured validator node
        description: The node must exist in the persistent settings

        ADMIN /acton_fundAccount
        summary: Fund an account from the genesis wallet
        description: Localton sends a signed transfer and waits for the destination message

        ConfigDocument.ready: `true` when the local network can process requests
        RuntimeState.ready: `true` when the network can process requests
        FundAccountRequest.address: TON address that receives the funds"#]]
    .assert_eq(&format!("{documented_operations}\n\n{documented_fields}"));
}

#[test]
fn browser_headers_allow_actonscan_preflight() {
    let mut request_headers = HeaderMap::new();
    request_headers.insert(
        HeaderName::from_static("access-control-request-headers"),
        HeaderValue::from_static("content-type"),
    );
    let mut response_headers = HeaderMap::new();

    cors::apply_browser_headers(&mut response_headers, &request_headers, true);

    assert_eq!(
        response_headers["access-control-allow-origin"],
        HeaderValue::from_static("*")
    );
    assert_eq!(
        response_headers["access-control-allow-headers"],
        HeaderValue::from_static("content-type")
    );
    assert_eq!(
        response_headers["access-control-allow-private-network"],
        HeaderValue::from_static("true")
    );
}

#[tokio::test]
async fn ton_http_api_proxy_forwards_json_rpc_and_adds_cors() {
    let backend = Router::new().route(
        "/api/v2/jsonRPC",
        post(|| async {
            Json(json!({
                "ok": true,
                "result": [{"transaction_id": "test"}]
            }))
        }),
    );
    let (backend, backend_task) = serve_test_router(backend).await;
    let (public_api, public_api_task) = serve_test_router(proxy::router(backend)).await;

    let response = reqwest::Client::new()
        .post(format!("{public_api}/api/v2/jsonRPC"))
        .header("content-type", "application/json")
        .header("origin", "https://actonscan.com")
        .body(r#"{"jsonrpc":"2.0","method":"getTransactions"}"#)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()["access-control-allow-origin"],
        HeaderValue::from_static("*")
    );
    let body = response.bytes().await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["ok"], true);
    assert_eq!(json["result"][0]["transaction_id"], "test");
    public_api_task.abort();
    backend_task.abort();
}

#[tokio::test]
async fn admin_faucet_route_adds_cors_to_json_rejections() {
    let state = faucet::State::new("http://127.0.0.1:1".to_owned(), PathBuf::new());
    let (admin_api, admin_api_task) = serve_test_router(faucet_router(state)).await;

    let response = reqwest::Client::new()
        .post(format!("{admin_api}{FUND_ACCOUNT_PATH}"))
        .header("content-type", "application/json")
        .body("{")
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response.headers()["access-control-allow-origin"],
        HeaderValue::from_static("*")
    );
    admin_api_task.abort();
}

#[tokio::test]
async fn faucet_route_distinguishes_validation_and_infrastructure_errors_for_u128_amounts() {
    let state_dir = tempfile::tempdir().unwrap();
    std::fs::write(state_dir.path().join("manifest.json"), "{}").unwrap();
    let state = faucet::State::new(
        "http://127.0.0.1:1".to_owned(),
        state_dir.path().to_path_buf(),
    );
    let (admin_api, admin_api_task) = serve_test_router(faucet_router(state)).await;
    let client = reqwest::Client::new();

    let invalid = client
        .post(format!("{admin_api}{FUND_ACCOUNT_PATH}"))
        .header("content-type", "application/json")
        .body(r#"{"address":"not-an-address","amount":18446744073709551616}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        invalid.headers()["access-control-allow-origin"],
        HeaderValue::from_static("*")
    );
    let invalid: Value = serde_json::from_slice(&invalid.bytes().await.unwrap()).unwrap();
    assert_eq!(invalid["ok"], false);
    assert_eq!(invalid["code"], 400);
    assert!(
        invalid["error"]
            .as_str()
            .unwrap()
            .contains("invalid TON address")
    );

    let infrastructure = client
        .post(format!("{admin_api}{FUND_ACCOUNT_PATH}"))
        .header("content-type", "application/json")
        .body(format!(
            r#"{{"address":"0:{}","amount":18446744073709551616}}"#,
            "0".repeat(64)
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(infrastructure.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let infrastructure: Value =
        serde_json::from_slice(&infrastructure.bytes().await.unwrap()).unwrap();
    assert_eq!(infrastructure["ok"], false);
    assert_eq!(infrastructure["code"], 500);
    assert!(
        infrastructure["error"]
            .as_str()
            .unwrap()
            .contains("invalid manifest")
    );

    admin_api_task.abort();
}

#[tokio::test]
async fn faucet_submission_returns_confirmed_internal_message_hash() {
    let source = format!("-1:{}", "1".repeat(64));
    let destination = format!("0:{}", "2".repeat(64));
    let transaction_attempts = Arc::new(AtomicUsize::new(0));
    let attempts_for_handler = Arc::clone(&transaction_attempts);
    let source_for_handler = source.clone();
    let destination_for_handler = destination.clone();
    let backend = Router::new()
        .route(
            "/api/v2/sendBocReturnHash",
            post(|Json(payload): Json<Value>| async move {
                assert_eq!(payload["boc"], "AQID");
                Json(json!({
                    "ok": true,
                    "result": {
                        "@type": "raw.extMessageInfo",
                        "hash": "external-message-hash"
                    }
                }))
            }),
        )
        .route(
            "/api/v2/getTransactions",
            get(move |Query(parameters): Query<BTreeMap<String, String>>| {
                let source = source_for_handler.clone();
                let destination = destination_for_handler.clone();
                let attempts = Arc::clone(&attempts_for_handler);
                async move {
                    assert_eq!(parameters["address"], source);
                    assert_eq!(parameters["limit"], faucet::TRANSACTION_LOOKBACK);
                    let result = if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                        json!([])
                    } else {
                        json!([{
                            "in_msg": {
                                "hash": "external-message-hash"
                            },
                            "out_msgs": [
                                {
                                    "hash": "other-internal-message-hash",
                                    "destination": {
                                        "account_address": source
                                    }
                                },
                                {
                                    "hash": "internal-message-hash",
                                    "destination": destination
                                }
                            ]
                        }])
                    };
                    Json(json!({ "ok": true, "result": result }))
                }
            }),
        );
    let (backend, backend_task) = serve_test_router(backend).await;
    let state = faucet::State::new(backend, PathBuf::new());
    let message = wallets::FundAccountMessage {
        boc: vec![1, 2, 3],
        source_address: source,
        destination_address: destination,
        seqno: 7,
    };

    let external_hash = faucet::send_boc_return_hash(&state, &message.boc)
        .await
        .unwrap();
    let internal_hash = faucet::wait_for_transfer(&state, &message, &external_hash)
        .await
        .unwrap();

    assert_eq!(external_hash, "external-message-hash");
    assert_eq!(internal_hash, "internal-message-hash");
    assert!(transaction_attempts.load(Ordering::SeqCst) >= 2);
    backend_task.abort();
}
