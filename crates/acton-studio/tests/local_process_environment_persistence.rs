#![cfg(unix)]

use std::fmt::Write as _;
use std::net::{Ipv4Addr, TcpListener};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use acton_studio::{
    ContractRegistryStore, CreateEnvironmentConfig, CreateEnvironmentRequest, EnvironmentConfig,
    EnvironmentRuntime, LocalProcessEnvironmentRuntime, StudioEnvironment,
    UpdateEnvironmentRequest,
};
use expect_test::expect;
use serde_json::Value;

#[tokio::test]
async fn local_process_environment_persists_its_full_lifecycle() {
    let workspace =
        tempfile::tempdir_in("/tmp").expect("temporary Studio workspace must be created");
    let executable = workspace.path().join("fake-acton");
    write_sleeping_executable(&executable);
    let port = available_local_port();

    let runtime = LocalProcessEnvironmentRuntime::open(
        &executable,
        workspace.path(),
        ContractRegistryStore::for_project(workspace.path()),
        Vec::new(),
    )
    .await
    .expect("empty environment runtime must open");
    let created = runtime
        .create(CreateEnvironmentRequest {
            name: "Initial localnet".to_owned(),
            config: CreateEnvironmentConfig::ActonLocalnet {
                port: Some(port),
                fork_network: Some("testnet".to_owned()),
                fork_block_number: Some(123_456),
                accounts: vec!["deployer".to_owned(), "treasury".to_owned()],
                rate_limit: Some(25),
                response_delay_ms: Some(40),
                block_interval_ms: Some(500),
                no_mining: false,
                mine_empty_blocks: true,
            },
        })
        .await
        .expect("environment must be created");
    let created = runtime
        .update(
            &created.id,
            UpdateEnvironmentRequest {
                name: "Persistent localnet".to_owned(),
            },
        )
        .await
        .expect("environment name must be updated");
    let environment_dir = workspace
        .path()
        .join(".studio/environments")
        .join(&created.id);
    std::fs::write(
        environment_dir.join("persisted.marker"),
        "environment data survives",
    )
    .expect("environment marker must be written");

    let mut actual = String::new();
    append_environment(&mut actual, "created", &created, &environment_dir, None);

    runtime
        .shutdown()
        .await
        .expect("created environment must shut down");
    drop(runtime);

    let runtime = LocalProcessEnvironmentRuntime::open(
        &executable,
        workspace.path(),
        ContractRegistryStore::for_project(workspace.path()),
        Vec::new(),
    )
    .await
    .expect("environment runtime must reopen");
    let restored = only_environment(&runtime).await;
    append_environment(
        &mut actual,
        "after shutdown and open",
        &restored,
        &environment_dir,
        Some(&created),
    );

    let stopped = runtime
        .stop(&created.id)
        .await
        .expect("environment must stop");
    append_environment(
        &mut actual,
        "after user stop",
        &stopped,
        &environment_dir,
        Some(&created),
    );
    runtime
        .shutdown()
        .await
        .expect("stopped environment runtime must shut down");
    drop(runtime);

    let runtime = LocalProcessEnvironmentRuntime::open(
        &executable,
        workspace.path(),
        ContractRegistryStore::for_project(workspace.path()),
        Vec::new(),
    )
    .await
    .expect("stopped environment runtime must reopen");
    let restored_stopped = only_environment(&runtime).await;
    append_environment(
        &mut actual,
        "after stopped reopen",
        &restored_stopped,
        &environment_dir,
        Some(&created),
    );

    let restarted = runtime
        .restart(&created.id)
        .await
        .expect("stopped environment must restart");
    append_environment(
        &mut actual,
        "after restart",
        &restarted,
        &environment_dir,
        Some(&created),
    );
    runtime
        .shutdown()
        .await
        .expect("restarted environment runtime must shut down");
    drop(runtime);

    let runtime = LocalProcessEnvironmentRuntime::open(
        &executable,
        workspace.path(),
        ContractRegistryStore::for_project(workspace.path()),
        Vec::new(),
    )
    .await
    .expect("desired-running environment runtime must reopen");
    let resumed = only_environment(&runtime).await;
    append_environment(
        &mut actual,
        "after restarted reopen",
        &resumed,
        &environment_dir,
        Some(&created),
    );

    runtime
        .delete(&created.id)
        .await
        .expect("environment must be deleted");
    writeln!(
        actual,
        "after delete\nlisted: {}\ndata directory exists: {}",
        runtime
            .list()
            .await
            .expect("environment list must remain readable")
            .len(),
        environment_dir.exists(),
    )
    .expect("snapshot text must be writable");
    runtime
        .shutdown()
        .await
        .expect("empty environment runtime must shut down");
    drop(runtime);

    let runtime = LocalProcessEnvironmentRuntime::open(
        &executable,
        workspace.path(),
        ContractRegistryStore::for_project(workspace.path()),
        Vec::new(),
    )
    .await
    .expect("environment runtime must reopen after deletion");
    writeln!(
        actual,
        "after delete reopen\nlisted: {}",
        runtime
            .list()
            .await
            .expect("persisted environment list must remain readable")
            .len(),
    )
    .expect("snapshot text must be writable");
    runtime
        .shutdown()
        .await
        .expect("final environment runtime must shut down");

    expect![[r#"created
id: environment-1
name: Persistent localnet
status: Starting
same id/name/config: n/a
config: {"accounts":["deployer","treasury"],"blockIntervalMs":500,"forkBlockNumber":123456,"forkNetwork":"testnet","kind":"actonLocalnet","mineEmptyBlocks":true,"noMining":false,"port":"<PORT>","rateLimit":25,"responseDelayMs":40}
desired running: true
marker: environment data survives
after shutdown and open
id: environment-1
name: Persistent localnet
status: Starting
same id/name/config: true
config: {"accounts":["deployer","treasury"],"blockIntervalMs":500,"forkBlockNumber":123456,"forkNetwork":"testnet","kind":"actonLocalnet","mineEmptyBlocks":true,"noMining":false,"port":"<PORT>","rateLimit":25,"responseDelayMs":40}
desired running: true
marker: environment data survives
after user stop
id: environment-1
name: Persistent localnet
status: Stopped
same id/name/config: true
config: {"accounts":["deployer","treasury"],"blockIntervalMs":500,"forkBlockNumber":123456,"forkNetwork":"testnet","kind":"actonLocalnet","mineEmptyBlocks":true,"noMining":false,"port":"<PORT>","rateLimit":25,"responseDelayMs":40}
desired running: false
marker: environment data survives
after stopped reopen
id: environment-1
name: Persistent localnet
status: Stopped
same id/name/config: true
config: {"accounts":["deployer","treasury"],"blockIntervalMs":500,"forkBlockNumber":123456,"forkNetwork":"testnet","kind":"actonLocalnet","mineEmptyBlocks":true,"noMining":false,"port":"<PORT>","rateLimit":25,"responseDelayMs":40}
desired running: false
marker: environment data survives
after restart
id: environment-1
name: Persistent localnet
status: Starting
same id/name/config: true
config: {"accounts":["deployer","treasury"],"blockIntervalMs":500,"forkBlockNumber":123456,"forkNetwork":"testnet","kind":"actonLocalnet","mineEmptyBlocks":true,"noMining":false,"port":"<PORT>","rateLimit":25,"responseDelayMs":40}
desired running: true
marker: environment data survives
after restarted reopen
id: environment-1
name: Persistent localnet
status: Starting
same id/name/config: true
config: {"accounts":["deployer","treasury"],"blockIntervalMs":500,"forkBlockNumber":123456,"forkNetwork":"testnet","kind":"actonLocalnet","mineEmptyBlocks":true,"noMining":false,"port":"<PORT>","rateLimit":25,"responseDelayMs":40}
desired running: true
marker: environment data survives
after delete
listed: 0
data directory exists: false
after delete reopen
listed: 0
"#]]
    .assert_eq(&actual);
}

async fn only_environment(runtime: &LocalProcessEnvironmentRuntime) -> StudioEnvironment {
    let environments = runtime
        .list()
        .await
        .expect("environment list must be readable");
    let [environment] = environments.as_slice() else {
        panic!(
            "environment runtime must contain exactly one environment, found {}",
            environments.len()
        );
    };
    environment.clone()
}

fn append_environment(
    actual: &mut String,
    label: &str,
    environment: &StudioEnvironment,
    environment_dir: &Path,
    original: Option<&StudioEnvironment>,
) {
    let same_environment = original.map_or_else(
        || "n/a".to_owned(),
        |original| {
            (environment.id == original.id
                && environment.name == original.name
                && config_value(&environment.config) == config_value(&original.config))
            .to_string()
        },
    );
    writeln!(
        actual,
        "{label}\nid: {}\nname: {}\nstatus: {:?}\nsame id/name/config: {same_environment}\nconfig: {}\ndesired running: {}\nmarker: {}",
        environment.id,
        environment.name,
        environment.status,
        normalized_config(&environment.config),
        persisted_resume_on_startup(environment_dir),
        std::fs::read_to_string(environment_dir.join("persisted.marker"))
            .expect("environment marker must remain readable"),
    )
    .expect("snapshot text must be writable");
}

fn config_value(config: &EnvironmentConfig) -> Value {
    serde_json::to_value(config).expect("environment config must serialize")
}

fn normalized_config(config: &EnvironmentConfig) -> String {
    let mut config = config_value(config);
    config["port"] = Value::String("<PORT>".to_owned());
    serde_json::to_string(&config).expect("normalized environment config must serialize")
}

fn persisted_resume_on_startup(environment_dir: &Path) -> bool {
    let metadata = std::fs::read(environment_dir.join("environment.json"))
        .expect("environment metadata must remain readable");
    serde_json::from_slice::<Value>(&metadata)
        .expect("environment metadata must contain valid JSON")
        .get("resumeOnStartup")
        .and_then(Value::as_bool)
        .expect("environment metadata must contain desired-running state")
}

fn available_local_port() -> u16 {
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .expect("ephemeral local port must be available")
        .local_addr()
        .expect("ephemeral local address must be readable")
        .port()
}

fn write_sleeping_executable(path: &Path) {
    std::fs::write(path, "#!/bin/sh\nexec sleep 60\n")
        .expect("fake Acton executable must be written");
    let mut permissions = std::fs::metadata(path)
        .expect("fake Acton metadata must be available")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).expect("fake Acton executable must be executable");
}
