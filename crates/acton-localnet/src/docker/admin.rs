//! Coordinates a cold, recoverable administrative operation across the cluster.

use super::{
    DOCKER_METADATA_TIMEOUT, Deserialize, DockerNetwork, Duration, LOCALTON_SNAPSHOT_DIR,
    LOCALTON_STATE_DIR, SNAPSHOT_TIMEOUT, Serialize, Uuid,
};
use crate::{AdminOperation, AdminRequest, Status, admin::phase};
use crate::{Error, Node};
use std::collections::BTreeMap;
use std::process::Stdio;
use tokio::{
    io::AsyncWriteExt,
    sync::RwLock,
    time::{Instant, sleep},
};
use tokio::{process::Command, time::timeout};

const ADMIN_TIMEOUT: Duration = Duration::from_secs(180);
const JOURNAL: &str = "admin-recovery.json";

#[derive(Serialize, Deserialize, Default)]
struct Recovery {
    ready: bool,
    backups: BTreeMap<String, Backup>,
}

#[derive(Serialize, Deserialize)]
struct Backup {
    id: String,
    directory: String,
}

fn failure(error: impl std::fmt::Display) -> Error {
    Error::Internal {
        code: "environment_admin_failed",
        message: error.to_string(),
    }
}

#[derive(Serialize, Deserialize)]
struct SavedOperation {
    request: AdminRequest,
    operation: AdminOperation,
}

impl DockerNetwork {
    pub(crate) async fn save_admin_operation(
        &self,
        request: &AdminRequest,
        operation: &AdminOperation,
    ) -> Result<(), Error> {
        self.save_admin_record(request, operation).await?;

        let latest = self
            .compose_file
            .with_file_name("admin-operations")
            .join("latest.json");
        let temp = latest.with_extension("json.tmp");
        tokio::fs::write(&temp, serde_json::to_vec(&operation.id).map_err(failure)?)
            .await
            .map_err(failure)?;
        tokio::fs::rename(temp, latest).await.map_err(failure)
    }

    async fn save_admin_record(
        &self,
        request: &AdminRequest,
        operation: &AdminOperation,
    ) -> Result<(), Error> {
        let dir = self.compose_file.with_file_name("admin-operations");
        tokio::fs::create_dir_all(&dir).await.map_err(failure)?;

        let path = dir.join(format!("{}.json", operation.id));
        let temp = path.with_extension("json.tmp");
        let record = SavedOperation {
            request: request.clone(),
            operation: operation.clone(),
        };

        tokio::fs::write(&temp, serde_json::to_vec(&record).map_err(failure)?)
            .await
            .map_err(failure)?;
        tokio::fs::rename(temp, &path).await.map_err(failure)
    }

    pub(crate) async fn saved_admin_operation(
        &self,
        request: Option<&AdminRequest>,
    ) -> Result<Option<AdminOperation>, Error> {
        let dir = self.compose_file.with_file_name("admin-operations");
        let id = match request {
            Some(request) => request.id().to_owned(),
            None => match tokio::fs::read(dir.join("latest.json")).await {
                Ok(bytes) => serde_json::from_slice::<String>(&bytes).map_err(failure)?,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(e) => return Err(failure(e)),
            },
        };

        Uuid::parse_str(&id).map_err(failure)?;
        let bytes = match tokio::fs::read(dir.join(format!("{id}.json"))).await {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(failure(e)),
        };

        let saved: SavedOperation = serde_json::from_slice(&bytes).map_err(failure)?;
        if let Some(request) = request {
            request.check_retry(&saved.request)?;
        }

        let mut op = saved.operation;
        if op.is_active() {
            op.phase = "failed".into();
            op.finished_at = Some(chrono::Utc::now().to_rfc3339());
            op.error = Some(concat!(
                "The localnet service restarted before the operation result was recorded. ",
                "Recovery ran before startup. Inspect the account before submitting another edit"
            ).into());

            // An old interrupted ID can be retried while a newer operation runs.
            // Finalizing its record must not replace that operation's latest ID.
            self.save_admin_record(&saved.request, &op).await?;
        }

        Ok(Some(op))
    }

    pub(crate) async fn admin_is_running(&self, nodes: &[Node]) -> bool {
        if self.has_admin_recovery() {
            return false;
        }

        self.status(nodes)
            .await
            .is_ok_and(|status| status == Status::Running)
    }

    pub(crate) fn has_admin_recovery(&self) -> bool {
        self.compose_file.with_file_name(JOURNAL).exists()
    }

    async fn save_recovery(&self, journal: &Recovery) -> Result<(), Error> {
        let path = self.compose_file.with_file_name(JOURNAL);
        let temp = path.with_extension("json.tmp");
        tokio::fs::write(&temp, serde_json::to_vec(journal).map_err(failure)?)
            .await
            .map_err(failure)?;
        tokio::fs::rename(temp, path).await.map_err(failure)
    }

    /// Called before any ordinary startup, including after a Studio crash.
    pub(crate) async fn recover_admin(&self) -> Result<(), Error> {
        let path = self.compose_file.with_file_name(JOURNAL);
        if !path.exists() {
            return Ok(());
        }

        let journal: Recovery =
            serde_json::from_slice(&tokio::fs::read(&path).await.map_err(failure)?)
                .map_err(failure)?;
        self.stop().await?;

        if journal.ready {
            for (service, backup) in &journal.backups {
                self.offline_admin(
                    service,
                    &[
                        "snapshot",
                        "restore",
                        &backup.id,
                        "--snapshot-dir",
                        &backup.directory,
                    ],
                    None,
                )
                .await?;
            }
            self.reset_indexer().await?;
        }

        // Opening the owner only reconciles Docker; it does not start services.
        // Retain the journal until the entire deployment is back, even when the
        // crash happened before backups were ready and no state was changed.
        self.start_all().await?;

        // Keep recovery archives in their own namespace; a joined-node archive must
        // never appear as a restorable genesis snapshot in the Studio snapshot list.
        tokio::fs::remove_file(path).await.map_err(failure)
    }

    pub(crate) async fn apply_admin(
        &self,
        nodes: &[Node],
        request: &AdminRequest,
        operation: &RwLock<Option<AdminOperation>>,
    ) -> Result<u32, Error> {
        // Probe before stopping anything: saved environments may use older images.
        self.live_admin("localton", &["godmode", "prepare", "--help"], None)
            .await
            .map_err(|error| failure(format!(
                "Could not verify administrative hardfork support in this environment's Localton image: {error}"
            )))?;

        let mut inspect = self.docker_command();
        inspect.args([
            "image",
            "inspect",
            "--format",
            "{{ index .Config.Labels \"org.ton.localton.admin-hardforks\" }}",
            &self.image,
        ]);
        let image = self
            .command_output(
                inspect,
                "check hardfork indexer support",
                "environment_admin_failed",
                DOCKER_METADATA_TIMEOUT,
            )
            .await?;
        if String::from_utf8_lossy(&image.stdout).trim() != "1" {
            return Err(failure(
                "This Localton image does not support account indexing after a hardfork. Create an environment with a compatible image",
            ));
        }
        if self.has_admin_recovery() {
            self.recover_admin().await?;
        }

        let mut services = vec!["localton".to_owned()];
        services.extend(nodes.iter().map(|node| node.id.clone()));

        phase(operation, "stopping").await;

        // A failed stop can leave only part of the cluster running. Include it
        // in recovery even though no account state has been changed yet.
        let result = async {
            self.stop().await?;
            self.admin_work(&services, request, operation).await
        }
        .await;

        if let Err(error) = result {
            phase(operation, "restoring").await;
            let recovery = if self.has_admin_recovery() {
                self.recover_admin().await
            } else {
                self.start_all().await
            };

            if let Err(restore) = recovery {
                return Err(failure(format!(
                    "{error}. Recovery also failed: {restore}. Cold backups and the recovery journal have been retained."
                )));
            }

            return Err(error);
        }

        result
    }

    async fn admin_work(
        &self,
        services: &[String],
        request: &AdminRequest,
        operation: &RwLock<Option<AdminOperation>>,
    ) -> Result<u32, Error> {
        phase(operation, "backingUp").await;
        let mut journal = Recovery::default();
        self.save_recovery(&journal).await?;

        for service in services {
            let name = format!("Before edit {} ({service})", request.id());
            let directory = format!("{LOCALTON_SNAPSHOT_DIR}/admin/{}/{service}", request.id());
            let snapshot = self
                .offline_admin(
                    service,
                    &[
                        "snapshot",
                        "create",
                        "--snapshot-dir",
                        &directory,
                        "--name",
                        &name,
                    ],
                    None,
                )
                .await?;
            let id = snapshot["id"]
                .as_str()
                .ok_or_else(|| failure("Snapshot response has no id"))?;
            journal.backups.insert(
                service.clone(),
                Backup {
                    id: id.into(),
                    directory,
                },
            );
            self.save_recovery(&journal).await?;
        }

        // Recovery can restore the cluster only after every node has a backup.
        // Persist that boundary before changing validator configuration.
        journal.ready = true;
        self.save_recovery(&journal).await?;

        phase(operation, "suspending").await;
        for service in services {
            self.offline_admin(service, &["godmode", "suspend"], None)
                .await?;
        }

        self.start_core(services).await?;
        let head = self.wait_live("localton", "observe").await?;

        // One shared plan must be grafted onto the same head on every node.
        for service in &services[1..] {
            let theirs = self.wait_live(service, "observe").await?;
            if theirs != head {
                return Err(failure(format!(
                    "Node {service} has a different head; no hardfork was installed"
                )));
            }
        }

        phase(operation, "building").await;
        let AdminRequest::Accounts { edits, .. } = request;
        let plan = self
            .live_admin(
                "localton",
                &["godmode", "prepare"],
                Some(serde_json::to_vec(edits).map_err(failure)?),
            )
            .await?;
        let seqno = plan["masterchain"]["seqno"]
            .as_u64()
            .ok_or_else(|| failure("Invalid hardfork plan"))? as u32;
        let encoded = serde_json::to_vec(&plan).map_err(failure)?;

        self.stop().await?;
        phase(operation, "installing").await;

        for service in services {
            self.offline_admin(service, &["godmode", "install", "-"], Some(encoded.clone()))
                .await?;
        }

        self.start_core(services).await?;
        phase(operation, "verifying").await;

        for service in services {
            self.wait_live(service, "verify").await?;
        }

        self.stop().await?;
        for service in services {
            self.offline_admin(service, &["godmode", "finish"], None)
                .await?;
            self.offline_admin(service, &["godmode", "resume"], None)
                .await?;
        }

        phase(operation, "resuming").await;
        self.start_core(services).await?;

        // Acceptance is insufficient: exercise ordinary collation after the edit.
        let deadline = Instant::now() + ADMIN_TIMEOUT;
        loop {
            let mut all = true;
            for service in services {
                match self.live_admin(service, &["lite", "last"], None).await {
                    Ok(current) => {
                        all &= current["seqno"]
                            .as_u64()
                            .is_some_and(|n| n >= u64::from(seqno) + 2)
                    }
                    Err(_) => all = false,
                }
            }
            if all {
                break;
            }
            if Instant::now() >= deadline {
                return Err(failure(
                    "Ordinary block production did not resume after the edit",
                ));
            }
            sleep(Duration::from_secs(1)).await;
        }

        phase(operation, "indexing").await;
        self.start_all().await?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(failure)?;
        let url = format!(
            "http://127.0.0.1:{}/api/v3/masterchainInfo",
            self.compose_config.ports().api_v3
        );
        let deadline = Instant::now() + ADMIN_TIMEOUT;
        loop {
            if let Ok(response) = client.get(&url).send().await
                && let Ok(value) = response.json::<serde_json::Value>().await
                && value
                    .pointer("/last/seqno")
                    .and_then(serde_json::Value::as_u64)
                    .is_some_and(|n| n >= u64::from(seqno))
            {
                break;
            }
            if Instant::now() >= deadline {
                return Err(failure("Indexer did not reach the administrative block"));
            }
            sleep(Duration::from_secs(1)).await;
        }
        tokio::fs::remove_file(self.compose_file.with_file_name(JOURNAL))
            .await
            .map_err(failure)?;
        Ok(seqno)
    }

    async fn start_core(&self, services: &[String]) -> Result<(), Error> {
        let mut command = self.compose_command();
        command.args(["up", "-d", "--no-deps"]).args(services);
        self.run_command(
            command,
            "start network nodes",
            "environment_admin_failed",
            ADMIN_TIMEOUT,
        )
        .await
    }

    async fn start_all(&self) -> Result<(), Error> {
        let mut command = self.compose_command();
        command.args(["up", "-d", "--wait", "--wait-timeout", "600"]);
        self.run_command(
            command,
            "restart the complete environment",
            "environment_admin_failed",
            Duration::from_secs(660),
        )
        .await
    }

    async fn wait_live(&self, service: &str, action: &str) -> Result<serde_json::Value, Error> {
        let deadline = Instant::now() + ADMIN_TIMEOUT;
        loop {
            match self.live_admin(service, &["godmode", action], None).await {
                Ok(value) => return Ok(value),
                Err(error) if Instant::now() >= deadline => return Err(error),
                Err(_) => sleep(Duration::from_secs(1)).await,
            }
        }
    }

    async fn live_admin(
        &self,
        service: &str,
        args: &[&str],
        input: Option<Vec<u8>>,
    ) -> Result<serde_json::Value, Error> {
        let mut command = self.compose_command();
        command
            .args(["exec", "-T", service, "/usr/local/bin/localton"])
            .args(args);

        // --help is intentionally not a JSON command.
        if args.contains(&"--help") {
            self.run_command(
                command,
                "check administrator support",
                "environment_admin_failed",
                ADMIN_TIMEOUT,
            )
            .await?;
            return Ok(serde_json::Value::Null);
        }

        command.args(["--state-dir", LOCALTON_STATE_DIR]);
        self.admin_json(
            command,
            service,
            args,
            input,
            if args.first() == Some(&"godmode") {
                Duration::from_secs(900)
            } else {
                Duration::from_secs(90)
            },
        )
        .await
    }

    async fn offline_admin(
        &self,
        service: &str,
        args: &[&str],
        input: Option<Vec<u8>>,
    ) -> Result<serde_json::Value, Error> {
        let mut command = self.docker_command();
        let state_volume = format!(
            "{}_{}-state:{LOCALTON_STATE_DIR}",
            self.project_name, service
        );
        let backups_volume = format!(
            "{}_localton-snapshots:{LOCALTON_SNAPSHOT_DIR}",
            self.project_name
        );
        command
            .args([
                "run",
                "--rm",
                "-i",
                "--network",
                "none",
                "--volume",
                &state_volume,
                "--volume",
                &backups_volume,
                "--entrypoint",
                "/usr/local/bin/localton",
                &self.image,
            ])
            .args(args)
            .args(["--state-dir", LOCALTON_STATE_DIR]);
        self.admin_json(command, service, args, input, SNAPSHOT_TIMEOUT)
            .await
    }

    /// Streams request data separately from command diagnostics. Both pipes must
    /// drain together, and an early process failure takes priority over broken stdin.
    async fn admin_json(
        &self,
        mut command: Command,
        service: &str,
        args: &[&str],
        input: Option<Vec<u8>>,
        command_timeout: Duration,
    ) -> Result<serde_json::Value, Error> {
        let started = Instant::now();
        // Only command names belong in progress logs, never the request payload.
        let action = args.iter().take(2).copied().collect::<Vec<_>>().join(" ");
        log::info!("operation=admin_command node={service} action={action:?} phase=started");

        command
            .kill_on_drop(true)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let result = timeout(command_timeout, async {
            let mut child = command.spawn().map_err(failure)?;
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| failure("Missing command stdin"))?;
            let write = async move {
                if let Some(input) = input {
                    stdin.write_all(&input).await?;
                }
                drop(stdin);
                Ok::<_, std::io::Error>(())
            };
            let (written, output) = tokio::join!(write, child.wait_with_output());
            let output = output.map_err(failure)?;
            if !output.status.success() {
                return Err(failure(format!(
                    "Command exited with {}: {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr)
                )));
            }

            written.map_err(failure)?;
            serde_json::from_slice(&output.stdout).map_err(failure)
        })
        .await
        .map_err(failure)
        .and_then(|result| result);

        log::info!(
            "operation=admin_command node={service} action={action:?} duration_ms={} outcome={}",
            started.elapsed().as_millis(),
            if result.is_ok() {
                "completed"
            } else {
                "failed"
            }
        );

        result.map_err(|error| failure(format!("{service}: {action}: {error}")))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{
        CreateNetwork, NetworkConfig, Runtime,
        activity::{ActivityConfig, ActivityStatus},
        catalog,
        docker::DockerTarget,
        storage,
    };

    #[tokio::test]
    async fn admin_command_preserves_process_diagnostics_when_stdin_closes_early() {
        let dir = tempfile::tempdir().unwrap();
        let driver = DockerNetwork {
            compose_file: dir.path().join("compose.yaml"),
            compose_config: NetworkConfig {
                port_base: 0,
                ports: None,
                block_time_ms: None,
                election_time_seconds: None,
                imported_account_bocs: vec![],
            },
            docker_target: DockerTarget::Context("unused".into()),
            isolated_docker_config_dir: None,
            image: "unused".into(),
            project_name: "unused".into(),
            startup_log_file: dir.path().join("startup.log"),
        };

        // Closing stdin before consuming a large payload forces a broken pipe.
        // The actionable error still comes from the child process's stderr.
        let mut command = Command::new("sh");
        command.args(["-c", "exec 0<&-; echo snapshot restore failed >&2; exit 1"]);
        let error = driver
            .admin_json(
                command,
                "node-1",
                &["snapshot", "restore"],
                Some(vec![0; 1024 * 1024]),
                Duration::from_secs(5),
            )
            .await
            .unwrap_err();

        expect_test::expect![["node-1: snapshot restore: Command exited with exit status: 1: snapshot restore failed\n"]]
            .assert_eq(&error.to_string());
    }

    async fn run_edit(runtime: &Runtime, request: AdminRequest) -> Result<u32, Error> {
        let accepted = runtime.start_admin(request.clone()).await?;
        let retried = runtime.start_admin(request).await?;
        if retried.id != accepted.id || !retried.is_active() {
            return Err(failure("Retry did not return the active operation"));
        }
        if !matches!(runtime.snapshots().await, Err(Error::Conflict { .. })) {
            return Err(failure(
                "Snapshots were not excluded during an administrative operation",
            ));
        }
        let mut phase = String::new();
        loop {
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
            },
            docker_target: DockerTarget::Context("unused".into()),
            isolated_docker_config_dir: None,
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
        let interrupted = driver.saved_admin_operation(None).await.unwrap().unwrap();
        assert!(!interrupted.is_active());
        assert_eq!(interrupted.phase, "failed");
        let retry = driver
            .saved_admin_operation(Some(&request))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(retry.finished_at, interrupted.finished_at);
        let mut changed = serde_json::to_value(&request).unwrap();
        changed["edits"][0]["balance"] = "2".into();
        let changed: AdminRequest = serde_json::from_value(changed).unwrap();
        assert!(
            driver
                .saved_admin_operation(Some(&changed))
                .await
                .unwrap_err()
                .to_string()
                .contains("different request")
        );
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
        let driver = DockerNetwork::materialize(&location.path, dir.path(), &location.network)
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

            let account_url =
                format!("http://127.0.0.1:28303/api/v3/accountStates?address={address}");
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

        if result.is_err() {
            let mut command = driver.compose_command();
            command.args(["logs", "--tail", "35", "v3-worker", "localton", "node-1"]);
            if let Ok(output) = command.output().await {
                eprintln!("{}", String::from_utf8_lossy(&output.stdout));
            }
        }
        driver.delete().await.unwrap();
        result.unwrap();
    }
}
