//! Studio exercises the actual Acton CLI and control service; only Docker and TON APIs are fixtures.

use super::*;
use acton_studio::{
    ContractRegistryStore, CreateEnvironmentConfig, CreateEnvironmentRequest,
    CreateEnvironmentSnapshotRequest, CreateFullTonNodeRequest, EnvironmentConfig,
    EnvironmentRuntime, EnvironmentStatus, LocalProcessEnvironmentRuntime,
    RemoveFullTonNodeRequest, UpdateEnvironmentRequest,
};

fn executable(root: &Path, block_start: bool) -> std::path::PathBuf {
    let path = root.join("acton-fixture");
    // JSON literals are valid Python strings here; no shell interpolation touches
    // paths or environment values. The wrapper delegates every action to real Acton.
    let source = format!(
        r"#!/usr/bin/env python3
import json, os, sys
root = json.loads({root:?})
binary = json.loads({binary:?})
os.environ.update({{'PATH': root + '/bin:' + os.environ.get('PATH', ''),
    'ACTON_LOCALNET_IMAGE': 'localton:fixture', 'DOCKER_CONTEXT': 'localnet-test',
    'LOCALNET_TEST_DIR': root, 'ACTON_LOG_DIR': root + '/logs', 'NO_COLOR': '1'}})
if {block_start}:
    os.environ['LOCALNET_TEST_BLOCK_START'] = '1'
with open(root + '/acton-commands', 'a') as output:
    output.write(json.dumps(sys.argv[1:]) + '\n')
os.execv(binary, [binary] + sys.argv[1:])
",
        root = serde_json::to_string(&root.display().to_string()).expect("root"),
        binary = serde_json::to_string(env!("CARGO_BIN_EXE_acton")).expect("executable"),
        block_start = if block_start { "True" } else { "False" }
    );
    std::fs::write(&path, source).expect("Acton wrapper");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
        .expect("executable wrapper");
    path
}

async fn studio(root: &Path, executable: &Path) -> LocalProcessEnvironmentRuntime {
    LocalProcessEnvironmentRuntime::open(
        executable,
        root,
        ContractRegistryStore::for_project(root),
        Vec::new(),
    )
    .await
    .expect("Studio runtime")
}

fn request(name: &str) -> CreateEnvironmentRequest {
    CreateEnvironmentRequest {
        name: name.to_owned(),
        config: CreateEnvironmentConfig::FullTonNetwork {
            api_v2_port: None,
            api_v3_port: None,
            admin_port: None,
            config_port: None,
            observability_port: None,
            block_time_ms: Some(400),
            election_time_seconds: Some(30),
            imported_accounts: Vec::new(),
        },
    }
}

async fn running(
    runtime: &LocalProcessEnvironmentRuntime,
    id: &str,
) -> acton_studio::StudioEnvironment {
    tokio::time::timeout(Duration::from_secs(25), async {
        loop {
            let network = runtime.get(id).await.expect("environment");
            if network.status == EnvironmentStatus::Running {
                return network;
            }
            if network.status == EnvironmentStatus::Failed {
                panic!("Studio startup failed: {:?}", network.error);
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("Studio startup deadline")
}

async fn snapshot_complete(runtime: &LocalProcessEnvironmentRuntime, id: &str) {
    tokio::time::timeout(Duration::from_secs(25), async {
        loop {
            let operation = runtime
                .snapshot_operation(id)
                .await
                .expect("snapshot operation")
                .expect("accepted snapshot");
            if !operation.is_active() {
                expect![["Completed:None"]]
                    .assert_eq(&format!("{:?}:{:?}", operation.phase, operation.error));
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("snapshot deadline");
}

#[tokio::test]
async fn studio_uses_cli_for_lifecycle_and_http_for_nodes_and_snapshots() {
    let mut service = Service::start(false).await;
    let independent = service.client().await;
    let first_v2 = api_listener(service.network.network.config.port_base + 2).await;
    let first_v3 = api_listener(service.network.network.config.port_base + 3).await;
    cli(&service.state(), &["start", "integration"]).await;

    let executable = executable(service.root.path(), false);
    std::fs::write(
        service.root.path().join("Acton.toml"),
        "[package]\nname = \"studio-fixture\"\ndescription = \"Studio integration fixture\"\nversion = \"0.1.0\"\n[contracts]\n",
    )
    .expect("project manifest");
    let runtime = studio(service.root.path(), &executable).await;
    let created = runtime
        .create(request("Studio network"))
        .await
        .expect("create through Studio");
    let EnvironmentConfig::FullTonNetwork {
        api_v2_port,
        api_v3_port,
        ..
    } = created.config
    else {
        panic!("full network")
    };
    let v2 = api_listener(api_v2_port).await;
    let v3 = api_listener(api_v3_port).await;
    let ready = running(&runtime, &created.id).await;
    let timings = ready.startup_timings.expect("shared startup timings");
    expect![["true:true:true:true"]].assert_eq(&format!(
        "{}:{}:{}:{}",
        timings.compose_ms.is_some(),
        timings.ton_ready_ms.is_some(),
        timings.api_ready_ms.is_some(),
        timings.indexer_ready_ms.is_some()
    ));

    let metadata_path = service
        .root
        .path()
        .join(".studio/environments")
        .join(&created.id)
        .join("environment.json");
    let metadata: Value =
        serde_json::from_slice(&std::fs::read(&metadata_path).expect("Studio metadata"))
            .expect("metadata");
    let network_id = metadata["config"]["networkId"]
        .as_str()
        .expect("network reference");
    let location = catalog::list(&service.state())
        .await
        .expect("catalog")
        .into_iter()
        .find(|entry| entry.network.id == network_id)
        .expect("Studio network in common catalog");
    expect![[r#"["importedAccounts","kind","networkId"]"#]].assert_eq(
        &serde_json::to_string(
            &metadata["config"]
                .as_object()
                .expect("reference")
                .keys()
                .collect::<Vec<_>>(),
        )
        .expect("keys"),
    );

    runtime
        .add_full_ton_node(
            &created.id,
            CreateFullTonNodeRequest {
                name: "peer".to_owned(),
                validator: false,
            },
        )
        .await
        .expect("join node");
    runtime
        .enter_full_ton_validation(&created.id, "node-1")
        .await
        .expect("enter validation");
    runtime
        .leave_full_ton_validation(&created.id, "node-1")
        .await
        .expect("leave validation");
    let unsafe_removal = runtime
        .remove_full_ton_node(&created.id, "node-1", RemoveFullTonNodeRequest::default())
        .await;
    expect![["true"]].assert_eq(
        &matches!(
            unsafe_removal,
            Err(acton_studio::EnvironmentRuntimeError::Conflict { .. })
        )
        .to_string(),
    );
    runtime
        .remove_full_ton_node(
            &created.id,
            "node-1",
            RemoveFullTonNodeRequest { force: true },
        )
        .await
        .expect("forced removal");

    runtime
        .create_snapshot(
            &created.id,
            CreateEnvironmentSnapshotRequest {
                name: Some("checkpoint".to_owned()),
            },
        )
        .await
        .expect("snapshot accepted");
    snapshot_complete(&runtime, &created.id).await;
    expect![["snapshot-1"]].assert_eq(
        &runtime
            .list_snapshots(&created.id)
            .await
            .expect("snapshots")[0]
            .id,
    );
    runtime.stop(&created.id).await.expect("user stop");
    runtime
        .restore_snapshot(&created.id, "snapshot-1")
        .await
        .expect("restore stopped network");
    snapshot_complete(&runtime, &created.id).await;
    running(&runtime, &created.id).await;
    runtime
        .delete_snapshot(&created.id, "snapshot-1")
        .await
        .expect("delete snapshot");
    runtime
        .update(
            &created.id,
            UpdateEnvironmentRequest {
                name: "Renamed environment".to_owned(),
            },
        )
        .await
        .expect("rename");
    runtime.shutdown().await.expect("Studio graceful shutdown");
    expect![["true:false:false"]].assert_eq(&format!(
        "{}:{}:{}",
        service.network.path.join("fixture-running").exists(),
        location.path.join("fixture-running").exists(),
        location.path.join("service.json").exists()
    ));
    drop(runtime);

    let runtime = studio(service.root.path(), &executable).await;
    let resumed = running(&runtime, &created.id).await;
    expect![["Renamed environment"]].assert_eq(&resumed.name);
    runtime
        .stop(&created.id)
        .await
        .expect("persist stop intent");
    runtime
        .shutdown()
        .await
        .expect("shutdown stopped environment");
    drop(runtime);

    let runtime = studio(service.root.path(), &executable).await;
    expect![["Stopped"]].assert_eq(&format!(
        "{:?}",
        runtime
            .get(&created.id)
            .await
            .expect("stopped environment")
            .status
    ));
    runtime
        .delete(&created.id)
        .await
        .expect("delete stopped environment");
    runtime.shutdown().await.expect("empty Studio shutdown");
    expect![["0:false:true"]].assert_eq(&format!(
        "{}:{}:{}",
        runtime.list().await.expect("environments").len(),
        metadata_path.exists(),
        service.network.path.join("fixture-running").exists()
    ));

    let commands =
        std::fs::read_to_string(service.root.path().join("acton-commands")).expect("commands");
    let starts = commands
        .lines()
        .filter(|line| {
            serde_json::from_str::<Vec<String>>(line)
                .expect("argv")
                .iter()
                .any(|arg| arg == "start")
        })
        .count();
    expect![["2"]].assert_eq(&starts.to_string());
    service.stop(&independent).await;
    first_v2.abort();
    first_v3.abort();
    v2.abort();
    v3.abort();
    let _ = tokio::join!(first_v2, first_v3, v2, v3);
    drop(runtime);
    drop(service);
}

#[tokio::test]
async fn studio_shutdown_interrupts_an_actual_cli_start_gracefully() {
    let mut service = Service::start(false).await;
    let independent = service.client().await;
    let executable = executable(service.root.path(), true);
    let runtime = studio(service.root.path(), &executable).await;
    let created = runtime
        .create(request("Interrupted startup"))
        .await
        .expect("create network");
    tokio::time::timeout(Duration::from_secs(15), async {
        while !service.root.path().join("running").exists() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("Compose startup reached");
    tokio::time::timeout(Duration::from_secs(20), runtime.shutdown())
        .await
        .expect("shutdown deadline")
        .expect("graceful shutdown");
    expect![["Stopped:false:true"]].assert_eq(&format!(
        "{:?}:{}:{}",
        runtime.get(&created.id).await.expect("environment").status,
        service.root.path().join("running").exists(),
        service.network.path.join("service.json").exists()
    ));
    service.stop(&independent).await;
    drop(runtime);
    drop(service);
}

#[tokio::test]
async fn studio_waits_for_indexer_on_independently_selected_ports() {
    let mut service = Service::start(false).await;
    let independent = service.client().await;
    let executable = executable(service.root.path(), false);
    let runtime = studio(service.root.path(), &executable).await;

    let reservations: Vec<_> = (0..5)
        .map(|_| std::net::TcpListener::bind("127.0.0.1:0").expect("available port"))
        .collect();
    let ports: Vec<_> = reservations
        .iter()
        .map(|listener| listener.local_addr().expect("reserved address").port())
        .collect();
    let mut create = request("Indexer readiness");
    if let CreateEnvironmentConfig::FullTonNetwork {
        api_v2_port,
        api_v3_port,
        admin_port,
        config_port,
        observability_port,
        ..
    } = &mut create.config
    {
        *api_v2_port = Some(ports[0]);
        *api_v3_port = Some(ports[1]);
        *admin_port = Some(ports[2]);
        *config_port = Some(ports[3]);
        *observability_port = Some(ports[4]);
    }
    drop(reservations);
    let created = runtime.create(create).await.expect("custom port network");
    let v2 = api_listener(ports[0]).await;
    let height = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let response_height = std::sync::Arc::clone(&height);
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, ports[1]))
        .await
        .expect("mock indexer");
    let app = axum::Router::new().fallback(move || {
        let seqno = response_height.load(std::sync::atomic::Ordering::Acquire);
        async move { axum::Json(json!({"last":{"seqno":seqno}})) }
    });
    let v3 = tokio::spawn(async move { axum::serve(listener, app).await.expect("indexer API") });
    let location = catalog::list(&service.state())
        .await
        .expect("catalog")
        .into_iter()
        .find(|entry| entry.network.name == "Indexer readiness")
        .expect("created network");
    let client = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Ok(client) = Client::connect(&location.path).await {
                break client;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("service discovery");
    let operation = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Some(operation) = client.network().await.expect("network").operation {
                break operation;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("start accepted");
    let waiting = wait_for_progress(&client, &operation.id, "waitingForApis", 2).await;
    expect![["Starting:Waiting for Indexer"]].assert_eq(&format!(
        "{:?}:{}",
        runtime
            .get(&created.id)
            .await
            .expect("Studio projection")
            .status,
        waiting.progress.expect("readiness progress").detail
    ));
    height.store(10, std::sync::atomic::Ordering::Release);
    running(&runtime, &created.id).await;
    let config = client.network().await.expect("network config").config;
    expect![["true:true:true:true:true"]].assert_eq(&format!(
        "{}:{}:{}:{}:{}",
        config.ports().api_v2 == ports[0],
        config.ports().api_v3 == ports[1],
        config.ports().admin == ports[2],
        config.ports().config == ports[3],
        config.ports().observability == ports[4]
    ));
    runtime.shutdown().await.expect("Studio shutdown");
    service.stop(&independent).await;
    v2.abort();
    v3.abort();
    let _ = tokio::join!(v2, v3);
    drop(runtime);
    drop(service);
}

#[tokio::test]
async fn studio_ctrl_c_reports_progress_before_actual_shutdown_completion() {
    let mut service = Service::start(false).await;
    let independent = service.client().await;
    let executable = executable(service.root.path(), false);
    let runtime = studio(service.root.path(), &executable).await;
    let created = runtime
        .create(request("Slow shutdown"))
        .await
        .expect("create network");
    let EnvironmentConfig::FullTonNetwork {
        api_v2_port,
        api_v3_port,
        ..
    } = created.config
    else {
        panic!("full network")
    };
    let v2 = api_listener(api_v2_port).await;
    let v3 = api_listener(api_v3_port).await;
    running(&runtime, &created.id).await;
    runtime.shutdown().await.expect("save running intent");
    drop(runtime);

    let log_path = service.root.path().join("studio.log");
    let log = std::fs::File::create(&log_path).expect("Studio output");
    let reservation = std::net::TcpListener::bind("127.0.0.1:0").expect("Studio port");
    let port = reservation
        .local_addr()
        .expect("Studio address")
        .port()
        .to_string();
    drop(reservation);
    let mut child = Command::new(executable)
        .arg("--project-root")
        .arg(service.root.path())
        .args(["studio", "start", "--port", &port, "--no-open"])
        .stdin(Stdio::null())
        .stdout(Stdio::from(log.try_clone().expect("clone output")))
        .stderr(Stdio::from(log))
        .kill_on_drop(true)
        .spawn()
        .expect("Studio CLI");
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let output = std::fs::read_to_string(&log_path).expect("Studio output");
            if output.contains("Starting Acton Studio")
                && service.root.path().join("running").exists()
            {
                break;
            }
            if let Some(status) = child.try_wait().expect("Studio status") {
                panic!("Studio exited with {status}: {output}");
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("Studio resumed environment");

    // Hold Docker cleanup until the terminal has reported the actual pending
    // environment. This verifies feedback independently of machine speed.
    std::fs::write(service.root.path().join("hold-stop"), "").expect("hold stop");
    let signal = Command::new("kill")
        .args(["-INT", &child.id().expect("owned Studio PID").to_string()])
        .status()
        .await
        .expect("Ctrl-C");
    expect![["true"]].assert_eq(&signal.success().to_string());
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let output = std::fs::read_to_string(&log_path).expect("Studio output");
            if output.contains("0/1 environments stopped") {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("immediate shutdown progress");
    expect![["true:true:false"]].assert_eq(&format!(
        "{}:{}:{}",
        child.try_wait().expect("Studio status").is_none(),
        service.root.path().join("running").exists(),
        std::fs::read_to_string(&log_path)
            .expect("output")
            .contains("Stopped Acton Studio")
    ));
    std::fs::remove_file(service.root.path().join("hold-stop")).expect("release stop");
    let status = tokio::time::timeout(Duration::from_secs(15), child.wait())
        .await
        .expect("shutdown deadline")
        .expect("Studio exit");
    let output = std::fs::read_to_string(log_path).expect("Studio output");
    let messages = output
        .lines()
        .filter(|line| {
            line.contains("(shutdown requested)")
                || line.contains("0/1 environments stopped")
                || line.contains("Stopped Acton Studio")
        })
        .map(|line| format!("|{}", line.split(" in ").next().expect("message")))
        .collect::<Vec<_>>()
        .join("\n");
    expect![[r#"
        |    Stopping Acton Studio gracefully (shutdown requested)
        |    Stopping environment "Slow shutdown" — 0/1 environments stopped
        |     Stopped Acton Studio gracefully
    "#]]
    .assert_eq(&format!("{messages}\n"));
    expect![["true:false:true"]].assert_eq(&format!(
        "{}:{}:{}",
        status.success(),
        service.root.path().join("running").exists(),
        service.network.path.join("service.json").exists()
    ));
    service.stop(&independent).await;
    v2.abort();
    v3.abort();
    let _ = tokio::join!(v2, v3);
    drop(service);
}
