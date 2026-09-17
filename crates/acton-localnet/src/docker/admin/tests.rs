#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;
use crate::{
    CreateNetwork, NetworkConfig, Runtime,
    activity::{ActivityConfig, ActivityStatus},
    catalog,
    docker::DockerTarget,
    storage,
};

async fn run_edit(runtime: &Runtime, request: AdminRequest) -> Result<u32, Error> {
    let accepted = runtime.start_admin(request.clone()).await?;
    let retried = runtime.start_admin(request).await?;
    if retried.id != accepted.id || !retried.is_active() {
        return Err(failure("Retry did not return the active operation"));
    }

    let mut phase = String::new();
    loop {
        // Inventory reads committed manifests without taking the mutation lock,
        // so Studio can keep polling while the edit and its recovery run.
        runtime.snapshots().await?;

        let operation = runtime
            .admin_operation()
            .await?
            .ok_or_else(|| failure("Lost administrative operation"))?;
        if operation.phase != phase {
            eprintln!("Admin phase: {}", operation.phase);
            phase = operation.phase.clone();
        }
        if operation.finished_at.is_some() {
            return match operation.error {
                Some(error) => Err(failure(error)),
                None => operation
                    .block_seqno
                    .ok_or_else(|| failure("No verified block")),
            };
        }
        sleep(Duration::from_millis(250)).await;
    }
}

#[tokio::test]
async fn operation_ids_survive_restarts_and_reject_different_content() {
    let dir = tempfile::tempdir().unwrap();
    let driver = DockerNetwork {
        compose_file: dir.path().join("compose.yaml"),
        compose_config: NetworkConfig {
            port_base: 0,
            ports: None,
            block_time_ms: None,
            election_time_seconds: None,
            imported_account_bocs: vec![],
            startup_wallets: vec![],
        },
        docker_target: DockerTarget::Context("unused".into()),
        client: Default::default(),
        pull_progress: Default::default(),
        image: "unused".into(),
        project_name: "unused".into(),
        startup_log_file: dir.path().join("startup.log"),
    };
    let request: AdminRequest = serde_json::from_value(serde_json::json!({
        "kind": "accounts",
        "id": Uuid::new_v4().to_string(),
        "edits": [{
            "address": format!("0:{}", "11".repeat(32)),
            "type": "balance",
            "balance": "1"
        }]
    }))
    .unwrap();
    request.validate().unwrap();
    let operation = AdminOperation {
        id: request.id().into(),
        phase: "preparing".into(),
        started_at: chrono::Utc::now().to_rfc3339(),
        finished_at: None,
        error: None,
        block_seqno: None,
    };
    driver
        .save_admin_operation(&request, &operation)
        .await
        .unwrap();
    let interrupted = driver
        .saved_admin_operation(None, None)
        .await
        .unwrap()
        .unwrap();
    assert!(!interrupted.is_active());
    assert_eq!(interrupted.phase, "failed");
    let retry = driver
        .saved_admin_operation(Some(&request), None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retry.finished_at, interrupted.finished_at);
    let mut changed = serde_json::to_value(&request).unwrap();
    changed["edits"][0]["balance"] = "2".into();
    let changed: AdminRequest = serde_json::from_value(changed).unwrap();
    assert!(
        driver
            .saved_admin_operation(Some(&changed), None)
            .await
            .unwrap_err()
            .to_string()
            .contains("different request")
    );
}

#[tokio::test]
#[ignore = "requires Docker and ACTON_LOCALNET_IMAGE built with localton-admin-dev"]
async fn new_nodes_bootstrap_after_repeated_administrative_hardforks() {
    use crate::{OperationStatus, runtime::Action};

    assert!(std::env::var("ACTON_LOCALNET_IMAGE").is_ok());
    let dir = tempfile::tempdir_in("/tmp").unwrap();
    let location = catalog::create(
        dir.path(),
        CreateNetwork {
            name: "hardfork-join-regression".into(),
            port_base: Some(28600),
            block_time_ms: Some(1000),
            election_time_seconds: Some(3600),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let driver = DockerNetwork::materialize(&location.path, dir.path(), &location.network, false)
        .await
        .unwrap();
    eprintln!(
        "Hardfork join project: {} ({})",
        driver.project_name,
        dir.path().display()
    );

    let result: Result<serde_json::Value, Error> = async {
        driver.start_all().await?;
        let runtime = Runtime::open(&location.path).await?;
        runtime.reconcile().await;
        let basechain = format!("0:{}", "42".repeat(32));
        let masterchain = format!("-1:{}", "a4".repeat(32));
        let mut joins = Vec::new();

        for (index, address, balance) in [
            (1, &basechain, "42000000000"),
            (2, &masterchain, "7000000000"),
        ] {
            // The second edit leaves the basechain unchanged. Its state must
            // still be downloadable under the new masterchain bootstrap block.
            let request = serde_json::from_value(serde_json::json!({
                "kind": "accounts",
                "id": Uuid::new_v4().to_string(),
                "edits": [{
                    "address": address,
                    "type": "balance",
                    "balance": balance,
                }],
            }))
            .map_err(failure)?;
            let fork = run_edit(&runtime, request).await?;

            eprintln!("Joining fresh node {index} after hardfork {fork}");
            let accepted = runtime
                .submit(Action::AddNode {
                    name: format!("after-fork-{index}"),
                    validator: false,
                })
                .await?;

            loop {
                let operation = runtime.operation(&accepted.id).await?;
                match operation.status {
                    OperationStatus::Running => sleep(Duration::from_millis(250)).await,
                    OperationStatus::Completed => break,
                    OperationStatus::Failed => {
                        return Err(failure(
                            operation.error.unwrap_or_else(|| "Node join failed".into()),
                        ));
                    }
                }
            }

            let service = format!("node-{index}");
            let native = driver
                .live_admin(&service, &["lite", "account", address], None)
                .await?;
            let preserved = driver
                .live_admin(&service, &["lite", "account", &basechain], None)
                .await?;
            joins.push(serde_json::json!({
                "node": service,
                "balance": native["balance_nano"],
                "preservedBasechainBalance": preserved["balance_nano"],
                "pastHardfork": native["block"]["seqno"].as_u64().is_some_and(|n| n > u64::from(fork)),
            }));
        }

        // Persisted init blocks must also allow all existing databases to reopen.
        driver.stop().await?;
        driver.start_all().await?;

        let mut restarted = Vec::new();
        for service in ["localton", "node-1", "node-2"] {
            let account = driver
                .live_admin(service, &["lite", "account", &basechain], None)
                .await?;
            restarted.push(account["balance_nano"].clone());
        }

        Ok(serde_json::json!({"joins": joins, "restartedBalances": restarted}))
    }
    .await;

    if result.is_err()
        && let Some(diagnostics) = driver.failed_container_diagnostics().await
    {
        eprintln!("{diagnostics}");
    }
    driver.delete().await.unwrap();
    expect_test::expect![[r#"
        Object {
            "joins": Array [
                Object {
                    "balance": String("42000000000"),
                    "node": String("node-1"),
                    "pastHardfork": Bool(true),
                    "preservedBasechainBalance": String("42000000000"),
                },
                Object {
                    "balance": String("7000000000"),
                    "node": String("node-2"),
                    "pastHardfork": Bool(true),
                    "preservedBasechainBalance": String("42000000000"),
                },
            ],
            "restartedBalances": Array [
                String("42000000000"),
                String("42000000000"),
                String("42000000000"),
            ],
        }
    "#]]
    .assert_debug_eq(&result.unwrap());
}

#[tokio::test]
#[ignore = "requires Docker and ACTON_LOCALNET_IMAGE built with localton-admin-dev"]
async fn administrative_hardfork_and_rollback_on_two_nodes() {
    assert!(std::env::var("ACTON_LOCALNET_IMAGE").is_ok());
    let dir = tempfile::tempdir_in("/tmp").unwrap();
    let nodes = vec![Node {
        id: "node-1".into(),
        name: "replica".into(),
        validator: false,
        port_base: 19000,
        stopped: false,
    }];
    let mut location = catalog::create(
        dir.path(),
        CreateNetwork {
            name: "admin-smoke".into(),
            port_base: Some(28300),
            block_time_ms: Some(1000),
            election_time_seconds: Some(3600),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    location.network.nodes = nodes.clone();
    storage::write_json(&location.path.join("network.json"), &location.network)
        .await
        .unwrap();
    let driver = DockerNetwork::materialize(&location.path, dir.path(), &location.network, false)
        .await
        .unwrap();
    eprintln!(
        "Docker admin test project: {} ({})",
        driver.project_name,
        dir.path().display()
    );
    let result: Result<(), Error> = async {
        driver.start_all().await?;
        eprintln!("Complete environment started");

        let address = format!("0:{}", "22".repeat(32));
        let request: AdminRequest = serde_json::from_value(serde_json::json!({
            "kind": "accounts",
            "id": Uuid::new_v4().to_string(),
            "edits": [{
                "address": address,
                "type": "balance",
                "balance": "42000000000"
            }]
        }))
        .unwrap();

        let runtime = Runtime::open(&location.path).await?;
        runtime.reconcile().await;
        runtime
            .configure_activity(ActivityConfig::default(), true)
            .await?;

        let seqno = run_edit(&runtime, request).await?;
        if runtime.activity().await?.status != ActivityStatus::Stopped {
            return Err(failure(
                "Activity generator was not stopped before the hardfork",
            ));
        }

        eprintln!("Hardfork completed at {seqno}");
        let account = driver
            .live_admin("localton", &["lite", "account", &address], None)
            .await?;
        if account["balance_nano"] != "42000000000" {
            return Err(failure(format!("Incorrect native account: {account}")));
        }

        let replica = driver
            .live_admin("node-1", &["lite", "account", &address], None)
            .await?;
        if replica["balance_nano"] != "42000000000" {
            return Err(failure(format!("Incorrect replica account: {replica}")));
        }

        let account_url = format!("http://127.0.0.1:28303/api/v3/accountStates?address={address}");
        let response: serde_json::Value = reqwest::get(&account_url)
            .await
            .map_err(failure)?
            .json()
            .await
            .map_err(failure)?;
        eprintln!("Indexed account: {response}");
        if response["accounts"][0]["balance"] != "42000000000" {
            return Err(failure(format!("Incorrect indexed account: {response}")));
        }

        // Both edits preserve transaction LT. Indexing must still replace
        // the first hardfork's account state with the second one.
        let changed: AdminRequest = serde_json::from_value(serde_json::json!({
            "kind": "accounts",
            "id": Uuid::new_v4().to_string(),
            "edits": [{
                "address": address,
                "type": "balance",
                "balance": "43000000000"
            }]
        }))
        .unwrap();
        run_edit(&runtime, changed).await?;
        let updated: serde_json::Value = reqwest::get(&account_url)
            .await
            .map_err(failure)?
            .json()
            .await
            .map_err(failure)?;
        if updated["accounts"][0]["balance"] != "43000000000" {
            return Err(failure(format!(
                "A second hardfork was not indexed: {updated}"
            )));
        }

        eprintln!("Repeated account overwrite was indexed");

        // A masterchain-only balance edit must update global supply too.
        // Resumed native block production checks the resulting state rules.
        // Keep the fixture separate from the Elector at -1:333...333,
        // whose balance changes as it receives ordinary block rewards.
        let masterchain_address = format!("-1:{}", "a4".repeat(32));
        let masterchain_edit = serde_json::from_value(serde_json::json!({
            "kind": "accounts",
            "id": Uuid::new_v4().to_string(),
            "edits": [{
                "address": masterchain_address,
                "type": "balance",
                "balance": "7000000000"
            }]
        }))
        .unwrap();
        run_edit(&runtime, masterchain_edit).await?;

        for service in ["localton", "node-1"] {
            let account = driver
                .live_admin(service, &["lite", "account", &masterchain_address], None)
                .await?;
            if account["balance_nano"] != "7000000000" {
                return Err(failure(format!(
                    "Masterchain balance edit was not applied on {service}: {account}"
                )));
            }
        }

        eprintln!("Masterchain-only hardfork resumed block production on both nodes");

        // Freezing an uninitialized account fails after snapshots exist,
        // exercising rollback rather than transport-level validation.
        let invalid: AdminRequest = serde_json::from_value(serde_json::json!({
            "kind": "accounts",
            "id": Uuid::new_v4().to_string(),
            "edits": [{"address": address, "type": "freeze"}]
        }))
        .unwrap();
        let error = run_edit(&runtime, invalid)
            .await
            .err()
            .ok_or_else(|| failure("Invalid edit was accepted"))?;
        eprintln!("Expected rejected operation: {error}");

        if !error.to_string().contains("Only an active account") {
            return Err(error);
        }
        if driver.has_admin_recovery() {
            return Err(failure("Recovery journal remains"));
        }
        if !driver.admin_is_running(&nodes).await {
            return Err(failure("Environment did not recover"));
        }

        let account = driver
            .live_admin("localton", &["lite", "account", &address], None)
            .await?;
        if account["balance_nano"] != "43000000000" {
            return Err(failure(format!("Incorrect native account: {account}")));
        }

        drop(runtime);

        for ready in [false, true] {
            driver.stop().await?;
            let mut journal = Recovery::default();
            if ready {
                for service in ["localton", "node-1"] {
                    let directory =
                        format!("{LOCALTON_SNAPSHOT_DIR}/admin/recovery-test/{service}");
                    let snapshot = driver
                        .offline_admin(
                            service,
                            &["snapshot", "create", "--snapshot-dir", &directory],
                            None,
                        )
                        .await?;
                    journal.backups.insert(
                        service.into(),
                        Backup {
                            id: snapshot["id"]
                                .as_str()
                                .ok_or_else(|| failure("Missing snapshot id"))?
                                .into(),
                            directory,
                        },
                    );
                }
                journal.ready = true;
            }
            driver.save_recovery(&journal).await?;
            if ready {
                // Simulate the crash after validator suspension; restoring the
                // cold archives must recover election keys as well as accounts.
                for service in ["localton", "node-1"] {
                    driver
                        .offline_admin(service, &["godmode", "suspend"], None)
                        .await?;
                }
            }
            // No explicit start: opening the new owner must finish recovery.
            let reopened = Runtime::open(&location.path).await?;
            reopened.reconcile().await;
            if reopened.get().await.status != Status::Running || driver.has_admin_recovery() {
                return Err(failure(format!(
                    "Startup recovery did not complete (ready={ready})"
                )));
            }
            for service in ["localton", "node-1"] {
                let account = driver
                    .live_admin(service, &["lite", "account", &address], None)
                    .await?;
                if account["balance_nano"] != "43000000000" {
                    return Err(failure(format!(
                        "Recovery lost account state on {service}: {account}"
                    )));
                }
            }
            eprintln!("Startup recovery restarted both nodes (ready={ready})");
        }

        Ok(())
    }
    .await;

    if result.is_err()
        && let Some(diagnostics) = driver.failed_container_diagnostics().await
    {
        eprintln!("{diagnostics}");
    }
    driver.delete().await.unwrap();
    result.unwrap();
}
