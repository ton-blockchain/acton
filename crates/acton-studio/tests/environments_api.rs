use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use acton_studio::{
    CreateEnvironmentRequest, EnvironmentConfig, EnvironmentRuntime, EnvironmentRuntimeError,
    EnvironmentRuntimeFuture, EnvironmentStatus, STUDIO_ENVIRONMENTS_PATH, StudioEnvironment,
    StudioServer, StudioServerConfig,
};
use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, Response, StatusCode};
use axum::routing::any;
use expect_test::expect;
use tower::ServiceExt;

#[derive(Default)]
struct TestEnvironmentRuntime {
    next_id: AtomicU64,
    environments: Mutex<Vec<StudioEnvironment>>,
}

impl EnvironmentRuntime for TestEnvironmentRuntime {
    fn list(&self) -> EnvironmentRuntimeFuture<'_, Vec<StudioEnvironment>> {
        Box::pin(async {
            Ok(self
                .environments
                .lock()
                .expect("environment lock must not be poisoned")
                .clone())
        })
    }

    fn create(
        &self,
        request: CreateEnvironmentRequest,
    ) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        Box::pin(async move {
            let port = request.port.unwrap_or(5411);
            let environment = StudioEnvironment {
                id: format!(
                    "test-environment-{}",
                    self.next_id.fetch_add(1, Ordering::Relaxed) + 1
                ),
                name: request.name,
                status: EnvironmentStatus::Running,
                rpc_url: format!("http://127.0.0.1:{port}"),
                config: EnvironmentConfig {
                    port,
                    fork_network: request.fork_network,
                    fork_block_number: request.fork_block_number,
                    accounts: request.accounts,
                    rate_limit: request.rate_limit,
                    response_delay_ms: request.response_delay_ms,
                    block_interval_ms: request.block_interval_ms,
                    no_mining: request.no_mining,
                    mine_empty_blocks: request.mine_empty_blocks,
                },
                error: None,
            };
            self.environments
                .lock()
                .expect("environment lock must not be poisoned")
                .push(environment.clone());
            Ok(environment)
        })
    }

    fn get(&self, environment_id: &str) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        let environment_id = environment_id.to_owned();
        Box::pin(async move {
            self.environments
                .lock()
                .expect("environment lock must not be poisoned")
                .iter()
                .find(|environment| environment.id == environment_id)
                .cloned()
                .ok_or(EnvironmentRuntimeError::NotFound { environment_id })
        })
    }

    fn stop(&self, environment_id: &str) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        let environment_id = environment_id.to_owned();
        Box::pin(async move {
            let mut environments = self
                .environments
                .lock()
                .expect("environment lock must not be poisoned");
            let environment = environments
                .iter_mut()
                .find(|environment| environment.id == environment_id)
                .ok_or_else(|| EnvironmentRuntimeError::NotFound {
                    environment_id: environment_id.clone(),
                })?;
            environment.status = EnvironmentStatus::Stopped;
            let result = environment.clone();
            drop(environments);
            Ok(result)
        })
    }

    fn restart(&self, environment_id: &str) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        let environment_id = environment_id.to_owned();
        Box::pin(async move {
            let mut environments = self
                .environments
                .lock()
                .expect("environment lock must not be poisoned");
            let environment = environments
                .iter_mut()
                .find(|environment| environment.id == environment_id)
                .ok_or_else(|| EnvironmentRuntimeError::NotFound {
                    environment_id: environment_id.clone(),
                })?;
            environment.status = EnvironmentStatus::Starting;
            let result = environment.clone();
            drop(environments);
            Ok(result)
        })
    }
}

fn router() -> Router {
    StudioServer::new(StudioServerConfig::new("test-version"))
        .with_environment_runtime(TestEnvironmentRuntime::default())
        .router()
}

async fn response_snapshot(response: Response<Body>) -> String {
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body must be readable");
    format!("status: {status}\nbody: {}", String::from_utf8_lossy(&body))
}

async fn proxy_target(request: Request<Body>) -> (StatusCode, String) {
    let (parts, body) = request.into_parts();
    let body = to_bytes(body, usize::MAX)
        .await
        .expect("proxied request body must be readable");
    let marker = parts
        .headers
        .get("x-test-marker")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("missing");
    (
        StatusCode::ACCEPTED,
        format!(
            "method: {}\nuri: {}\nmarker: {marker}\nbody: {}",
            parts.method,
            parts.uri,
            String::from_utf8_lossy(&body)
        ),
    )
}

#[tokio::test]
async fn environment_create_list_stop_and_restart_share_one_api_contract() {
    let app = router();
    let create = app
        .clone()
        .oneshot(
            Request::post(STUDIO_ENVIRONMENTS_PATH)
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{
                        "name":"Forked mainnet",
                        "port":5511,
                        "forkNetwork":"mainnet",
                        "forkBlockNumber":81973221,
                        "accounts":["deployer","treasury"],
                        "rateLimit":30,
                        "responseDelayMs":120,
                        "blockIntervalMs":750,
                        "mineEmptyBlocks":true
                    }"#,
                ))
                .expect("create request must be valid"),
        )
        .await
        .expect("create request must succeed");
    let list = app
        .clone()
        .oneshot(
            Request::get(STUDIO_ENVIRONMENTS_PATH)
                .body(Body::empty())
                .expect("list request must be valid"),
        )
        .await
        .expect("list request must succeed");
    let stop = app
        .clone()
        .oneshot(
            Request::post("/api/v1/environments/test-environment-1/stop")
                .body(Body::empty())
                .expect("stop request must be valid"),
        )
        .await
        .expect("stop request must succeed");
    let restart = app
        .oneshot(
            Request::post("/api/v1/environments/test-environment-1/restart")
                .body(Body::empty())
                .expect("restart request must be valid"),
        )
        .await
        .expect("restart request must succeed");
    let actual = format!(
        "CREATE\n{}\n\nLIST\n{}\n\nSTOP\n{}\n\nRESTART\n{}",
        response_snapshot(create).await,
        response_snapshot(list).await,
        response_snapshot(stop).await,
        response_snapshot(restart).await
    );

    expect![[r#"CREATE
status: 201 Created
body: {"id":"test-environment-1","name":"Forked mainnet","status":"running","rpcUrl":"/api/v1/environments/test-environment-1/rpc","config":{"port":5511,"forkNetwork":"mainnet","forkBlockNumber":81973221,"accounts":["deployer","treasury"],"rateLimit":30,"responseDelayMs":120,"blockIntervalMs":750,"noMining":false,"mineEmptyBlocks":true}}

LIST
status: 200 OK
body: [{"id":"test-environment-1","name":"Forked mainnet","status":"running","rpcUrl":"/api/v1/environments/test-environment-1/rpc","config":{"port":5511,"forkNetwork":"mainnet","forkBlockNumber":81973221,"accounts":["deployer","treasury"],"rateLimit":30,"responseDelayMs":120,"blockIntervalMs":750,"noMining":false,"mineEmptyBlocks":true}}]

STOP
status: 200 OK
body: {"id":"test-environment-1","name":"Forked mainnet","status":"stopped","rpcUrl":"/api/v1/environments/test-environment-1/rpc","config":{"port":5511,"forkNetwork":"mainnet","forkBlockNumber":81973221,"accounts":["deployer","treasury"],"rateLimit":30,"responseDelayMs":120,"blockIntervalMs":750,"noMining":false,"mineEmptyBlocks":true}}

RESTART
status: 200 OK
body: {"id":"test-environment-1","name":"Forked mainnet","status":"starting","rpcUrl":"/api/v1/environments/test-environment-1/rpc","config":{"port":5511,"forkNetwork":"mainnet","forkBlockNumber":81973221,"accounts":["deployer","treasury"],"rateLimit":30,"responseDelayMs":120,"blockIntervalMs":750,"noMining":false,"mineEmptyBlocks":true}}"#]]
    .assert_eq(&actual);
}

#[tokio::test]
async fn environment_rpc_is_proxied_through_the_studio_origin() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("proxy test listener must bind");
    let port = listener
        .local_addr()
        .expect("proxy test listener must have an address")
        .port();
    let upstream = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new()
                .fallback(any(proxy_target))
                .into_make_service(),
        )
        .await
        .expect("proxy target must serve");
    });

    let app = router();
    app.clone()
        .oneshot(
            Request::post(STUDIO_ENVIRONMENTS_PATH)
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"name":"Proxy target","port":{port}}}"#
                )))
                .expect("create request must be valid"),
        )
        .await
        .expect("create request must succeed");
    let response = app
        .oneshot(
            Request::post(
                "/api/v1/environments/test-environment-1/rpc/api/v3/transactions?limit=2",
            )
            .header("content-type", "application/json")
            .header("x-test-marker", "forwarded")
            .body(Body::from(r#"{"account":"test"}"#))
            .expect("proxy request must be valid"),
        )
        .await
        .expect("proxy request must succeed");
    let actual = response_snapshot(response).await;
    upstream.abort();

    expect![[r#"status: 202 Accepted
body: method: POST
uri: /api/v3/transactions?limit=2
marker: forwarded
body: {"account":"test"}"#]]
    .assert_eq(&actual);
}

#[tokio::test]
async fn unknown_environment_uses_a_structured_not_found_error() {
    let response = router()
        .oneshot(
            Request::post("/api/v1/environments/missing/stop")
                .body(Body::empty())
                .expect("stop request must be valid"),
        )
        .await
        .expect("stop request must succeed");
    let actual = response_snapshot(response).await;

    expect![[r#"status: 404 Not Found
body: {"error":{"code":"environment_not_found","message":"Environment missing was not found"}}"#]]
    .assert_eq(&actual);
}
