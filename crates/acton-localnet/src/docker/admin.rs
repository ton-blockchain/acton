//! Coordinates a cold, recoverable administrative operation across the cluster.

#[cfg(test)]
mod tests;

use super::{
    DOCKER_METADATA_TIMEOUT, Deserialize, DockerNetwork, Duration, LOCALTON_SNAPSHOT_DIR,
    LOCALTON_STATE_DIR, SNAPSHOT_TIMEOUT, Serialize, Uuid,
};
use crate::{AdminOperation, AdminRequest, Status, admin::phase};
use crate::{Error, Node};
use std::collections::BTreeMap;
use tokio::{
    sync::RwLock,
    time::{Instant, sleep},
};

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
        id: Option<&str>,
    ) -> Result<Option<AdminOperation>, Error> {
        let dir = self.compose_file.with_file_name("admin-operations");
        let id = match id.or_else(|| request.map(AdminRequest::id)) {
            Some(id) => id.to_owned(),
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

        let image = self
            .operation(
                "inspect_image",
                "environment_admin_failed",
                DOCKER_METADATA_TIMEOUT,
                async {
                    self.client()
                        .await?
                        .inspect_image(&self.image)
                        .await
                        .map_err(super::prerequisites::api_error)
                },
            )
            .await?;
        if image
            .config
            .and_then(|c| c.labels)
            .and_then(|l| l.get("org.ton.localton.admin-hardforks").cloned())
            .as_deref()
            != Some("1")
        {
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

        // A newly joined node can still be a few blocks behind when the cluster
        // stops. With validation suspended, let it reach the fixed source head
        // before preparing the edit. Equal-height forks must still be rejected.
        for service in &services[1..] {
            let started = Instant::now();
            loop {
                let theirs = self.wait_live(service, "observe").await?;
                if theirs == head {
                    tracing::info!(
                        operation = "synchronize_admin_head",
                        node = service,
                        target = %head["seqno"],
                        duration_ms = started.elapsed().as_millis(),
                        outcome = "synchronized",
                    );
                    break;
                }

                let behind = theirs["seqno"]
                    .as_u64()
                    .zip(head["seqno"].as_u64())
                    .is_some_and(|(actual, target)| actual < target);
                if !behind || started.elapsed() >= ADMIN_TIMEOUT {
                    return Err(failure(format!(
                        "Node {service} did not reach the suspended head; no hardfork was installed. Expected {head}, observed {theirs}"
                    )));
                }

                tracing::info!(
                    operation = "synchronize_admin_head",
                    node = service,
                    target = %head["seqno"],
                    current = %theirs["seqno"],
                    duration_ms = started.elapsed().as_millis(),
                    outcome = "waiting",
                );
                sleep(Duration::from_secs(1)).await;
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
        self.operation(
            "start_nodes",
            "environment_admin_failed",
            ADMIN_TIMEOUT,
            self.start_services(Some(services), false, false),
        )
        .await
    }

    /// Starts the complete topology and waits for health probes and setup jobs.
    /// Boxing keeps Bollard's large request futures off the enclosing lifecycle stack.
    pub(crate) async fn start_all(&self) -> Result<(), Error> {
        self.operation(
            "start_network",
            "network_start_failed",
            Duration::from_secs(660),
            self.start_services(None, true, true),
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
        let mut command = vec!["/usr/local/bin/localton"];
        command.extend_from_slice(args);
        if args.contains(&"--help") {
            self.exec(service, &command, input.as_deref(), ADMIN_TIMEOUT)
                .await?;
            return Ok(serde_json::Value::Null);
        }
        command.extend_from_slice(&["--state-dir", LOCALTON_STATE_DIR]);
        let duration = if args.first() == Some(&"godmode") {
            Duration::from_secs(900)
        } else {
            Duration::from_secs(90)
        };
        self.admin_json(
            service,
            args,
            self.exec(service, &command, input.as_deref(), duration),
        )
        .await
    }

    async fn offline_admin(
        &self,
        service: &str,
        args: &[&str],
        input: Option<Vec<u8>>,
    ) -> Result<serde_json::Value, Error> {
        let mut command = args.to_vec();
        command.extend_from_slice(&["--state-dir", LOCALTON_STATE_DIR]);
        self.admin_json(
            service,
            args,
            self.offline(service, &command, input.as_deref(), SNAPSHOT_TIMEOUT),
        )
        .await
    }

    /// Records action names and duration without including account payloads in logs.
    async fn admin_json(
        &self,
        service: &str,
        args: &[&str],
        work: impl Future<Output = Result<super::process::Output, Error>>,
    ) -> Result<serde_json::Value, Error> {
        let started = Instant::now();
        let action = args.iter().take(2).copied().collect::<Vec<_>>().join(" ");
        log::info!(
            "operation=admin_command node={service} action={action:?} duration_ms=0 outcome=started"
        );
        let result = async { serde_json::from_slice(&work.await?.stdout).map_err(failure) }.await;
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
