use std::convert::Infallible;
use std::fmt::Write as _;
use std::fs;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use acton_studio::{
    ContractRegistryStore, CreateEnvironmentRequest, EnvironmentConfig, EnvironmentEndpoints,
    EnvironmentRuntime, EnvironmentRuntimeError, EnvironmentRuntimeFuture, EnvironmentStatus,
    StudioEnvironment, StudioServer, StudioServerConfig, UpdateEnvironmentRequest,
};
use axum::body::{Body, Bytes, to_bytes};
use axum::extract::State;
use axum::http::{Request, Response, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{any, get};
use axum::{Json, Router};
use expect_test::expect;
use serde_json::{Value, json};
use ton::ton_core::types::TonAddress;
use tower::ServiceExt;

const ENVIRONMENT_ID: &str = "full-ton-1";
const CONTRACT_ADDRESS: &str = "EQC8g9REpyFH8-occ0FD8DnFcjPxjVRjk2u_ESCFnMhmXo6z";
const DEPLOYMENT_ADDRESS: &str =
    "0:b9a663682236bafbc6c81bb2ec607630b09f8e233575e149ccd48b0ca9e13c6c";
const DEPLOYMENT_CODE_HASH: &str =
    "b993c68c596425f05d1bc492d7c03e2979ab669901ed5a57e35e6dd4d6089d27";
const DEPLOYMENT_BOC: &str = "te6ccgEBCAEA3gACq0gA3hg/j9iig2aTi8NU/hguuHV4Mf1mEUmqqnI9JLMCjg8ALmmY2giNrr7xsgbsuxgdjCwn44jNXXhSczUiwyp4TxsQ7msoAAAAAAAAAAAAANL430UZAgEAEAAAAAAAAAAAART/APSkE/S88sgLAwIBYgcEAgFYBgUAF7itDtRNDTHzHXCx+AAFu+F4AJzQ+JGRMOAg1ywj9DsnfI4YMe1E0AHXCx8B1h/XCx9YoAHIzssfye1U4NcsIdOpeDQxjhIw7UTQ1h8wyM7PkAAAAALJ7VTggQ/2AccA8vQ=";

#[derive(Clone)]
struct FixedEnvironmentRuntime {
    environment: StudioEnvironment,
}

impl EnvironmentRuntime for FixedEnvironmentRuntime {
    fn list(&self) -> EnvironmentRuntimeFuture<'_, Vec<StudioEnvironment>> {
        let environment = self.environment.clone();
        Box::pin(async move { Ok(vec![environment]) })
    }

    fn get(&self, environment_id: &str) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        let environment = (self.environment.id == environment_id).then(|| self.environment.clone());
        let environment_id = environment_id.to_owned();
        Box::pin(
            async move { environment.ok_or(EnvironmentRuntimeError::NotFound { environment_id }) },
        )
    }

    fn create(
        &self,
        _request: CreateEnvironmentRequest,
    ) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        Box::pin(async {
            Err(EnvironmentRuntimeError::Internal {
                code: "test_runtime_is_fixed",
                message: "The test runtime has a fixed environment".to_owned(),
            })
        })
    }

    fn update(
        &self,
        environment_id: &str,
        _request: UpdateEnvironmentRequest,
    ) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        let environment_id = environment_id.to_owned();
        Box::pin(async move { Err(EnvironmentRuntimeError::NotFound { environment_id }) })
    }

    fn delete(&self, environment_id: &str) -> EnvironmentRuntimeFuture<'_, ()> {
        let environment_id = environment_id.to_owned();
        Box::pin(async move { Err(EnvironmentRuntimeError::NotFound { environment_id }) })
    }

    fn stop(&self, environment_id: &str) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        let environment_id = environment_id.to_owned();
        Box::pin(async move { Err(EnvironmentRuntimeError::NotFound { environment_id }) })
    }

    fn restart(&self, environment_id: &str) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        let environment_id = environment_id.to_owned();
        Box::pin(async move { Err(EnvironmentRuntimeError::NotFound { environment_id }) })
    }
}

struct MockEnvironmentApi {
    base_url: String,
    requests: Arc<Mutex<Vec<String>>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for MockEnvironmentApi {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[derive(Clone, Debug)]
struct CapturedDeploymentRequest {
    body: Vec<u8>,
    content_length: Option<String>,
}

struct DeploymentMockEnvironmentApi {
    base_url: String,
    requests: Arc<Mutex<Vec<CapturedDeploymentRequest>>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for DeploymentMockEnvironmentApi {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn deployment_proxy_target(
    State(requests): State<Arc<Mutex<Vec<CapturedDeploymentRequest>>>>,
    request: Request<Body>,
) -> Response<Body> {
    let (parts, body) = request.into_parts();
    let body = to_bytes(body, usize::MAX)
        .await
        .expect("deployment request body must be readable");
    requests
        .lock()
        .expect("deployment request lock must not be poisoned")
        .push(CapturedDeploymentRequest {
            body: body.to_vec(),
            content_length: parts
                .headers
                .get("content-length")
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned),
        });
    let response = if parts.uri.query() == Some("reject=error") {
        json!({"error": {"code": -32000, "message": "rejected"}})
    } else {
        json!({"ok": true, "result": {}})
    };
    (
        StatusCode::OK,
        [("content-type", "application/json")],
        response.to_string(),
    )
        .into_response()
}

async fn spawn_deployment_mock_environment_api() -> DeploymentMockEnvironmentApi {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("deployment mock listener must bind");
    let address = listener
        .local_addr()
        .expect("deployment mock listener must have an address");
    let router = Router::new()
        .fallback(any(deployment_proxy_target))
        .with_state(requests.clone());
    let task = tokio::spawn(async move {
        axum::serve(listener, router.into_make_service())
            .await
            .expect("deployment mock must serve");
    });
    DeploymentMockEnvironmentApi {
        base_url: format!("http://{address}"),
        requests,
        task,
    }
}

async fn mock_account_states(
    State(requests): State<Arc<Mutex<Vec<String>>>>,
    request: Request<Body>,
) -> Json<Value> {
    requests
        .lock()
        .expect("mock request lock must not be poisoned")
        .push(format!("{} {}", request.method(), request.uri()));
    Json(json!({
        "accounts": [{
            "address": CONTRACT_ADDRESS,
            "account_state_hash": "00".repeat(32),
            "balance": "1234567890",
            "code_hash": "11".repeat(32),
            "contract_methods": [],
            "data_hash": "22".repeat(32),
            "extra_currencies": {},
            "interfaces": [],
            "last_transaction_hash": "33".repeat(32),
            "last_transaction_lt": "42",
            "status": "active"
        }]
    }))
}

async fn proxy_target(
    State(requests): State<Arc<Mutex<Vec<String>>>>,
    request: Request<Body>,
) -> (StatusCode, String) {
    requests
        .lock()
        .expect("mock request lock must not be poisoned")
        .push(format!("{} {}", request.method(), request.uri()));
    let (parts, body) = request.into_parts();
    let body = to_bytes(body, usize::MAX)
        .await
        .expect("proxied request body must be readable");
    let body = String::from_utf8_lossy(&body);
    let body_line = if body.is_empty() {
        "body:".to_owned()
    } else {
        format!("body: {body}")
    };
    (
        StatusCode::ACCEPTED,
        format!("method: {}\nuri: {}\n{body_line}", parts.method, parts.uri),
    )
}

async fn spawn_mock_environment_api() -> MockEnvironmentApi {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("mock environment API listener must bind");
    let address = listener
        .local_addr()
        .expect("mock environment API listener must have an address");
    let router = Router::new()
        .route("/api/v3/accountStates", get(mock_account_states))
        .fallback(any(proxy_target))
        .with_state(requests.clone());
    let task = tokio::spawn(async move {
        axum::serve(listener, router.into_make_service())
            .await
            .expect("mock environment API must serve");
    });
    MockEnvironmentApi {
        base_url: format!("http://{address}"),
        requests,
        task,
    }
}

fn full_ton_environment(base_url: &str) -> StudioEnvironment {
    StudioEnvironment::new(
        ENVIRONMENT_ID,
        "Full TON network",
        EnvironmentStatus::Running,
        EnvironmentConfig::FullTonNetwork {
            api_v2_port: 18080,
            api_v3_port: 18081,
            admin_port: 18082,
            config_port: 18083,
            imported_accounts: Vec::new(),
        },
        EnvironmentEndpoints {
            api_v2: Some(format!("{base_url}/api/v2")),
            api_v3: Some(format!("{base_url}/api/v3")),
            config: Some(format!("{base_url}/config")),
            control: Some(base_url.to_owned()),
        },
    )
}

fn localnet_environment(base_url: &str) -> StudioEnvironment {
    StudioEnvironment::new(
        "localnet-1",
        "Localnet",
        EnvironmentStatus::Running,
        EnvironmentConfig::ActonLocalnet {
            port: 5411,
            fork_network: None,
            fork_block_number: None,
            accounts: Vec::new(),
            rate_limit: None,
            response_delay_ms: None,
            block_interval_ms: None,
            no_mining: false,
            mine_empty_blocks: false,
        },
        EnvironmentEndpoints {
            api_v2: Some(format!("{base_url}/api/v2")),
            api_v3: Some(format!("{base_url}/api/v3")),
            config: None,
            control: Some(base_url.to_owned()),
        },
    )
}

fn router(environment: StudioEnvironment, contract_registry: ContractRegistryStore) -> Router {
    StudioServer::new(StudioServerConfig::new("test-version"))
        .with_environment_runtime(FixedEnvironmentRuntime { environment })
        .with_contract_registry(contract_registry)
        .router()
}

async fn response_snapshot(response: Response<Body>) -> String {
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body must be readable");
    let mut value: Value = serde_json::from_slice(&body).expect("response body must be JSON");
    value
        .as_object_mut()
        .expect("response body must be an object")
        .remove("@extra");
    format!(
        "status: {status}\nbody:\n{}",
        serde_json::to_string_pretty(&value).expect("response JSON must serialize")
    )
}

async fn raw_response_snapshot(response: Response<Body>) -> String {
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body must be readable");
    format!(
        "status: {status}\nbody:\n{}",
        String::from_utf8_lossy(&body)
    )
}

async fn proxy_and_registry_snapshot(
    upstream: &MockEnvironmentApi,
    project: &tempfile::TempDir,
    environment: StudioEnvironment,
    discovery_request: Request<Body>,
) -> String {
    let environment_id = environment.id.clone();
    let first_session = {
        let app = router(
            environment.clone(),
            ContractRegistryStore::for_project(project.path()),
        );
        let proxy = app
            .clone()
            .oneshot(discovery_request)
            .await
            .expect("discovery request must succeed");
        let list = app
            .oneshot(
                Request::get(format!(
                    "/api/v1/environments/{environment_id}/rpc/acton_listContracts"
                ))
                .body(Body::empty())
                .expect("list request must be valid"),
            )
            .await
            .expect("list request must succeed");
        format!(
            "PROXY REQUEST\n{}\n\nLIST\n{}",
            raw_response_snapshot(proxy).await,
            response_snapshot(list).await
        )
    };

    let reopened_list = router(
        environment,
        ContractRegistryStore::for_project(project.path()),
    )
    .oneshot(
        Request::get(format!(
            "/api/v1/environments/{environment_id}/rpc/acton_listContracts"
        ))
        .body(Body::empty())
        .expect("reopened list request must be valid"),
    )
    .await
    .expect("reopened list request must succeed");
    let upstream_requests = upstream
        .requests
        .lock()
        .expect("mock request lock must not be poisoned")
        .join("\n");
    format!(
        "{first_session}\n\nLIST AFTER STORE RECREATION\n{}\n\nUPSTREAM REQUESTS\n{upstream_requests}",
        response_snapshot(reopened_list).await
    )
}

#[tokio::test]
async fn full_ton_read_query_does_not_discover_contract() {
    let upstream = spawn_mock_environment_api().await;
    let project = tempfile::tempdir().expect("test project must be created");
    let environment = full_ton_environment(&upstream.base_url);
    let discovery_request = Request::get(format!(
        "/api/v1/environments/{ENVIRONMENT_ID}/rpc/api/v2/getAddressInformation?address={CONTRACT_ADDRESS}"
    ))
    .body(Body::empty())
    .expect("query discovery request must be valid");
    let actual =
        proxy_and_registry_snapshot(&upstream, &project, environment, discovery_request).await;

    expect![[r#"
        PROXY REQUEST
        status: 202 Accepted
        body:
        method: GET
        uri: /api/v2/getAddressInformation?address=EQC8g9REpyFH8-occ0FD8DnFcjPxjVRjk2u_ESCFnMhmXo6z
        body:

        LIST
        status: 200 OK
        body:
        {
          "ok": true,
          "result": []
        }

        LIST AFTER STORE RECREATION
        status: 200 OK
        body:
        {
          "ok": true,
          "result": []
        }

        UPSTREAM REQUESTS
        GET /api/v2/getAddressInformation?address=EQC8g9REpyFH8-occ0FD8DnFcjPxjVRjk2u_ESCFnMhmXo6z"#]]
    .assert_eq(&actual);
}

#[tokio::test]
async fn contract_facade_only_claims_its_exact_routes_and_methods() {
    let upstream = spawn_mock_environment_api().await;
    let project = tempfile::tempdir().expect("test project must be created");
    let app = router(
        localnet_environment(&upstream.base_url),
        ContractRegistryStore::for_project(project.path()),
    );
    let passthrough = app
        .clone()
        .oneshot(
            Request::post("/api/v1/environments/localnet-1/rpc/acton_mine")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"blocks":2}"#))
                .expect("passthrough request must be valid"),
        )
        .await
        .expect("passthrough request must succeed");
    let wrong_method = app
        .oneshot(
            Request::post("/api/v1/environments/localnet-1/rpc/acton_listContracts")
                .body(Body::empty())
                .expect("wrong-method request must be valid"),
        )
        .await
        .expect("wrong-method request must succeed");
    let actual = format!(
        "UNRELATED ACTON ROUTE\n{}\n\nKNOWN ROUTE WITH WRONG METHOD\n{}",
        raw_response_snapshot(passthrough).await,
        response_snapshot(wrong_method).await
    );

    expect![[r#"
        UNRELATED ACTON ROUTE
        status: 202 Accepted
        body:
        method: POST
        uri: /acton_mine
        body: {"blocks":2}

        KNOWN ROUTE WITH WRONG METHOD
        status: 405 Method Not Allowed
        body:
        {
          "code": 405,
          "error": "Method not allowed",
          "ok": false
        }"#]]
    .assert_eq(&actual);
}

#[tokio::test]
async fn localnet_json_body_read_does_not_discover_contract() {
    let upstream = spawn_mock_environment_api().await;
    let project = tempfile::tempdir().expect("test project must be created");
    let environment = localnet_environment(&upstream.base_url);
    let body = format!(r#"{{"address":"{CONTRACT_ADDRESS}","method":"seqno"}}"#);
    let discovery_request =
        Request::post("/api/v1/environments/localnet-1/rpc/api/v2/runGetMethod")
            .header("content-type", "application/json")
            .header("content-length", body.len())
            .body(Body::from(body))
            .expect("JSON discovery request must be valid");
    let actual =
        proxy_and_registry_snapshot(&upstream, &project, environment, discovery_request).await;

    expect![[r#"
        PROXY REQUEST
        status: 202 Accepted
        body:
        method: POST
        uri: /api/v2/runGetMethod
        body: {"address":"EQC8g9REpyFH8-occ0FD8DnFcjPxjVRjk2u_ESCFnMhmXo6z","method":"seqno"}

        LIST
        status: 200 OK
        body:
        {
          "ok": true,
          "result": []
        }

        LIST AFTER STORE RECREATION
        status: 200 OK
        body:
        {
          "ok": true,
          "result": []
        }

        UPSTREAM REQUESTS
        POST /api/v2/runGetMethod"#]]
    .assert_eq(&actual);
}

fn stored_deployment_candidate(project: &tempfile::TempDir) -> Option<Value> {
    let registry_path = project
        .path()
        .join(".studio/environments/localnet-1/registry.json");
    let registry: Value = serde_json::from_slice(&fs::read(registry_path).ok()?).ok()?;
    let mut candidate = registry
        .get("deploymentCandidates")?
        .get(DEPLOYMENT_ADDRESS)?
        .clone();
    let candidate_object = candidate
        .as_object_mut()
        .expect("deployment candidate must be an object");
    candidate_object.remove("observedAt");
    let display_address = candidate_object
        .get("address")
        .and_then(Value::as_str)
        .expect("deployment candidate must have an address");
    assert_eq!(
        TonAddress::from_str(display_address)
            .expect("deployment candidate address must be valid")
            .to_hex(),
        DEPLOYMENT_ADDRESS
    );
    candidate_object.insert(
        "address".to_owned(),
        Value::String("<validated-friendly-address>".to_owned()),
    );
    Some(candidate)
}

#[tokio::test]
async fn deployment_write_routes_record_tycho_candidates() {
    struct Case {
        label: &'static str,
        path: &'static str,
        content_type: &'static str,
        body: String,
        chunked: bool,
    }

    let json_body = || json!({"boc": DEPLOYMENT_BOC}).to_string();
    let json_rpc = |method| {
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": {"boc": DEPLOYMENT_BOC},
        })
        .to_string()
    };
    let form_body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("boc", DEPLOYMENT_BOC)
        .finish();
    let cases = vec![
        Case {
            label: "REST sendBoc",
            path: "api/v2/sendBoc",
            content_type: "application/json",
            body: json_body(),
            chunked: false,
        },
        Case {
            label: "REST sendBocReturnHash form",
            path: "api/v2/sendBocReturnHash",
            content_type: "application/x-www-form-urlencoded",
            body: form_body,
            chunked: false,
        },
        Case {
            label: "V3 message chunked",
            path: "api/v3/message",
            content_type: "application/json",
            body: json_body(),
            chunked: true,
        },
        Case {
            label: "Acton internal",
            path: "acton_sendInternalMessage",
            content_type: "application/json",
            body: json_body(),
            chunked: false,
        },
        Case {
            label: "JSON-RPC root sendBoc",
            path: "api/v2",
            content_type: "application/json",
            body: json_rpc("sendBoc"),
            chunked: false,
        },
        Case {
            label: "JSON-RPC canonical sendBocReturnHash",
            path: "api/v2/jsonRPC",
            content_type: "application/json",
            body: json_rpc("sendBocReturnHash"),
            chunked: false,
        },
        Case {
            label: "JSON-RPC nested sendBoc",
            path: "api/v2/v2/jsonRPC",
            content_type: "application/json",
            body: json_rpc("sendBoc"),
            chunked: false,
        },
    ];
    let upstream = spawn_deployment_mock_environment_api().await;
    let mut actual = String::new();

    for case in cases {
        let project = tempfile::tempdir().expect("test project must be created");
        let app = router(
            localnet_environment(&upstream.base_url),
            ContractRegistryStore::for_project(project.path()),
        );
        let body = if case.chunked {
            let body = case.body.as_bytes();
            let split = body.len() / 2;
            Body::from_stream(futures::stream::iter([
                Ok::<_, Infallible>(Bytes::copy_from_slice(&body[..split])),
                Ok(Bytes::copy_from_slice(&body[split..])),
            ]))
        } else {
            Body::from(case.body.clone())
        };
        let response = app
            .oneshot(
                Request::post(format!("/api/v1/environments/localnet-1/rpc/{}", case.path))
                    .header("content-type", case.content_type)
                    .body(body)
                    .expect("deployment request must be valid"),
            )
            .await
            .expect("deployment request must succeed");
        assert_eq!(response.status(), StatusCode::OK);
        to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("deployment response must be readable");
        let candidate =
            stored_deployment_candidate(&project).expect("deployment candidate must be persisted");
        assert_eq!(
            candidate.get("codeHash").and_then(Value::as_str),
            Some(DEPLOYMENT_CODE_HASH)
        );
        let captured = upstream
            .requests
            .lock()
            .expect("deployment request lock must not be poisoned")
            .last()
            .expect("upstream request must be captured")
            .clone();
        assert_eq!(captured.body, case.body.as_bytes());
        if case.chunked {
            assert_eq!(captured.content_length, None);
        }
        writeln!(
            actual,
            "{}: {}",
            case.label,
            serde_json::to_string(&candidate).expect("candidate must serialize")
        )
        .expect("deployment summary must be writable");
    }

    expect![[r#"
        REST sendBoc: {"address":"<validated-friendly-address>","codeHash":"b993c68c596425f05d1bc492d7c03e2979ab669901ed5a57e35e6dd4d6089d27"}
        REST sendBocReturnHash form: {"address":"<validated-friendly-address>","codeHash":"b993c68c596425f05d1bc492d7c03e2979ab669901ed5a57e35e6dd4d6089d27"}
        V3 message chunked: {"address":"<validated-friendly-address>","codeHash":"b993c68c596425f05d1bc492d7c03e2979ab669901ed5a57e35e6dd4d6089d27"}
        Acton internal: {"address":"<validated-friendly-address>","codeHash":"b993c68c596425f05d1bc492d7c03e2979ab669901ed5a57e35e6dd4d6089d27"}
        JSON-RPC root sendBoc: {"address":"<validated-friendly-address>","codeHash":"b993c68c596425f05d1bc492d7c03e2979ab669901ed5a57e35e6dd4d6089d27"}
        JSON-RPC canonical sendBocReturnHash: {"address":"<validated-friendly-address>","codeHash":"b993c68c596425f05d1bc492d7c03e2979ab669901ed5a57e35e6dd4d6089d27"}
        JSON-RPC nested sendBoc: {"address":"<validated-friendly-address>","codeHash":"b993c68c596425f05d1bc492d7c03e2979ab669901ed5a57e35e6dd4d6089d27"}
    "#]]
    .assert_eq(&actual);
}

#[tokio::test]
async fn successful_http_json_rpc_error_does_not_record_deployment() {
    let upstream = spawn_deployment_mock_environment_api().await;
    let project = tempfile::tempdir().expect("test project must be created");
    let app = router(
        localnet_environment(&upstream.base_url),
        ContractRegistryStore::for_project(project.path()),
    );
    let response = app
        .oneshot(
            Request::post("/api/v1/environments/localnet-1/rpc/api/v2/jsonRPC?reject=error")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "jsonrpc": "2.0",
                        "id": 1,
                        "method": "sendBoc",
                        "params": {"boc": DEPLOYMENT_BOC},
                    })
                    .to_string(),
                ))
                .expect("rejected deployment request must be valid"),
        )
        .await
        .expect("rejected deployment request must proxy");
    let snapshot = raw_response_snapshot(response).await;

    assert_eq!(stored_deployment_candidate(&project), None);
    expect![[r#"
        status: 200 OK
        body:
        {"error":{"code":-32000,"message":"rejected"}}"#]]
    .assert_eq(&snapshot);
}
