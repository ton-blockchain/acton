//! Exercises the Studio HTTP handlers, pinned manifests and contract registry
//! against a control-service fixture; Docker's hardfork is covered separately.

use super::*;
use crate::local_process::{
    EnvironmentDriver, LocalProcessEnvironmentRuntime, persist_environment_definition,
    runtime_endpoints,
};
use crate::{
    ContractRegistryStore, EnvironmentStatus, StudioEnvironment, StudioServer, StudioServerConfig,
};
use acton_localnet::{CreateNetwork, catalog};
use axum::{
    Json, Router,
    body::{Body, to_bytes},
    extract::{Query, State},
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use expect_test::expect;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64},
    },
};
use tokio::sync::{Mutex, RwLock};
use tower::ServiceExt;

#[derive(Default)]
struct Control {
    requests: Mutex<Vec<Value>>,
    operations: Mutex<HashMap<String, AdminOperation>>,
    lose_response: AtomicBool,
    source_boc: RwLock<String>,
    source_reads: AtomicU64,
}

async fn submit(State(state): State<Arc<Control>>, Json(request): Json<AdminRequest>) -> Response {
    let operation = AdminOperation {
        id: request.id().into(),
        phase: "indexing".into(),
        started_at: "2026-09-07T00:00:00Z".into(),
        finished_at: None,
        error: None,
        block_seqno: None,
    };
    state
        .requests
        .lock()
        .await
        .push(serde_json::to_value(request).expect("request"));
    state
        .operations
        .lock()
        .await
        .entry(operation.id.clone())
        .or_insert_with(|| operation.clone());
    if state.lose_response.swap(false, Ordering::AcqRel) {
        return (StatusCode::BAD_GATEWAY, Json(json!({"error": {"code": "lost_response", "message": "Response lost after admission"}}))).into_response();
    }
    Json(operation).into_response()
}

async fn result(
    State(state): State<Arc<Control>>,
    Query(query): Query<HashMap<String, String>>,
) -> Json<Option<AdminOperation>> {
    Json(
        state
            .operations
            .lock()
            .await
            .get(query.get("id").expect("specific operation ID"))
            .cloned(),
    )
}

async fn source(State(state): State<Arc<Control>>) -> Json<Value> {
    state.source_reads.fetch_add(1, Ordering::AcqRel);
    Json(json!({"ok": true, "result": {"bytes": state.source_boc.read().await.clone()}}))
}

async fn request(router: &Router, path: &str, body: Option<&Value>) -> (u16, Value) {
    let request = Request::builder()
        .uri(path)
        .method(if body.is_some() { "POST" } else { "GET" })
        .header("content-type", "application/json")
        .body(body.map_or_else(Body::empty, |body| Body::from(body.to_string())))
        .expect("request");
    let response = router.clone().oneshot(request).await.expect("response");
    let status = response.status().as_u16();
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body");
    (
        status,
        serde_json::from_slice(&bytes).expect("JSON response"),
    )
}

#[tokio::test]
async fn import_pins_cells_and_registers_after_success_across_a_studio_restart() {
    let temp = tempfile::tempdir_in("/tmp").expect("workspace");
    let registry = ContractRegistryStore::for_project(temp.path());
    let runtime =
        LocalProcessEnvironmentRuntime::open("unused", temp.path(), registry.clone(), vec![])
            .await
            .expect("runtime");
    let state = Arc::new(Control::default());
    state.lose_response.store(true, Ordering::Release);
    *state.source_boc.write().await = SOURCE_BOC.into();

    let root = localnet::root(temp.path());
    fs::create_dir_all(&root).await.expect("catalog");
    let mut location = catalog::create(
        &root,
        CreateNetwork {
            name: "import-test".into(),
            ..Default::default()
        },
    )
    .await
    .expect("network");
    location.network.status = acton_localnet::Status::Running;
    let network = location.network.clone();
    let server = Router::new()
        .route(
            "/v1/health",
            get(|| async { Json(json!({"service":"acton-localnet", "protocolVersion":1})) }),
        )
        .route(
            "/v1/network",
            get(move || {
                let network = network.clone();
                async move { Json(network) }
            }),
        )
        .route("/v1/network/admin", get(result).post(submit))
        .route("/api/v2/getShardAccountCell", get(source))
        .with_state(Arc::clone(&state));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener");
    let endpoint = format!("http://{}", listener.local_addr().expect("address"));
    let listener = tokio::spawn(async move { axum::serve(listener, server).await.expect("serve") });
    fs::write(
        location.path.join("service.json"),
        serde_json::to_vec(
            &json!({"protocolVersion":1,"url":endpoint,"pid":std::process::id(),"token":"fixture"}),
        )
        .expect("descriptor"),
    )
    .await
    .expect("descriptor file");

    let config = localnet::configuration(&location.network, vec![]);
    let environment = Arc::new(LocalEnvironment {
        details: RwLock::new(StudioEnvironment::new(
            "environment-1",
            "Target",
            EnvironmentStatus::Running,
            config.clone(),
            runtime_endpoints(&config),
        )),
        driver: EnvironmentDriver::FullTonNetwork(Box::new(localnet::FullLocalnet::new(
            Path::new("unused"),
            temp.path(),
            location,
        ))),
        child: Mutex::new(None),
        lifecycle: Arc::new(Mutex::new(())),
        snapshot_operation: RwLock::new(None),
        generation: AtomicU64::new(1),
        resume_on_startup: AtomicBool::new(false),
        deleted: AtomicBool::new(false),
    });
    persist_environment_definition(&runtime.inner, &environment, false)
        .await
        .expect("persist environment");
    runtime
        .inner
        .environments
        .write()
        .await
        .push(Arc::clone(&environment));
    let source_config = EnvironmentConfig::ActonSimulatedLocalnet {
        port: 1,
        fork_network: None,
        fork_block_number: None,
        accounts: vec![],
        rate_limit: None,
        response_delay_ms: None,
        block_time_ms: None,
        no_mining: false,
        mine_empty_blocks: false,
    };
    let source_environment = Arc::new(LocalEnvironment {
        details: RwLock::new(StudioEnvironment::new(
            "environment-2",
            "Source",
            EnvironmentStatus::Running,
            source_config.clone(),
            crate::EnvironmentEndpoints {
                api_v2: Some(format!("{endpoint}/api/v2")),
                ..Default::default()
            },
        )),
        driver: EnvironmentDriver::new(
            Path::new("unused"),
            temp.path(),
            temp.path(),
            &source_config,
            None,
        )
        .expect("source driver"),
        child: Mutex::new(None),
        lifecycle: Arc::new(Mutex::new(())),
        snapshot_operation: RwLock::new(None),
        generation: AtomicU64::new(1),
        resume_on_startup: AtomicBool::new(false),
        deleted: AtomicBool::new(false),
    });
    runtime
        .inner
        .environments
        .write()
        .await
        .push(source_environment);
    let router = StudioServer::new(StudioServerConfig::new("test"))
        .with_environment_runtime(runtime.clone())
        .router();
    let id = Uuid::new_v4().to_string();
    let import = json!({"id":id, "accounts":[{"sourceEnvironmentId":"environment-2","address": format!("0:{}", "45".repeat(32)),"name":"Imported account"}]});
    let path = "/api/v1/environments/environment-1/imports";
    let first = request(&router, path, Some(&import)).await;
    let registry_before = registry
        .snapshot("environment-1")
        .await
        .expect("registry")
        .contracts
        .len();
    // The source can become unreadable after preparation. A retry still submits
    // the original cells and never asks the source to resolve the account again.
    *state.source_boc.write().await = "source is no longer readable".into();
    let second = request(&router, path, Some(&import)).await;
    let requests = state.requests.lock().await.clone();
    let mut changed = import.clone();
    changed["accounts"][0]["name"] = "Different import".into();
    let conflict = request(&router, path, Some(&changed)).await;

    {
        let mut operations = state.operations.lock().await;
        let failed = operations.get_mut(&id).expect("first operation");
        failed.phase = "failed".into();
        failed.finished_at = Some("2026-09-07T00:01:00Z".into());
        failed.error = Some("Hardfork rolled back".into());
        drop(operations);
    }
    let failed = request(
        &router,
        &format!("/api/v1/environments/environment-1/admin?id={id}"),
        None,
    )
    .await;
    let after_failure = registry
        .snapshot("environment-1")
        .await
        .expect("registry")
        .contracts
        .len();

    // A terminal failure requires a new import ID. It must not publish a
    // contract; a subsequent successful operation can still recover on startup.
    let mut successful_import = import.clone();
    let id = Uuid::new_v4().to_string();
    successful_import["id"] = id.clone().into();
    *state.source_boc.write().await = SOURCE_BOC.into();
    let success = request(&router, path, Some(&successful_import)).await;
    let first_finalized = directory(&runtime.inner, "environment-1")
        .join(format!("{}.json", import["id"].as_str().expect("first id")))
        .exists();

    runtime.inner.shutting_down.store(true, Ordering::Release);
    drop(router);
    drop(runtime);
    state
        .operations
        .lock()
        .await
        .get_mut(&id)
        .expect("operation")
        .phase = "completed".into();
    state
        .operations
        .lock()
        .await
        .get_mut(&id)
        .expect("operation")
        .finished_at = Some("2026-09-07T00:01:00Z".into());
    let restarted_registry = ContractRegistryStore::for_project(temp.path());
    let restarted = LocalProcessEnvironmentRuntime::open(
        "unused",
        temp.path(),
        restarted_registry.clone(),
        vec![],
    )
    .await
    .expect("reopen Studio");
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if restarted_registry
                .snapshot("environment-1")
                .await
                .expect("registry")
                .contracts
                .len()
                == 1
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("background registration without a browser");
    let snapshot = restarted_registry
        .snapshot("environment-1")
        .await
        .expect("registry");
    let actual = json!({
        "firstStatus": first.0,
        "retryStatus": second.0,
        "retryPhase": second.1["phase"],
        "identicalEdits": requests[0] == requests[1],
        "sourceReads": state.source_reads.load(Ordering::Acquire),
        "registeredBeforeSuccess": registry_before,
        "registeredAfterFailure": after_failure,
        "failedPhase": failed.1["phase"],
        "failedManifestFinalized": first_finalized,
        "newImportStatus": success.0,
        "conflictStatus": conflict.0,
        "conflictCode": conflict.1["error"]["code"],
        "registeredAfterRestart": snapshot.contracts.len(),
        "name": snapshot.address_name(&format!("0:{}", "45".repeat(32))),
    });
    expect![[r#"
        {
          "conflictCode": "account_import_id_reused",
          "conflictStatus": 409,
          "failedManifestFinalized": true,
          "failedPhase": "failed",
          "firstStatus": 500,
          "identicalEdits": true,
          "name": "Imported account",
          "newImportStatus": 200,
          "registeredAfterFailure": 0,
          "registeredAfterRestart": 1,
          "registeredBeforeSuccess": 0,
          "retryPhase": "indexing",
          "retryStatus": 200,
          "sourceReads": 2
        }"#]]
    .assert_eq(&serde_json::to_string_pretty(&actual).expect("snapshot"));

    restarted.inner.shutting_down.store(true, Ordering::Release);
    drop(restarted);
    listener.abort();
    let _ = listener.await;
}

const SOURCE_BOC: &str = "te6cckEBBAEAbQABUAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABAmnABFRUVFRUVFRUVFRUVFRUVFRUVFRUVFRUVFRUVFRUVFRQAAAAAAAAAAAAAAAAAQ7msoATQAIDAAIAAAgAAAAqoem81g==";
