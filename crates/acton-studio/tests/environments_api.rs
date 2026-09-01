use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use acton_studio::{
    CreateEnvironmentConfig, CreateEnvironmentRequest, CreateEnvironmentSnapshotRequest,
    EnvironmentConfig, EnvironmentEndpoints, EnvironmentRuntime, EnvironmentRuntimeError,
    EnvironmentRuntimeFuture, EnvironmentSnapshot, EnvironmentSnapshotOperation,
    EnvironmentSnapshotOperationKind, EnvironmentSnapshotOperationPhase, EnvironmentStatus,
    STUDIO_ENVIRONMENTS_PATH, StudioEnvironment, StudioServer, StudioServerConfig,
    UpdateEnvironmentRequest,
};
use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, Response, StatusCode};
use axum::response::Json;
use axum::routing::any;
use expect_test::expect;
use serde_json::{Value, json};
use tower::ServiceExt;

#[derive(Default)]
struct TestEnvironmentRuntime {
    next_id: AtomicU64,
    environments: Mutex<Vec<StudioEnvironment>>,
    snapshots: Mutex<Vec<EnvironmentSnapshot>>,
    snapshot_operation: Mutex<Option<EnvironmentSnapshotOperation>>,
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
                            config: None,
                            control: Some(format!("http://127.0.0.1:{port}")),
                        },
                    )
                }
                CreateEnvironmentConfig::FullTonNetwork {
                    api_v2_port,
                    api_v3_port,
                    admin_port,
                    config_port,
                    imported_accounts,
                } => {
                    let api_v2_port = api_v2_port.unwrap_or(18080);
                    let api_v3_port = api_v3_port.unwrap_or(18081);
                    let admin_port = admin_port.unwrap_or(18082);
                    let config_port = config_port.unwrap_or(18083);
                    (
                        EnvironmentConfig::FullTonNetwork {
                            api_v2_port,
                            api_v3_port,
                            admin_port,
                            config_port,
                            imported_accounts,
                        },
                        EnvironmentEndpoints {
                            api_v2: Some(format!("http://127.0.0.1:{api_v2_port}/api/v2")),
                            api_v3: Some(format!("http://127.0.0.1:{api_v3_port}/api/v3")),
                            config: Some(format!("http://127.0.0.1:{config_port}")),
                            control: Some(format!("http://127.0.0.1:{admin_port}")),
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

    fn list_snapshots(
        &self,
        _environment_id: &str,
    ) -> EnvironmentRuntimeFuture<'_, Vec<EnvironmentSnapshot>> {
        Box::pin(async {
            Ok(self
                .snapshots
                .lock()
                .expect("snapshot lock must not be poisoned")
                .clone())
        })
    }

    fn create_snapshot(
        &self,
        _environment_id: &str,
        request: CreateEnvironmentSnapshotRequest,
    ) -> EnvironmentRuntimeFuture<'_, EnvironmentSnapshotOperation> {
        Box::pin(async move {
            let snapshot = snapshot_fixture("snapshot-1", request.name.clone());
            self.snapshots
                .lock()
                .expect("snapshot lock must not be poisoned")
                .push(snapshot);
            let operation = operation_fixture(
                EnvironmentSnapshotOperationKind::Create,
                Some("snapshot-1"),
                request.name,
            );
            *self
                .snapshot_operation
                .lock()
                .expect("snapshot operation lock must not be poisoned") = Some(operation.clone());
            Ok(operation)
        })
    }

    fn restore_snapshot(
        &self,
        _environment_id: &str,
        snapshot_id: &str,
    ) -> EnvironmentRuntimeFuture<'_, EnvironmentSnapshotOperation> {
        let snapshot_id = snapshot_id.to_owned();
        Box::pin(async move {
            let operation = operation_fixture(
                EnvironmentSnapshotOperationKind::Restore,
                Some(&snapshot_id),
                None,
            );
            *self
                .snapshot_operation
                .lock()
                .expect("snapshot operation lock must not be poisoned") = Some(operation.clone());
            Ok(operation)
        })
    }

    fn delete_snapshot(
        &self,
        _environment_id: &str,
        snapshot_id: &str,
    ) -> EnvironmentRuntimeFuture<'_, ()> {
        let snapshot_id = snapshot_id.to_owned();
        Box::pin(async move {
            self.snapshots
                .lock()
                .expect("snapshot lock must not be poisoned")
                .retain(|snapshot| snapshot.id != snapshot_id);
            Ok(())
        })
    }

    fn snapshot_operation(
        &self,
        _environment_id: &str,
    ) -> EnvironmentRuntimeFuture<'_, Option<EnvironmentSnapshotOperation>> {
        Box::pin(async {
            Ok(self
                .snapshot_operation
                .lock()
                .expect("snapshot operation lock must not be poisoned")
                .clone())
        })
    }
}

fn snapshot_fixture(id: &str, name: Option<String>) -> EnvironmentSnapshot {
    EnvironmentSnapshot {
        format_version: 1,
        id: id.to_owned(),
        name,
        created_at: 1_786_000_000,
        archive_size_bytes: 52_000_000,
        state_size_bytes: 125_000_000,
        state_schema_version: 1,
        ton_release: "v2026.06".to_owned(),
        masterchain_seqno: Some(42),
    }
}

fn operation_fixture(
    kind: EnvironmentSnapshotOperationKind,
    snapshot_id: Option<&str>,
    snapshot_name: Option<String>,
) -> EnvironmentSnapshotOperation {
    EnvironmentSnapshotOperation {
        kind,
        phase: EnvironmentSnapshotOperationPhase::Preparing,
        started_at: "2026-08-04T12:00:00Z".to_owned(),
        finished_at: None,
        snapshot_id: snapshot_id.map(ToOwned::to_owned),
        snapshot_name,
        startup_timings: None,
        error: None,
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

async fn api_call_proxy_target(request: Request<Body>) -> Json<Value> {
    let (parts, body) = request.into_parts();
    let body = to_bytes(body, usize::MAX)
        .await
        .expect("proxied API call body must be readable");
    Json(json!({
        "method": parts.method.as_str(),
        "uri": parts.uri.to_string(),
        "requestSource": parts
            .headers
            .get("x-acton-request-source")
            .and_then(|value| value.to_str().ok()),
        "acceptEncoding": parts
            .headers
            .get("accept-encoding")
            .and_then(|value| value.to_str().ok()),
        "body": String::from_utf8_lossy(&body),
    }))
}

#[tokio::test]
async fn snapshot_routes_return_long_running_operation_state() {
    let app = router();
    let create = app
        .clone()
        .oneshot(
            Request::post("/api/v1/environments/environment-1/snapshots")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"Before upgrade"}"#))
                .expect("create snapshot request must be valid"),
        )
        .await
        .expect("create snapshot request must succeed");
    let list = app
        .clone()
        .oneshot(
            Request::get("/api/v1/environments/environment-1/snapshots")
                .body(Body::empty())
                .expect("list snapshots request must be valid"),
        )
        .await
        .expect("list snapshots request must succeed");
    let operation = app
        .clone()
        .oneshot(
            Request::get("/api/v1/environments/environment-1/snapshot-operation")
                .body(Body::empty())
                .expect("snapshot operation request must be valid"),
        )
        .await
        .expect("snapshot operation request must succeed");
    let restore = app
        .clone()
        .oneshot(
            Request::post("/api/v1/environments/environment-1/snapshots/snapshot-1/restore")
                .body(Body::empty())
                .expect("restore snapshot request must be valid"),
        )
        .await
        .expect("restore snapshot request must succeed");
    let delete = app
        .clone()
        .oneshot(
            Request::delete("/api/v1/environments/environment-1/snapshots/snapshot-1")
                .body(Body::empty())
                .expect("delete snapshot request must be valid"),
        )
        .await
        .expect("delete snapshot request must succeed");
    let empty_list = app
        .oneshot(
            Request::get("/api/v1/environments/environment-1/snapshots")
                .body(Body::empty())
                .expect("empty list request must be valid"),
        )
        .await
        .expect("empty list request must succeed");
    let actual = format!(
        "CREATE\n{}\n\nLIST\n{}\n\nOPERATION\n{}\n\nRESTORE\n{}\n\nDELETE\n{}\n\nEMPTY LIST\n{}",
        response_snapshot(create).await,
        response_snapshot(list).await,
        response_snapshot(operation).await,
        response_snapshot(restore).await,
        response_snapshot(delete).await,
        response_snapshot(empty_list).await,
    );

    expect![[r#"CREATE
status: 202 Accepted
body: {"kind":"create","phase":"preparing","startedAt":"2026-08-04T12:00:00Z","snapshotId":"snapshot-1","snapshotName":"Before upgrade"}

LIST
status: 200 OK
body: [{"formatVersion":1,"id":"snapshot-1","name":"Before upgrade","createdAt":1786000000,"archiveSizeBytes":52000000,"stateSizeBytes":125000000,"stateSchemaVersion":1,"tonRelease":"v2026.06","masterchainSeqno":42}]

OPERATION
status: 200 OK
body: {"kind":"create","phase":"preparing","startedAt":"2026-08-04T12:00:00Z","snapshotId":"snapshot-1","snapshotName":"Before upgrade"}

RESTORE
status: 202 Accepted
body: {"kind":"restore","phase":"preparing","startedAt":"2026-08-04T12:00:00Z","snapshotId":"snapshot-1"}

DELETE
status: 204 No Content
body:

EMPTY LIST
status: 200 OK
body: []"#]]
    .assert_eq(&actual);
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

    expect![[r#"
        LIST
        status: 200 OK
        body: [{"id":"testnet","name":"Testnet","status":"running","lifecycle":"external","rpcUrl":"/api/v1/environments/testnet/rpc","config":{"kind":"remoteTonNetwork","network":"testnet"},"capabilities":["apiV2","apiV3","explorer","integration","wallets","simulator","contracts","apiCalls"],"endpoints":{"apiV2":"/api/v1/environments/testnet/rpc/api/v2","apiV3":"/api/v1/environments/testnet/rpc/api/v3"},"network":{"id":"testnet","label":"Testnet","chainId":-3,"testOnly":true,"supportsActions":true}},{"id":"mainnet","name":"Mainnet","status":"running","lifecycle":"external","rpcUrl":"/api/v1/environments/mainnet/rpc","config":{"kind":"remoteTonNetwork","network":"mainnet"},"capabilities":["apiV2","apiV3","explorer","integration","wallets","simulator","contracts","apiCalls"],"endpoints":{"apiV2":"/api/v1/environments/mainnet/rpc/api/v2","apiV3":"/api/v1/environments/mainnet/rpc/api/v3"},"network":{"id":"mainnet","label":"Mainnet","chainId":-239,"testOnly":false,"supportsActions":true}}]

        GET TESTNET
        status: 200 OK
        body: {"id":"testnet","name":"Testnet","status":"running","lifecycle":"external","rpcUrl":"/api/v1/environments/testnet/rpc","config":{"kind":"remoteTonNetwork","network":"testnet"},"capabilities":["apiV2","apiV3","explorer","integration","wallets","simulator","contracts","apiCalls"],"endpoints":{"apiV2":"/api/v1/environments/testnet/rpc/api/v2","apiV3":"/api/v1/environments/testnet/rpc/api/v3"},"network":{"id":"testnet","label":"Testnet","chainId":-3,"testOnly":true,"supportsActions":true}}

        GET MAINNET
        status: 200 OK
        body: {"id":"mainnet","name":"Mainnet","status":"running","lifecycle":"external","rpcUrl":"/api/v1/environments/mainnet/rpc","config":{"kind":"remoteTonNetwork","network":"mainnet"},"capabilities":["apiV2","apiV3","explorer","integration","wallets","simulator","contracts","apiCalls"],"endpoints":{"apiV2":"/api/v1/environments/mainnet/rpc/api/v2","apiV3":"/api/v1/environments/mainnet/rpc/api/v3"},"network":{"id":"mainnet","label":"Mainnet","chainId":-239,"testOnly":false,"supportsActions":true}}

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
        body: {"id":"test-environment-1","name":"Forked mainnet","status":"running","lifecycle":"managed","rpcUrl":"/api/v1/environments/test-environment-1/rpc","config":{"kind":"actonLocalnet","port":5511,"forkNetwork":"mainnet","forkBlockNumber":81973221,"accounts":["deployer","treasury"],"rateLimit":30,"responseDelayMs":120,"blockIntervalMs":750,"noMining":false,"mineEmptyBlocks":true},"capabilities":["apiV2","apiV3","controlApi","explorer","integration","gramFaucet","jettonFaucet","wallets","simulator","contracts","apiCalls","mining","timeTravel","checkpoints"],"endpoints":{"apiV2":"/api/v1/environments/test-environment-1/rpc/api/v2","apiV3":"/api/v1/environments/test-environment-1/rpc/api/v3","control":"/api/v1/environments/test-environment-1/rpc"},"network":{"id":"mainnet","label":"Mainnet fork","chainId":-3,"testOnly":true,"supportsActions":false}}

        LIST
        status: 200 OK
        body: [{"id":"testnet","name":"Testnet","status":"running","lifecycle":"external","rpcUrl":"/api/v1/environments/testnet/rpc","config":{"kind":"remoteTonNetwork","network":"testnet"},"capabilities":["apiV2","apiV3","explorer","integration","wallets","simulator","contracts","apiCalls"],"endpoints":{"apiV2":"/api/v1/environments/testnet/rpc/api/v2","apiV3":"/api/v1/environments/testnet/rpc/api/v3"},"network":{"id":"testnet","label":"Testnet","chainId":-3,"testOnly":true,"supportsActions":true}},{"id":"mainnet","name":"Mainnet","status":"running","lifecycle":"external","rpcUrl":"/api/v1/environments/mainnet/rpc","config":{"kind":"remoteTonNetwork","network":"mainnet"},"capabilities":["apiV2","apiV3","explorer","integration","wallets","simulator","contracts","apiCalls"],"endpoints":{"apiV2":"/api/v1/environments/mainnet/rpc/api/v2","apiV3":"/api/v1/environments/mainnet/rpc/api/v3"},"network":{"id":"mainnet","label":"Mainnet","chainId":-239,"testOnly":false,"supportsActions":true}},{"id":"test-environment-1","name":"Forked mainnet","status":"running","lifecycle":"managed","rpcUrl":"/api/v1/environments/test-environment-1/rpc","config":{"kind":"actonLocalnet","port":5511,"forkNetwork":"mainnet","forkBlockNumber":81973221,"accounts":["deployer","treasury"],"rateLimit":30,"responseDelayMs":120,"blockIntervalMs":750,"noMining":false,"mineEmptyBlocks":true},"capabilities":["apiV2","apiV3","controlApi","explorer","integration","gramFaucet","jettonFaucet","wallets","simulator","contracts","apiCalls","mining","timeTravel","checkpoints"],"endpoints":{"apiV2":"/api/v1/environments/test-environment-1/rpc/api/v2","apiV3":"/api/v1/environments/test-environment-1/rpc/api/v3","control":"/api/v1/environments/test-environment-1/rpc"},"network":{"id":"mainnet","label":"Mainnet fork","chainId":-3,"testOnly":true,"supportsActions":false}}]

        UPDATE
        status: 200 OK
        body: {"id":"test-environment-1","name":"Renamed environment","status":"running","lifecycle":"managed","rpcUrl":"/api/v1/environments/test-environment-1/rpc","config":{"kind":"actonLocalnet","port":5511,"forkNetwork":"mainnet","forkBlockNumber":81973221,"accounts":["deployer","treasury"],"rateLimit":30,"responseDelayMs":120,"blockIntervalMs":750,"noMining":false,"mineEmptyBlocks":true},"capabilities":["apiV2","apiV3","controlApi","explorer","integration","gramFaucet","jettonFaucet","wallets","simulator","contracts","apiCalls","mining","timeTravel","checkpoints"],"endpoints":{"apiV2":"/api/v1/environments/test-environment-1/rpc/api/v2","apiV3":"/api/v1/environments/test-environment-1/rpc/api/v3","control":"/api/v1/environments/test-environment-1/rpc"},"network":{"id":"mainnet","label":"Mainnet fork","chainId":-3,"testOnly":true,"supportsActions":false}}

        STOP
        status: 200 OK
        body: {"id":"test-environment-1","name":"Renamed environment","status":"stopped","lifecycle":"managed","rpcUrl":"/api/v1/environments/test-environment-1/rpc","config":{"kind":"actonLocalnet","port":5511,"forkNetwork":"mainnet","forkBlockNumber":81973221,"accounts":["deployer","treasury"],"rateLimit":30,"responseDelayMs":120,"blockIntervalMs":750,"noMining":false,"mineEmptyBlocks":true},"capabilities":["apiV2","apiV3","controlApi","explorer","integration","gramFaucet","jettonFaucet","wallets","simulator","contracts","apiCalls","mining","timeTravel","checkpoints"],"endpoints":{"apiV2":"/api/v1/environments/test-environment-1/rpc/api/v2","apiV3":"/api/v1/environments/test-environment-1/rpc/api/v3","control":"/api/v1/environments/test-environment-1/rpc"},"network":{"id":"mainnet","label":"Mainnet fork","chainId":-3,"testOnly":true,"supportsActions":false}}

        RESTART
        status: 200 OK
        body: {"id":"test-environment-1","name":"Renamed environment","status":"starting","lifecycle":"managed","rpcUrl":"/api/v1/environments/test-environment-1/rpc","config":{"kind":"actonLocalnet","port":5511,"forkNetwork":"mainnet","forkBlockNumber":81973221,"accounts":["deployer","treasury"],"rateLimit":30,"responseDelayMs":120,"blockIntervalMs":750,"noMining":false,"mineEmptyBlocks":true},"capabilities":["apiV2","apiV3","controlApi","explorer","integration","gramFaucet","jettonFaucet","wallets","simulator","contracts","apiCalls","mining","timeTravel","checkpoints"],"endpoints":{"apiV2":"/api/v1/environments/test-environment-1/rpc/api/v2","apiV3":"/api/v1/environments/test-environment-1/rpc/api/v3","control":"/api/v1/environments/test-environment-1/rpc"},"network":{"id":"mainnet","label":"Mainnet fork","chainId":-3,"testOnly":true,"supportsActions":false}}

        DELETE
        status: 204 No Content
        body:

        LIST AFTER DELETE
        status: 200 OK
        body: [{"id":"testnet","name":"Testnet","status":"running","lifecycle":"external","rpcUrl":"/api/v1/environments/testnet/rpc","config":{"kind":"remoteTonNetwork","network":"testnet"},"capabilities":["apiV2","apiV3","explorer","integration","wallets","simulator","contracts","apiCalls"],"endpoints":{"apiV2":"/api/v1/environments/testnet/rpc/api/v2","apiV3":"/api/v1/environments/testnet/rpc/api/v3"},"network":{"id":"testnet","label":"Testnet","chainId":-3,"testOnly":true,"supportsActions":true}},{"id":"mainnet","name":"Mainnet","status":"running","lifecycle":"external","rpcUrl":"/api/v1/environments/mainnet/rpc","config":{"kind":"remoteTonNetwork","network":"mainnet"},"capabilities":["apiV2","apiV3","explorer","integration","wallets","simulator","contracts","apiCalls"],"endpoints":{"apiV2":"/api/v1/environments/mainnet/rpc/api/v2","apiV3":"/api/v1/environments/mainnet/rpc/api/v3"},"network":{"id":"mainnet","label":"Mainnet","chainId":-239,"testOnly":false,"supportsActions":true}}]"#]]
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
async fn studio_records_api_calls_per_environment_for_every_proxy_target() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("API call test listener must bind");
    let port = listener
        .local_addr()
        .expect("API call test listener must have an address")
        .port();
    let upstream = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new()
                .fallback(any(api_call_proxy_target))
                .into_make_service(),
        )
        .await
        .expect("API call test server must run");
    });
    let app = router();

    for name in ["First", "Second"] {
        let response = app
            .clone()
            .oneshot(
                Request::post(STUDIO_ENVIRONMENTS_PATH)
                    .header("content-type", "application/json")
                    .body(Body::from(format!(
                        r#"{{"name":"{name}","config":{{"kind":"actonLocalnet","port":{port},"accounts":[],"noMining":false,"mineEmptyBlocks":false}}}}"#
                    )))
                    .expect("environment request must be valid"),
            )
            .await
            .expect("environment request must succeed");
        let _ = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("environment response must be readable");
    }

    let external = app
        .clone()
        .oneshot(
            Request::get(
                "/api/v1/environments/test-environment-1/rpc/api/v3/transactions?limit=5&limit=7",
            )
            .header("accept-encoding", "br, gzip")
            .body(Body::empty())
            .expect("external proxy request must be valid"),
        )
        .await
        .expect("external proxy request must succeed");
    let _ = to_bytes(external.into_body(), usize::MAX)
        .await
        .expect("external proxy response must be readable");

    let studio_ui = app
        .clone()
        .oneshot(
            Request::post("/api/v1/environments/test-environment-1/rpc/api/v2/jsonRPC")
                .header("content-type", "application/json")
                .header("x-acton-request-source", "studio-ui")
                .body(Body::from(
                    r#"{"jsonrpc":"2.0","id":"studio","method":"sendBoc","params":{"boc":"test"}}"#,
                ))
                .expect("Studio UI proxy request must be valid"),
        )
        .await
        .expect("Studio UI proxy request must succeed");
    let _ = to_bytes(studio_ui.into_body(), usize::MAX)
        .await
        .expect("Studio UI proxy response must be readable");

    let calls = app
        .clone()
        .oneshot(
            Request::get("/api/v1/environments/test-environment-1/api-calls?limit=10")
                .body(Body::empty())
                .expect("API calls request must be valid"),
        )
        .await
        .expect("API calls request must succeed");
    let calls = to_bytes(calls.into_body(), usize::MAX)
        .await
        .expect("API calls response must be readable");
    let mut calls: Value = serde_json::from_slice(&calls).expect("API calls response must be JSON");
    for call in calls["calls"]
        .as_array_mut()
        .expect("API calls must be an array")
    {
        call["timestamp_ms"] = json!("[TIMESTAMP_MS]");
        call["duration_ns"] = json!("[DURATION_NS]");
    }

    let other_environment = app
        .oneshot(
            Request::get("/api/v1/environments/test-environment-2/api-calls")
                .body(Body::empty())
                .expect("other API calls request must be valid"),
        )
        .await
        .expect("other API calls request must succeed");
    let other_environment = to_bytes(other_environment.into_body(), usize::MAX)
        .await
        .expect("other API calls response must be readable");
    let other_environment: Value =
        serde_json::from_slice(&other_environment).expect("other API calls response must be JSON");
    upstream.abort();

    let actual = serde_json::to_string_pretty(&json!({
        "firstEnvironment": calls,
        "secondEnvironment": other_environment,
    }))
    .expect("API calls snapshot must serialize");
    expect![[r#"
        {
          "firstEnvironment": {
            "calls": [
              {
                "api_family": "v3",
                "call_type": "read",
                "duration_ns": "[DURATION_NS]",
                "http_method": "GET",
                "method": "transactions",
                "path": "/api/v3/transactions",
                "query_params": {
                  "limit": [
                    "5",
                    "7"
                  ]
                },
                "request_body": null,
                "request_body_truncated": false,
                "request_id": null,
                "response_body": {
                  "acceptEncoding": null,
                  "body": "",
                  "method": "GET",
                  "requestSource": null,
                  "uri": "/api/v3/transactions?limit=5&limit=7"
                },
                "response_body_truncated": false,
                "sequence": 1,
                "source": "external",
                "status": "success",
                "status_code": 200,
                "timestamp_ms": "[TIMESTAMP_MS]"
              },
              {
                "api_family": "json_rpc",
                "call_type": "write",
                "duration_ns": "[DURATION_NS]",
                "http_method": "POST",
                "method": "sendBoc",
                "path": "/api/v2/jsonRPC",
                "query_params": null,
                "request_body": {
                  "id": "studio",
                  "jsonrpc": "2.0",
                  "method": "sendBoc",
                  "params": {
                    "boc": "test"
                  }
                },
                "request_body_truncated": false,
                "request_id": "studio",
                "response_body": {
                  "acceptEncoding": null,
                  "body": "{\"jsonrpc\":\"2.0\",\"id\":\"studio\",\"method\":\"sendBoc\",\"params\":{\"boc\":\"test\"}}",
                  "method": "POST",
                  "requestSource": null,
                  "uri": "/api/v2/jsonRPC"
                },
                "response_body_truncated": false,
                "sequence": 2,
                "source": "studio_ui",
                "status": "success",
                "status_code": 200,
                "timestamp_ms": "[TIMESTAMP_MS]"
              }
            ],
            "max_retained": 1200,
            "total_retained": 2
          },
          "secondEnvironment": {
            "calls": [],
            "max_retained": 1200,
            "total_retained": 0
          }
        }"#]]
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
                            "adminPort":18182,
                            "configPort":18183
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
        body: {"id":"test-environment-1","name":"Protocol network","status":"running","lifecycle":"managed","rpcUrl":"/api/v1/environments/test-environment-1/rpc","config":{"kind":"fullTonNetwork","apiV2Port":18180,"apiV3Port":18181,"adminPort":18182,"configPort":18183,"importedAccounts":[]},"capabilities":["apiV2","apiV3","configApi","controlApi","explorer","integration","gramFaucet","wallets","simulator","contracts","apiCalls","snapshots"],"endpoints":{"apiV2":"/api/v1/environments/test-environment-1/rpc/api/v2","apiV3":"/api/v1/environments/test-environment-1/rpc/api/v3","config":"/api/v1/environments/test-environment-1/rpc/config","control":"/api/v1/environments/test-environment-1/rpc"},"network":{"id":"full-ton-network","label":"Full localnet","chainId":-3,"testOnly":true,"supportsActions":true}}"#]]
    .assert_eq(&actual);
}

#[tokio::test]
async fn full_ton_environment_routes_each_api_to_its_own_upstream() {
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
    let admin_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("admin proxy test listener must bind");
    let admin_port = admin_listener
        .local_addr()
        .expect("admin listener must have an address")
        .port();
    let admin_upstream = tokio::spawn(async move {
        axum::serve(
            admin_listener,
            Router::new()
                .fallback(any(proxy_target))
                .into_make_service(),
        )
        .await
        .expect("admin proxy target must serve");
    });
    let config_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("config proxy test listener must bind");
    let config_port = config_listener
        .local_addr()
        .expect("config listener must have an address")
        .port();
    let config_upstream = tokio::spawn(async move {
        axum::serve(
            config_listener,
            Router::new()
                .fallback(any(proxy_target))
                .into_make_service(),
        )
        .await
        .expect("config proxy target must serve");
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
                            "apiV3Port":{v3_port},
                            "adminPort":{admin_port},
                            "configPort":{config_port}
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
    let config_response = app
        .clone()
        .oneshot(
            Request::get("/api/v1/environments/test-environment-1/rpc/config/openapi.json")
                .body(Body::empty())
                .expect("config proxy request must be valid"),
        )
        .await
        .expect("config proxy request must succeed");
    let actual = format!(
        "V2 ROOT\n{}\n\nV2\n{}\n\nV3\n{}\n\nFAUCET\n{}\n\nCONFIG\n{}",
        response_snapshot(v2_root_response).await,
        response_snapshot(v2_response).await,
        response_snapshot(v3_response).await,
        response_snapshot(faucet_response).await,
        response_snapshot(config_response).await,
    );
    v2_upstream.abort();
    v3_upstream.abort();
    admin_upstream.abort();
    config_upstream.abort();

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

CONFIG
status: 202 Accepted
body: method: GET
uri: /openapi.json
marker: missing
body: <empty>"#]]
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
