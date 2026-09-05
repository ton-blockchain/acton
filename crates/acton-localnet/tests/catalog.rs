//! Local discovery and directory preparation never take ownership of other networks.

use acton_localnet::{CreateNetwork, Operation, OperationStatus, catalog};
use expect_test::expect;

#[tokio::test]
async fn concurrent_creation_reserves_distinct_ports() {
    let root = tempfile::tempdir().expect("catalog directory");
    let (first, second) = tokio::join!(
        catalog::create(
            root.path(),
            CreateNetwork {
                name: "first".to_owned(),
                ..Default::default()
            }
        ),
        catalog::create(
            root.path(),
            CreateNetwork {
                name: "second".to_owned(),
                ..Default::default()
            }
        ),
    );
    let first = first.expect("first definition");
    let second = second.expect("second definition");
    expect![["true:2"]].assert_eq(&format!(
        "{}:{}",
        first
            .network
            .config
            .port_base
            .abs_diff(second.network.config.port_base)
            >= 5,
        catalog::list(root.path()).await.expect("catalog").len(),
    ));
}

#[tokio::test]
async fn explicit_endpoint_ports_do_not_collide_with_automatic_defaults() {
    let root = tempfile::tempdir().expect("catalog directory");
    let first = catalog::create(
        root.path(),
        CreateNetwork {
            name: "first".to_owned(),
            ..Default::default()
        },
    )
    .await
    .expect("first network");
    let admin = first.network.config.port_base + 5;
    let second = catalog::create(
        root.path(),
        CreateNetwork {
            name: "custom admin".to_owned(),
            ports: acton_localnet::PortOptions {
                admin: Some(admin),
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .await
    .expect("automatic ports skip the explicit admin port");

    let ports = second.network.config.ports();
    let mut unique = ports.all().to_vec();
    unique.sort_unstable();
    unique.dedup();
    expect![["true:5:true"]].assert_eq(&format!(
        "{}:{}:{}",
        ports.admin == admin,
        unique.len(),
        ports
            .all()
            .iter()
            .all(|port| !first.network.config.ports().all().contains(port))
    ));
}

#[tokio::test]
async fn definitions_validate_genesis_and_reserve_names_and_ports() {
    let root = tempfile::tempdir().expect("catalog directory");
    let network = catalog::create(
        root.path(),
        CreateNetwork {
            name: "demo".to_owned(),
            block_time_ms: Some(400),
            election_time_seconds: Some(30),
            ..Default::default()
        },
    )
    .await
    .expect("network");
    let mut errors = Vec::new();
    for request in [
        CreateNetwork {
            name: "demo".to_owned(),
            ..Default::default()
        },
        CreateNetwork {
            name: "zero".to_owned(),
            block_time_ms: Some(0),
            ..Default::default()
        },
        CreateNetwork {
            name: "election".to_owned(),
            election_time_seconds: Some(3),
            ..Default::default()
        },
        CreateNetwork {
            name: "ports".to_owned(),
            port_base: Some(network.network.config.port_base),
            ..Default::default()
        },
        CreateNetwork {
            name: "overflow".to_owned(),
            port_base: Some(65535),
            ..Default::default()
        },
        CreateNetwork {
            name: "accounts".to_owned(),
            imported_account_bocs: vec!["invalid".to_owned()],
            ..Default::default()
        },
    ] {
        errors.push(
            catalog::create(root.path(), request)
                .await
                .err()
                .expect("validation error")
                .to_string(),
        );
    }
    expect![[r"
        Network demo already exists
        Block time must be greater than zero
        Election time must be at least 4 seconds
        The requested five-port range is unavailable
        The requested five-port range is unavailable
        Imported ShardAccount BoCs must be nonempty hexadecimal strings
    "]]
    .assert_eq(&format!("{}\n", errors.join("\n")));
    let reopened = catalog::list(root.path())
        .await
        .expect("persisted definitions");
    expect![["demo:400:Stopped"]].assert_eq(&format!(
        "{}:{}:{:?}",
        reopened[0].network.name,
        reopened[0]
            .network
            .config
            .block_time_ms
            .expect("block time"),
        reopened[0].network.status
    ));
}

#[tokio::test]
async fn readable_directories_preserve_identity_and_move_only_selected_history() {
    let temp = tempfile::tempdir().expect("catalog directory");
    let root = dunce::canonicalize(temp.path()).expect("canonical root");
    let mut selected = catalog::create(
        &root,
        CreateNetwork {
            name: "Dev net/../Тест".to_owned(),
            ..Default::default()
        },
    )
    .await
    .expect("selected network");
    let other = catalog::create(
        &root,
        CreateNetwork {
            name: "other".to_owned(),
            ..Default::default()
        },
    )
    .await
    .expect("other network");
    let untouched = std::fs::read(other.path.join("network.json")).expect("other definition");
    let expected_directory = selected.path.clone();
    let folder = expected_directory
        .file_name()
        .expect("folder")
        .to_str()
        .expect("UTF-8");
    let (prefix, hash) = folder.rsplit_once('-').expect("name and hash");
    expect![["Dev-net----Тест:16:true"]].assert_eq(&format!(
        "{prefix}:{}:{}",
        hash.len(),
        hash.bytes().all(|b| b.is_ascii_hexdigit())
    ));

    // Reproduce the previous on-disk layout, including historical failure links.
    let old_directory = root.join("networks").join(&selected.network.id);
    std::fs::rename(&selected.path, &old_directory).expect("old directory");
    selected.path = old_directory.clone();
    let log_path = old_directory.join("startup.log").display().to_string();
    let operation = Operation {
        snapshot_id: None,
        snapshot_name: None,
        startup_timings: None,
        id: "operation-1".to_owned(),
        kind: "start".to_owned(),
        phase: "failed".to_owned(),
        status: OperationStatus::Failed,
        started_at: 1,
        duration_ms: 1,
        progress: None,
        completed_steps: Vec::new(),
        error: Some(format!("Failed\nFull log: {log_path}")),
        error_code: None,
        error_status: None,
        log_path,
        result: None,
    };
    selected.network.operation = Some(operation.clone());
    selected.network.error.clone_from(&operation.error);
    std::fs::write(
        old_directory.join("network.json"),
        serde_json::to_vec(&selected.network).expect("record"),
    )
    .expect("record file");
    std::fs::write(old_directory.join("startup.log"), "preserved log").expect("log");
    std::fs::write(old_directory.join("runtime.json"), "pinned Docker identity")
        .expect("descriptor");
    let operations = root.join("operations");
    std::fs::create_dir(&operations).expect("old operation directory");
    std::fs::write(
        operations.join("operation-1.json"),
        serde_json::to_vec(&operation).expect("operation"),
    )
    .expect("operation file");
    std::fs::write(
        operations.join("unrelated.json"),
        serde_json::to_vec(&Operation {
            id: "unrelated".to_owned(),
            log_path: other.path.join("startup.log").display().to_string(),
            ..operation
        })
        .expect("other operation"),
    )
    .expect("other history");

    let prepared = selected
        .prepare(&root)
        .await
        .expect("prepare selected directory");
    let history: Operation = serde_json::from_slice(
        &std::fs::read(prepared.path.join("operations/operation-1.json")).expect("moved history"),
    )
    .expect("history");
    expect![["true:false:true:true:true:true:true"]].assert_eq(&format!(
        "{}:{}:{}:{}:{}:{}:{}",
        prepared.path == expected_directory,
        old_directory.exists(),
        std::path::Path::new(&history.log_path).is_file(),
        history
            .error
            .as_ref()
            .expect("error")
            .contains(&history.log_path),
        operations.join("unrelated.json").is_file(),
        !operations.join("operation-1.json").exists(),
        std::fs::read(other.path.join("network.json")).expect("other record") == untouched,
    ));
    expect![["pinned Docker identity"]].assert_eq(
        &std::fs::read_to_string(prepared.path.join("runtime.json")).expect("same Docker identity"),
    );
    let again = prepared
        .prepare(&root)
        .await
        .expect("idempotent preparation");
    expect![["preserved log"]].assert_eq(
        &std::fs::read_to_string(again.path.join("startup.log")).expect("preserved log"),
    );
}
