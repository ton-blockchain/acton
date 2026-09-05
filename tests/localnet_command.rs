//! Subprocess tests exercise CLI discovery, HTTP operations, and graceful teardown.
//! Docker is replaced only in the child environment; the real runtime is used.

#![cfg(unix)]

use acton_localnet::{CreateNetwork, Network, Operation, catalog, client::Client};
use expect_test::expect;
use reqwest::Method;
use serde_json::{Value, json};
use std::{os::unix::fs::PermissionsExt, path::Path, process::Stdio, time::Duration};
use tokio::process::{Child, Command};

#[path = "localnet_command/isolation.rs"]
mod isolation;

#[path = "localnet_command/selection.rs"]
mod selection;

#[path = "localnet_command/offline.rs"]
mod offline;

#[path = "localnet_command/shutdown.rs"]
mod shutdown;

#[path = "localnet_command/studio.rs"]
mod studio;

// Independent temporary projects share the host port space. Serialize fixtures
// until their mock APIs are gone; each scenario can still run several services.
static FIXTURE_PORTS: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct Service {
    root: tempfile::TempDir,
    child: Child,
    network: catalog::NetworkDirectory,
    _port_guard: tokio::sync::MutexGuard<'static, ()>,
}

impl Service {
    async fn start(block_start: bool) -> Self {
        Self::launch(block_start, &["serve"]).await
    }

    async fn launch(block_start: bool, args: &[&str]) -> Self {
        let port_guard = FIXTURE_PORTS.lock().await;
        let root = tempfile::tempdir().expect("test workspace");
        let bin = root.path().join("bin");
        std::fs::create_dir(&bin).expect("fixture bin");
        let docker = bin.join("docker");
        std::fs::write(&docker, include_str!("fixtures/localnet/docker.py"))
            .expect("Docker fixture");
        std::fs::set_permissions(&docker, std::fs::Permissions::from_mode(0o755))
            .expect("executable fixture");

        let network = catalog::create(
            &root.path().join(".acton-localnet"),
            CreateNetwork {
                name: if args.first() == Some(&"start") {
                    "localnet"
                } else {
                    "integration"
                }
                .to_owned(),
                block_time_ms: Some(400),
                election_time_seconds: Some(30),
                ..Default::default()
            },
        )
        .await
        .expect("network definition");

        let log = std::fs::File::create(root.path().join("service.log")).expect("service log");
        let mut command = Command::from(acton(root.path(), args));
        command
            .stdin(Stdio::null())
            .stdout(Stdio::from(log.try_clone().expect("clone log")))
            .stderr(Stdio::from(log))
            .kill_on_drop(true);

        if block_start {
            command.env("LOCALNET_TEST_BLOCK_START", "1");
        }

        Self {
            root,
            child: command.spawn().expect("localnet service"),
            network,
            _port_guard: port_guard,
        }
    }

    fn state(&self) -> std::path::PathBuf {
        self.root.path().join(".acton-localnet")
    }

    async fn client(&mut self) -> Client {
        tokio::time::timeout(Duration::from_secs(20), async {
            loop {
                if let Ok(client) = Client::connect(&self.network.path).await {
                    break client;
                }

                if let Some(status) = self.child.try_wait().expect("service status") {
                    panic!(
                        "Service exited with {status}: {}",
                        std::fs::read_to_string(self.root.path().join("service.log"))
                            .unwrap_or_default()
                    );
                }

                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .expect("service discovery")
    }

    async fn stop(&mut self, client: &Client) {
        let _: Value = client
            .request(Method::POST, "/v1/shutdown", None)
            .await
            .expect("shutdown accepted");
        let status = tokio::time::timeout(Duration::from_secs(20), self.child.wait())
            .await
            .expect("graceful shutdown deadline")
            .expect("service exit");
        expect![["true:false"]].assert_eq(&format!(
            "{}:{}",
            status.success(),
            self.network.path.join("service.json").exists()
        ));
    }
}

async fn wait_for_progress(client: &Client, id: &str, phase: &str, completed: u64) -> Operation {
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let operation: Operation = client
                .request(Method::GET, &format!("/v1/operations/{id}"), None)
                .await
                .expect("operation");

            if operation.phase == phase
                && operation
                    .progress
                    .as_ref()
                    .is_some_and(|progress| progress.completed == completed)
            {
                break operation;
            }

            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("operation deadline")
}

async fn api_listener(port: u16) -> tokio::task::JoinHandle<()> {
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))
        .await
        .expect("mock TON API");
    let app = axum::Router::new().fallback(|| async {
        axum::Json(json!({"ok":true, "result":{"last":{"seqno":10}}, "last":{"seqno":10}}))
    });

    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("mock API server");
    })
}

fn acton(root: &Path, args: &[&str]) -> std::process::Command {
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_acton"));
    command
        .arg("--project-root")
        .arg(root)
        .arg("localnet")
        .args(args)
        .env(
            "PATH",
            format!(
                "{}:{}",
                root.join("bin").display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .env("ACTON_LOCALNET_IMAGE", "localton:fixture")
        .env("DOCKER_CONTEXT", "localnet-test")
        .env("LOCALNET_TEST_DIR", root)
        .env("ACTON_LOG_DIR", root.join("logs"))
        .env("NO_COLOR", "1");
    command
}

async fn cli(state: &Path, args: &[&str]) -> Value {
    let mut command = Command::from(acton(state.parent().expect("workspace"), args));
    let output = command.arg("--json").output().await.expect("CLI output");

    if !output.status.success() {
        panic!("CLI failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    serde_json::from_slice(&output.stdout).expect("CLI JSON")
}

#[tokio::test]
async fn cli_and_http_share_lifecycle_snapshots_and_persisted_state() {
    let mut service = Service::start(false).await;
    let client = service.client().await;
    let network = service.network.network.clone();
    let v2 = api_listener(network.config.port_base + 2).await;
    let v3 = api_listener(network.config.port_base + 3).await;

    let started = cli(&service.state(), &["start", "integration"]).await;
    let snapshot = cli(
        &service.state(),
        &["snapshot", "integration", "create", "checkpoint"],
    )
    .await;
    let restored = cli(
        &service.state(),
        &["snapshot", "integration", "restore", "snapshot-1", "--yes"],
    )
    .await;
    let node = cli(&service.state(), &["node", "integration", "add", "peer"]).await;
    let status = cli(&service.state(), &["status", "integration"]).await;

    expect![[r#"
        {
          "node": "peer",
          "nodes": 1,
          "readiness": {
            "completed": 3,
            "detail": "TON APIs and indexer ready",
            "total": 3,
            "unit": "checks passed"
          },
          "restored": "completed",
          "snapshot": "snapshot-1",
          "snapshotStatus": "completed",
          "startSteps": [
            "preparing",
            "checkingImage",
            "startingContainers",
            "waitingForApis"
          ],
          "started": "running"
        }"#]].assert_eq(
        &serde_json::to_string_pretty(&json!({
            "started":started["status"],
            "startSteps":started["operation"]["completedSteps"].as_array().expect("completed steps").iter().map(|step| &step["phase"]).collect::<Vec<_>>(),
            "readiness":started["operation"]["progress"],
            "snapshot":snapshot["result"]["id"],
            "snapshotStatus":snapshot["status"],
            "restored":restored["status"],
            "node":node["result"]["name"],
            "nodes":status["nodes"].as_array().expect("nodes").len(),
        }))
        .expect("snapshot"),
    );

    let human = Command::new(env!("CARGO_BIN_EXE_acton"))
        .arg("--project-root")
        .arg(service.root.path())
        .args(["localnet", "--state-dir"])
        .arg(service.state())
        .args(["status", "integration"])
        .env("NO_COLOR", "1")
        .env("ACTON_LOG_DIR", service.root.path().join("logs"))
        .output()
        .await
        .expect("human status output");
    expect![["true"]].assert_eq(&human.status.success().to_string());
    let output = String::from_utf8(human.stdout)
        .expect("UTF-8")
        .replace(
            started["state"]["volume"].as_str().expect("state volume"),
            "<state-volume>",
        )
        .replace(&network.id, "<network-id>")
        .replace(&network.endpoints.api_v2, "<api-v2>")
        .replace(&network.endpoints.api_v3, "<api-v3>")
        .replace(&network.endpoints.admin, "<admin>")
        .replace(&network.endpoints.config, "<config>")
        .replace(&network.endpoints.observability, "<dashboard>");
    expect![[r#"

        Full localnet "integration"
          Status:    running
          Network:   <network-id>
          State:     /var/lib/localton (inside Docker)
          Volume:    <state-volume>
          API v2:    <api-v2>
          API v3:    <api-v3>
          Admin:     <admin>
          Config:    <config>
          Dashboard: <dashboard>
    "#]]
    .assert_eq(&output);

    service.stop(&client).await;
    let record: Network = serde_json::from_slice(
        &std::fs::read(service.network.path.join("network.json")).expect("record"),
    )
    .expect("network record");
    expect![["Stopped:false"]].assert_eq(&format!(
        "{:?}:{}",
        record.status,
        service.root.path().join("running").exists()
    ));
    expect![[r"
        up
        stop
        snapshot-create
        up
        stop
        snapshot-restore
        down
        up
        up
        stop
    "]]
    .assert_eq(
        &std::fs::read_to_string(service.root.path().join("events")).expect("Docker events"),
    );

    v2.abort();
    v3.abort();

    let _ = tokio::join!(v2, v3);
    drop(service);
}

#[tokio::test]
async fn shutdown_interrupts_startup_and_conflicting_mutations_are_rejected() {
    let mut service = Service::start(true).await;
    let client = service.client().await;
    std::fs::write(service.root.path().join("force-pull"), "").expect("missing image");
    let operation: Operation = client
        .request(Method::POST, "/v1/network/start", None)
        .await
        .expect("start accepted");

    let pulling = wait_for_progress(&client, &operation.id, "pullingImage", 1).await;
    expect![[r#"
        {
          "completed": 1,
          "total": null,
          "unit": "layers ready",
          "detail": "bbbbbbbbbbbb: Download complete"
        }"#]]
    .assert_eq(&serde_json::to_string_pretty(&pulling.progress).expect("pull progress"));
    std::fs::write(service.root.path().join("continue-pull"), "").expect("finish pull");

    let starting = wait_for_progress(&client, &operation.id, "startingContainers", 3).await;
    expect![[r#"
        {
          "completed": 3,
          "total": 9,
          "unit": "ready",
          "detail": "localton: health check (+5 waiting)"
        }"#]]
    .assert_eq(&serde_json::to_string_pretty(&starting.progress).expect("container progress"));

    tokio::time::timeout(Duration::from_secs(10), async {
        while !service.root.path().join("running").exists() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("Compose started");

    let conflict = client
        .request::<Value>(Method::POST, "/v1/network/stop", None)
        .await
        .expect_err("conflict");
    expect![["true"]]
        .assert_eq(&matches!(conflict, acton_localnet::Error::Api { status: 409, .. }).to_string());
    service.stop(&client).await;

    let completed: Operation = serde_json::from_slice(
        &std::fs::read(
            service
                .network
                .path
                .join("operations")
                .join(format!("{}.json", operation.id)),
        )
        .expect("operation file"),
    )
    .expect("operation record");
    expect![["Failed:false"]].assert_eq(&format!(
        "{:?}:{}",
        completed.status,
        service.root.path().join("running").exists()
    ));

    drop(service);
}

#[tokio::test]
async fn foreground_ctrl_c_reports_request_before_graceful_completion() {
    let mut service = Service::launch(true, &["start"]).await;
    let _client = service.client().await;
    std::fs::write(service.root.path().join("slow-stop"), "").expect("delayed stop");

    tokio::time::timeout(Duration::from_secs(10), async {
        while !service.root.path().join("running").exists() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("startup reached Docker");

    // Signal only the foreground process spawned by this test. Its service lives
    // in a separate process group and must be stopped through the ownership path.
    let signal = Command::new("kill")
        .args([
            "-INT",
            &service.child.id().expect("foreground PID").to_string(),
        ])
        .status()
        .await
        .expect("Ctrl-C");
    expect![["true"]].assert_eq(&signal.success().to_string());

    let log_path = service.root.path().join("service.log");
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let output = std::fs::read_to_string(&log_path).expect("foreground output");
            if output.contains("(shutdown requested)") {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("immediate shutdown feedback");
    expect![["true:true"]].assert_eq(&format!(
        "{}:{}",
        service
            .child
            .try_wait()
            .expect("foreground status")
            .is_none(),
        service.root.path().join("running").exists()
    ));

    let status = tokio::time::timeout(Duration::from_secs(20), service.child.wait())
        .await
        .expect("shutdown deadline")
        .expect("foreground exit");
    let output = std::fs::read_to_string(log_path).expect("foreground output");
    let shutdown_lines = output
        .lines()
        .filter(|line| {
            line.contains("(shutdown requested)")
                || line.contains("Docker services gracefully;")
                || line.contains("Stopped Acton localnet gracefully")
        })
        .map(|line| format!("|{line}"))
        .collect::<Vec<_>>()
        .join("\n");
    expect![[r"
        |    Stopping Acton localnet gracefully (shutdown requested)
        |    Stopping Docker services gracefully; preserving network data
        |     Stopped Acton localnet gracefully
    "]]
    .assert_eq(&format!("{shutdown_lines}\n"));
    expect![["true:false:false"]].assert_eq(&format!(
        "{}:{}:{}",
        status.success(),
        service.network.path.join("service.json").exists(),
        service.root.path().join("running").exists()
    ));

    drop(service);
}
