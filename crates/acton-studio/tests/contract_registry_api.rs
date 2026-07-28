use std::sync::{Arc, Mutex};

use acton_studio::{
    ContractRegistryStore, CreateEnvironmentRequest, EnvironmentConfig, EnvironmentEndpoints,
    EnvironmentRuntime, EnvironmentRuntimeError, EnvironmentRuntimeFuture, EnvironmentStatus,
    StudioEnvironment, StudioServer, StudioServerConfig, UpdateEnvironmentRequest,
};
use axum::body::{Body, to_bytes};
use axum::extract::State;
use axum::http::{Request, Response, StatusCode};
use axum::routing::{any, get};
use axum::{Json, Router};
use expect_test::expect;
use serde_json::{Value, json};
use tower::ServiceExt;

const ENVIRONMENT_ID: &str = "full-ton-1";
const CONTRACT_ADDRESS: &str = "EQC8g9REpyFH8-occ0FD8DnFcjPxjVRjk2u_ESCFnMhmXo6z";

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
            validators: 1,
        },
        EnvironmentEndpoints {
            api_v2: Some(format!("{base_url}/api/v2")),
            api_v3: Some(format!("{base_url}/api/v3")),
            control: None,
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

async fn auto_discovery_snapshot(
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
async fn full_ton_query_request_discovers_contract_and_persists_it() {
    let upstream = spawn_mock_environment_api().await;
    let project = tempfile::tempdir().expect("test project must be created");
    let environment = full_ton_environment(&upstream.base_url);
    let discovery_request = Request::get(format!(
        "/api/v1/environments/{ENVIRONMENT_ID}/rpc/api/v2/getAddressInformation?address={CONTRACT_ADDRESS}"
    ))
    .body(Body::empty())
    .expect("query discovery request must be valid");
    let actual = auto_discovery_snapshot(&upstream, &project, environment, discovery_request).await;

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
          "result": [
            {
              "address": "kQC8g9REpyFH8-occ0FD8DnFcjPxjVRjk2u_ESCFnMhmXjU5",
              "codeHash": "1111111111111111111111111111111111111111111111111111111111111111",
              "sourceKind": "network",
              "status": "active"
            }
          ]
        }

        LIST AFTER STORE RECREATION
        status: 200 OK
        body:
        {
          "ok": true,
          "result": [
            {
              "address": "kQC8g9REpyFH8-occ0FD8DnFcjPxjVRjk2u_ESCFnMhmXjU5",
              "codeHash": "1111111111111111111111111111111111111111111111111111111111111111",
              "sourceKind": "network",
              "status": "active"
            }
          ]
        }

        UPSTREAM REQUESTS
        GET /api/v2/getAddressInformation?address=EQC8g9REpyFH8-occ0FD8DnFcjPxjVRjk2u_ESCFnMhmXo6z
        GET /api/v3/accountStates?address=0%3Abc83d444a72147f3ea1c734143f039c57233f18d5463936bbf1120859cc8665e&include_boc=false
        GET /api/v3/accountStates?address=0%3Abc83d444a72147f3ea1c734143f039c57233f18d5463936bbf1120859cc8665e&include_boc=false"#]]
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
async fn localnet_json_body_request_discovers_contract_and_persists_it() {
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
    let actual = auto_discovery_snapshot(&upstream, &project, environment, discovery_request).await;

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
          "result": [
            {
              "address": "kQC8g9REpyFH8-occ0FD8DnFcjPxjVRjk2u_ESCFnMhmXjU5",
              "codeHash": "1111111111111111111111111111111111111111111111111111111111111111",
              "sourceKind": "local",
              "status": "active"
            }
          ]
        }

        LIST AFTER STORE RECREATION
        status: 200 OK
        body:
        {
          "ok": true,
          "result": [
            {
              "address": "kQC8g9REpyFH8-occ0FD8DnFcjPxjVRjk2u_ESCFnMhmXjU5",
              "codeHash": "1111111111111111111111111111111111111111111111111111111111111111",
              "sourceKind": "local",
              "status": "active"
            }
          ]
        }

        UPSTREAM REQUESTS
        POST /api/v2/runGetMethod
        GET /api/v3/accountStates?address=0%3Abc83d444a72147f3ea1c734143f039c57233f18d5463936bbf1120859cc8665e&include_boc=false
        GET /api/v3/accountStates?address=0%3Abc83d444a72147f3ea1c734143f039c57233f18d5463936bbf1120859cc8665e&include_boc=false"#]]
    .assert_eq(&actual);
}
