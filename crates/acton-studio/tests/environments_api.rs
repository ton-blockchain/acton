use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use acton_studio::{
    CreateEnvironmentConfig, CreateEnvironmentRequest, EnvironmentConfig, EnvironmentEndpoints,
    EnvironmentRuntime, EnvironmentRuntimeError, EnvironmentRuntimeFuture, EnvironmentStatus,
    STUDIO_ENVIRONMENTS_PATH, StudioEnvironment, StudioServer, StudioServerConfig,
    UpdateEnvironmentRequest,
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
            let (config, runtime_endpoints) = match request.config {
                CreateEnvironmentConfig::ActonLocalnet {
                    port,
                    fork_network,
                    fork_block_number,
                    accounts,
                    rate_limit,
                    response_delay_ms,
                    block_interval_ms,
                    no_mining,
                    mine_empty_blocks,
                } => {
                    let port = port.unwrap_or(5411);
                    (
                        EnvironmentConfig::ActonLocalnet {
                            port,
                            fork_network,
                            fork_block_number,
                            accounts,
                            rate_limit,
                            response_delay_ms,
                            block_interval_ms,
                            no_mining,
                            mine_empty_blocks,
                        },
                        EnvironmentEndpoints {
                            api_v2: Some(format!("http://127.0.0.1:{port}/api/v2")),
                            api_v3: Some(format!("http://127.0.0.1:{port}/api/v3")),
                            control: Some(format!("http://127.0.0.1:{port}")),
                        },
                    )
                }
                CreateEnvironmentConfig::FullTonNetwork {
                    api_v2_port,
                    api_v3_port,
                    validators,
                } => {
                    let api_v2_port = api_v2_port.unwrap_or(18080);
                    let api_v3_port = api_v3_port.unwrap_or(18081);
                    (
                        EnvironmentConfig::FullTonNetwork {
                            api_v2_port,
                            api_v3_port,
                            validators: validators.unwrap_or(1),
                        },
                        EnvironmentEndpoints {
                            api_v2: Some(format!("http://127.0.0.1:{api_v2_port}/api/v2")),
                            api_v3: Some(format!("http://127.0.0.1:{api_v3_port}/api/v3")),
                            control: None,
                        },
                    )
                }
            };
            let environment = StudioEnvironment::new(
                format!(
                    "test-environment-{}",
                    self.next_id.fetch_add(1, Ordering::Relaxed) + 1
                ),
                request.name,
                EnvironmentStatus::Running,
                config,
                runtime_endpoints,
            );
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

    fn update(
        &self,
        environment_id: &str,
        request: UpdateEnvironmentRequest,
    ) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
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
            environment.name = request.name;
            let result = environment.clone();
            drop(environments);
            Ok(result)
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

    fn delete(&self, environment_id: &str) -> EnvironmentRuntimeFuture<'_, ()> {
        let environment_id = environment_id.to_owned();
        Box::pin(async move {
            let mut environments = self
                .environments
                .lock()
                .expect("environment lock must not be poisoned");
            let previous_len = environments.len();
            environments.retain(|environment| environment.id != environment_id);
            if environments.len() == previous_len {
                return Err(EnvironmentRuntimeError::NotFound { environment_id });
            }
            drop(environments);
            Ok(())
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
    let body = String::from_utf8_lossy(&body);
    let separator = if body.is_empty() { "" } else { " " };
    format!("status: {status}\nbody:{separator}{body}")
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
    let body = if body.is_empty() {
        "<empty>".into()
    } else {
        String::from_utf8_lossy(&body)
    };
    (
        StatusCode::ACCEPTED,
        format!(
            "method: {}\nuri: {}\nmarker: {marker}\nbody: {}",
            parts.method, parts.uri, body
        ),
    )
}

#[tokio::test]
async fn public_ton_networks_are_permanent_external_environments() {
    let app = router();
    let list = app
        .clone()
        .oneshot(
            Request::get(STUDIO_ENVIRONMENTS_PATH)
                .body(Body::empty())
                .expect("list request must be valid"),
        )
        .await
        .expect("list request must succeed");
    let get_testnet = app
        .clone()
        .oneshot(
            Request::get("/api/v1/environments/testnet")
                .body(Body::empty())
                .expect("get request must be valid"),
        )
        .await
        .expect("get request must succeed");
    let update = app
        .clone()
        .oneshot(
            Request::patch("/api/v1/environments/testnet")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"Renamed"}"#))
                .expect("update request must be valid"),
        )
        .await
        .expect("update request must succeed");
    let get_mainnet = app
        .clone()
        .oneshot(
            Request::get("/api/v1/environments/mainnet")
                .body(Body::empty())
                .expect("get request must be valid"),
        )
        .await
        .expect("get request must succeed");
    let stop = app
        .clone()
        .oneshot(
            Request::post("/api/v1/environments/mainnet/stop")
                .body(Body::empty())
                .expect("stop request must be valid"),
        )
        .await
        .expect("stop request must succeed");
    let restart = app
        .clone()
        .oneshot(
            Request::post("/api/v1/environments/testnet/restart")
                .body(Body::empty())
                .expect("restart request must be valid"),
        )
        .await
        .expect("restart request must succeed");
    let delete = app
        .oneshot(
            Request::delete("/api/v1/environments/mainnet")
                .body(Body::empty())
                .expect("delete request must be valid"),
        )
        .await
        .expect("delete request must succeed");
    let actual = format!(
        "LIST\n{}\n\nGET TESTNET\n{}\n\nGET MAINNET\n{}\n\nUPDATE TESTNET\n{}\n\nSTOP MAINNET\n{}\n\nRESTART TESTNET\n{}\n\nDELETE MAINNET\n{}",
        response_snapshot(list).await,
        response_snapshot(get_testnet).await,
        response_snapshot(get_mainnet).await,
        response_snapshot(update).await,
        response_snapshot(stop).await,
        response_snapshot(restart).await,
        response_snapshot(delete).await,
    );

    expect![[r#"LIST
status: 200 OK
body: [{"id":"testnet","name":"Testnet","status":"running","lifecycle":"external","rpcUrl":"/api/v1/environments/testnet/rpc","config":{"kind":"remoteTonNetwork","network":"testnet"},"capabilities":["apiV2","apiV3","explorer","integration","wallets","simulator","contracts"],"endpoints":{"apiV2":"/api/v1/environments/testnet/rpc/api/v2","apiV3":"/api/v1/environments/testnet/rpc/api/v3"},"network":{"id":"testnet","label":"Testnet","chainId":-3,"testOnly":true}},{"id":"mainnet","name":"Mainnet","status":"running","lifecycle":"external","rpcUrl":"/api/v1/environments/mainnet/rpc","config":{"kind":"remoteTonNetwork","network":"mainnet"},"capabilities":["apiV2","apiV3","explorer","integration","wallets","simulator","contracts"],"endpoints":{"apiV2":"/api/v1/environments/mainnet/rpc/api/v2","apiV3":"/api/v1/environments/mainnet/rpc/api/v3"},"network":{"id":"mainnet","label":"Mainnet","chainId":-239,"testOnly":false}}]

GET TESTNET
status: 200 OK
body: {"id":"testnet","name":"Testnet","status":"running","lifecycle":"external","rpcUrl":"/api/v1/environments/testnet/rpc","config":{"kind":"remoteTonNetwork","network":"testnet"},"capabilities":["apiV2","apiV3","explorer","integration","wallets","simulator","contracts"],"endpoints":{"apiV2":"/api/v1/environments/testnet/rpc/api/v2","apiV3":"/api/v1/environments/testnet/rpc/api/v3"},"network":{"id":"testnet","label":"Testnet","chainId":-3,"testOnly":true}}

GET MAINNET
status: 200 OK
body: {"id":"mainnet","name":"Mainnet","status":"running","lifecycle":"external","rpcUrl":"/api/v1/environments/mainnet/rpc","config":{"kind":"remoteTonNetwork","network":"mainnet"},"capabilities":["apiV2","apiV3","explorer","integration","wallets","simulator","contracts"],"endpoints":{"apiV2":"/api/v1/environments/mainnet/rpc/api/v2","apiV3":"/api/v1/environments/mainnet/rpc/api/v3"},"network":{"id":"mainnet","label":"Mainnet","chainId":-239,"testOnly":false}}

UPDATE TESTNET
status: 409 Conflict
body: {"error":{"code":"environment_lifecycle_unavailable","message":"Testnet is an external environment and cannot be updated by Studio"}}

STOP MAINNET
status: 409 Conflict
body: {"error":{"code":"environment_lifecycle_unavailable","message":"Mainnet is an external environment and cannot be stopped by Studio"}}

RESTART TESTNET
status: 409 Conflict
body: {"error":{"code":"environment_lifecycle_unavailable","message":"Testnet is an external environment and cannot be restarted by Studio"}}

DELETE MAINNET
status: 409 Conflict
body: {"error":{"code":"environment_lifecycle_unavailable","message":"Mainnet is an external environment and cannot be deleted by Studio"}}"#]]
    .assert_eq(&actual);
}

#[tokio::test]
async fn remote_ton_networks_cannot_be_created() {
    let response = router()
        .oneshot(
            Request::post(STUDIO_ENVIRONMENTS_PATH)
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{
                        "name":"Another testnet",
                        "config":{
                            "kind":"remoteTonNetwork",
                            "network":"testnet"
                        }
                    }"#,
                ))
                .expect("create request must be valid"),
        )
        .await
        .expect("create request must succeed");
    let actual = response_snapshot(response).await;

    expect![[r"
        status: 422 Unprocessable Entity
        body: Failed to deserialize the JSON body into the target type: config.kind: unknown variant `remoteTonNetwork`, expected `actonLocalnet` or `fullTonNetwork` at line 4 column 53"]]
    .assert_eq(&actual);
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
                        "config":{
                            "kind":"actonLocalnet",
                            "port":5511,
                            "forkNetwork":"mainnet",
                            "forkBlockNumber":81973221,
                            "accounts":["deployer","treasury"],
                            "rateLimit":30,
                            "responseDelayMs":120,
                            "blockIntervalMs":750,
                            "mineEmptyBlocks":true
                        }
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
    let update = app
        .clone()
        .oneshot(
            Request::patch("/api/v1/environments/test-environment-1")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"Renamed environment"}"#))
                .expect("update request must be valid"),
        )
        .await
        .expect("update request must succeed");
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
        .clone()
        .oneshot(
            Request::post("/api/v1/environments/test-environment-1/restart")
                .body(Body::empty())
                .expect("restart request must be valid"),
        )
        .await
        .expect("restart request must succeed");
    let delete = app
        .clone()
        .oneshot(
            Request::delete("/api/v1/environments/test-environment-1")
                .body(Body::empty())
                .expect("delete request must be valid"),
        )
        .await
        .expect("delete request must succeed");
    let list_after_delete = app
        .oneshot(
            Request::get(STUDIO_ENVIRONMENTS_PATH)
                .body(Body::empty())
                .expect("list request must be valid"),
        )
        .await
        .expect("list request after delete must succeed");
    let actual = format!(
        "CREATE\n{}\n\nLIST\n{}\n\nUPDATE\n{}\n\nSTOP\n{}\n\nRESTART\n{}\n\nDELETE\n{}\n\nLIST AFTER DELETE\n{}",
        response_snapshot(create).await,
        response_snapshot(list).await,
        response_snapshot(update).await,
        response_snapshot(stop).await,
        response_snapshot(restart).await,
        response_snapshot(delete).await,
        response_snapshot(list_after_delete).await
    );

    expect![[r#"
        CREATE
        status: 201 Created
        body: {"id":"test-environment-1","name":"Forked mainnet","status":"running","lifecycle":"managed","rpcUrl":"/api/v1/environments/test-environment-1/rpc","config":{"kind":"actonLocalnet","port":5511,"forkNetwork":"mainnet","forkBlockNumber":81973221,"accounts":["deployer","treasury"],"rateLimit":30,"responseDelayMs":120,"blockIntervalMs":750,"noMining":false,"mineEmptyBlocks":true},"capabilities":["apiV2","apiV3","controlApi","explorer","integration","gramFaucet","jettonFaucet","wallets","simulator","contracts","apiCalls","mining","timeTravel","snapshots","checkpoints"],"endpoints":{"apiV2":"/api/v1/environments/test-environment-1/rpc/api/v2","apiV3":"/api/v1/environments/test-environment-1/rpc/api/v3","control":"/api/v1/environments/test-environment-1/rpc"},"network":{"id":"mainnet","label":"Mainnet fork","chainId":-3,"testOnly":true}}

        LIST
        status: 200 OK
        body: [{"id":"testnet","name":"Testnet","status":"running","lifecycle":"external","rpcUrl":"/api/v1/environments/testnet/rpc","config":{"kind":"remoteTonNetwork","network":"testnet"},"capabilities":["apiV2","apiV3","explorer","integration","wallets","simulator","contracts"],"endpoints":{"apiV2":"/api/v1/environments/testnet/rpc/api/v2","apiV3":"/api/v1/environments/testnet/rpc/api/v3"},"network":{"id":"testnet","label":"Testnet","chainId":-3,"testOnly":true}},{"id":"mainnet","name":"Mainnet","status":"running","lifecycle":"external","rpcUrl":"/api/v1/environments/mainnet/rpc","config":{"kind":"remoteTonNetwork","network":"mainnet"},"capabilities":["apiV2","apiV3","explorer","integration","wallets","simulator","contracts"],"endpoints":{"apiV2":"/api/v1/environments/mainnet/rpc/api/v2","apiV3":"/api/v1/environments/mainnet/rpc/api/v3"},"network":{"id":"mainnet","label":"Mainnet","chainId":-239,"testOnly":false}},{"id":"test-environment-1","name":"Forked mainnet","status":"running","lifecycle":"managed","rpcUrl":"/api/v1/environments/test-environment-1/rpc","config":{"kind":"actonLocalnet","port":5511,"forkNetwork":"mainnet","forkBlockNumber":81973221,"accounts":["deployer","treasury"],"rateLimit":30,"responseDelayMs":120,"blockIntervalMs":750,"noMining":false,"mineEmptyBlocks":true},"capabilities":["apiV2","apiV3","controlApi","explorer","integration","gramFaucet","jettonFaucet","wallets","simulator","contracts","apiCalls","mining","timeTravel","snapshots","checkpoints"],"endpoints":{"apiV2":"/api/v1/environments/test-environment-1/rpc/api/v2","apiV3":"/api/v1/environments/test-environment-1/rpc/api/v3","control":"/api/v1/environments/test-environment-1/rpc"},"network":{"id":"mainnet","label":"Mainnet fork","chainId":-3,"testOnly":true}}]

        UPDATE
        status: 200 OK
        body: {"id":"test-environment-1","name":"Renamed environment","status":"running","lifecycle":"managed","rpcUrl":"/api/v1/environments/test-environment-1/rpc","config":{"kind":"actonLocalnet","port":5511,"forkNetwork":"mainnet","forkBlockNumber":81973221,"accounts":["deployer","treasury"],"rateLimit":30,"responseDelayMs":120,"blockIntervalMs":750,"noMining":false,"mineEmptyBlocks":true},"capabilities":["apiV2","apiV3","controlApi","explorer","integration","gramFaucet","jettonFaucet","wallets","simulator","contracts","apiCalls","mining","timeTravel","snapshots","checkpoints"],"endpoints":{"apiV2":"/api/v1/environments/test-environment-1/rpc/api/v2","apiV3":"/api/v1/environments/test-environment-1/rpc/api/v3","control":"/api/v1/environments/test-environment-1/rpc"},"network":{"id":"mainnet","label":"Mainnet fork","chainId":-3,"testOnly":true}}

        STOP
        status: 200 OK
        body: {"id":"test-environment-1","name":"Renamed environment","status":"stopped","lifecycle":"managed","rpcUrl":"/api/v1/environments/test-environment-1/rpc","config":{"kind":"actonLocalnet","port":5511,"forkNetwork":"mainnet","forkBlockNumber":81973221,"accounts":["deployer","treasury"],"rateLimit":30,"responseDelayMs":120,"blockIntervalMs":750,"noMining":false,"mineEmptyBlocks":true},"capabilities":["apiV2","apiV3","controlApi","explorer","integration","gramFaucet","jettonFaucet","wallets","simulator","contracts","apiCalls","mining","timeTravel","snapshots","checkpoints"],"endpoints":{"apiV2":"/api/v1/environments/test-environment-1/rpc/api/v2","apiV3":"/api/v1/environments/test-environment-1/rpc/api/v3","control":"/api/v1/environments/test-environment-1/rpc"},"network":{"id":"mainnet","label":"Mainnet fork","chainId":-3,"testOnly":true}}

        RESTART
        status: 200 OK
        body: {"id":"test-environment-1","name":"Renamed environment","status":"starting","lifecycle":"managed","rpcUrl":"/api/v1/environments/test-environment-1/rpc","config":{"kind":"actonLocalnet","port":5511,"forkNetwork":"mainnet","forkBlockNumber":81973221,"accounts":["deployer","treasury"],"rateLimit":30,"responseDelayMs":120,"blockIntervalMs":750,"noMining":false,"mineEmptyBlocks":true},"capabilities":["apiV2","apiV3","controlApi","explorer","integration","gramFaucet","jettonFaucet","wallets","simulator","contracts","apiCalls","mining","timeTravel","snapshots","checkpoints"],"endpoints":{"apiV2":"/api/v1/environments/test-environment-1/rpc/api/v2","apiV3":"/api/v1/environments/test-environment-1/rpc/api/v3","control":"/api/v1/environments/test-environment-1/rpc"},"network":{"id":"mainnet","label":"Mainnet fork","chainId":-3,"testOnly":true}}

        DELETE
        status: 204 No Content
        body:

        LIST AFTER DELETE
        status: 200 OK
        body: [{"id":"testnet","name":"Testnet","status":"running","lifecycle":"external","rpcUrl":"/api/v1/environments/testnet/rpc","config":{"kind":"remoteTonNetwork","network":"testnet"},"capabilities":["apiV2","apiV3","explorer","integration","wallets","simulator","contracts"],"endpoints":{"apiV2":"/api/v1/environments/testnet/rpc/api/v2","apiV3":"/api/v1/environments/testnet/rpc/api/v3"},"network":{"id":"testnet","label":"Testnet","chainId":-3,"testOnly":true}},{"id":"mainnet","name":"Mainnet","status":"running","lifecycle":"external","rpcUrl":"/api/v1/environments/mainnet/rpc","config":{"kind":"remoteTonNetwork","network":"mainnet"},"capabilities":["apiV2","apiV3","explorer","integration","wallets","simulator","contracts"],"endpoints":{"apiV2":"/api/v1/environments/mainnet/rpc/api/v2","apiV3":"/api/v1/environments/mainnet/rpc/api/v3"},"network":{"id":"mainnet","label":"Mainnet","chainId":-239,"testOnly":false}}]"#]]
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
                    r#"{{"name":"Proxy target","config":{{"kind":"actonLocalnet","port":{port}}}}}"#
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
async fn full_ton_environment_advertises_only_its_supported_surface() {
    let response = router()
        .oneshot(
            Request::post(STUDIO_ENVIRONMENTS_PATH)
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{
                        "name":"Protocol network",
                        "config":{
                            "kind":"fullTonNetwork",
                            "apiV2Port":18180,
                            "apiV3Port":18181,
                            "validators":3
                        }
                    }"#,
                ))
                .expect("create request must be valid"),
        )
        .await
        .expect("create request must succeed");
    let actual = response_snapshot(response).await;

    expect![[r#"
        status: 201 Created
        body: {"id":"test-environment-1","name":"Protocol network","status":"running","lifecycle":"managed","rpcUrl":"/api/v1/environments/test-environment-1/rpc","config":{"kind":"fullTonNetwork","apiV2Port":18180,"apiV3Port":18181,"validators":3},"capabilities":["apiV2","apiV3","explorer","integration","gramFaucet","wallets","simulator","contracts"],"endpoints":{"apiV2":"/api/v1/environments/test-environment-1/rpc/api/v2","apiV3":"/api/v1/environments/test-environment-1/rpc/api/v3"},"network":{"id":"full-ton-network","label":"Local TON network","chainId":-239,"testOnly":true}}"#]]
    .assert_eq(&actual);
}

#[tokio::test]
async fn full_ton_environment_routes_v2_and_v3_to_separate_upstreams() {
    let v2_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("V2 proxy test listener must bind");
    let v2_port = v2_listener
        .local_addr()
        .expect("V2 listener must have an address")
        .port();
    let v2_upstream = tokio::spawn(async move {
        axum::serve(
            v2_listener,
            Router::new()
                .fallback(any(proxy_target))
                .into_make_service(),
        )
        .await
        .expect("V2 proxy target must serve");
    });
    let v3_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("V3 proxy test listener must bind");
    let v3_port = v3_listener
        .local_addr()
        .expect("V3 listener must have an address")
        .port();
    let v3_upstream = tokio::spawn(async move {
        axum::serve(
            v3_listener,
            Router::new()
                .fallback(any(proxy_target))
                .into_make_service(),
        )
        .await
        .expect("V3 proxy target must serve");
    });

    let app = router();
    app.clone()
        .oneshot(
            Request::post(STUDIO_ENVIRONMENTS_PATH)
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{
                        "name":"Protocol network",
                        "config":{{
                            "kind":"fullTonNetwork",
                            "apiV2Port":{v2_port},
                            "apiV3Port":{v3_port}
                        }}
                    }}"#
                )))
                .expect("create request must be valid"),
        )
        .await
        .expect("create request must succeed");
    let v2_root_response = app
        .clone()
        .oneshot(
            Request::get("/api/v1/environments/test-environment-1/rpc/api/v2")
                .body(Body::empty())
                .expect("V2 root proxy request must be valid"),
        )
        .await
        .expect("V2 root proxy request must succeed");
    let v2_response = app
        .clone()
        .oneshot(
            Request::get("/api/v1/environments/test-environment-1/rpc/api/v2/getMasterchainInfo")
                .body(Body::empty())
                .expect("V2 proxy request must be valid"),
        )
        .await
        .expect("V2 proxy request must succeed");
    let v3_response = app
        .clone()
        .oneshot(
            Request::get("/api/v1/environments/test-environment-1/rpc/api/v3/transactions?limit=1")
                .body(Body::empty())
                .expect("V3 proxy request must be valid"),
        )
        .await
        .expect("V3 proxy request must succeed");
    let faucet_response = app
        .clone()
        .oneshot(
            Request::post("/api/v1/environments/test-environment-1/rpc/acton_fundAccount")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"address":"test","amount":100}"#))
                .expect("faucet proxy request must be valid"),
        )
        .await
        .expect("faucet proxy request must succeed");
    let unsupported_control_response = app
        .oneshot(
            Request::get("/api/v1/environments/test-environment-1/rpc/status")
                .body(Body::empty())
                .expect("unsupported control proxy request must be valid"),
        )
        .await
        .expect("unsupported control proxy request must succeed");
    let actual = format!(
        "V2 ROOT\n{}\n\nV2\n{}\n\nV3\n{}\n\nFAUCET\n{}\n\nUNSUPPORTED CONTROL\n{}",
        response_snapshot(v2_root_response).await,
        response_snapshot(v2_response).await,
        response_snapshot(v3_response).await,
        response_snapshot(faucet_response).await,
        response_snapshot(unsupported_control_response).await,
    );
    v2_upstream.abort();
    v3_upstream.abort();

    expect![[r#"V2 ROOT
status: 202 Accepted
body: method: GET
uri: /api/v2
marker: missing
body: <empty>

V2
status: 202 Accepted
body: method: GET
uri: /api/v2/getMasterchainInfo
marker: missing
body: <empty>

V3
status: 202 Accepted
body: method: GET
uri: /api/v3/transactions?limit=1
marker: missing
body: <empty>

FAUCET
status: 202 Accepted
body: method: POST
uri: /acton_fundAccount
marker: missing
body: {"address":"test","amount":100}

UNSUPPORTED CONTROL
status: 409 Conflict
body: {"error":{"code":"environment_endpoint_unavailable","message":"This endpoint is not available in Protocol network"}}"#]]
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
