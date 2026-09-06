//! Administrative edits share the same admission and mutation locks as lifecycle commands.

use super::Runtime;
use crate::{AdminOperation, AdminRequest, Error, Status, docker::DockerNetwork};
use std::{sync::Arc, time::Instant};

impl Runtime {
    /// Starts a durable edit owned by this service, independent of the HTTP caller.
    pub async fn start_admin(&self, request: AdminRequest) -> Result<AdminOperation, Error> {
        request.validate()?;

        let admission = self.inner.admission.lock().await;
        if !*admission {
            return Err(Error::Conflict {
                code: "service_stopping",
                message: "The localnet service is stopping".into(),
            });
        }

        let entry = self.entry().await?;

        // Admission serializes submissions; the mutation lock belongs to the
        // running task. A retry observes that task without treating it as crashed.
        let previous = entry.admin_request.read().await.clone();
        if let Some(previous) = previous
            && previous.id() == request.id()
        {
            request.check_retry(&previous)?;
            let current = entry.admin_operation.read().await.clone();
            if let Some(operation) = current {
                return Ok(operation);
            }
        }

        let network = entry.record.read().await.clone();
        if let Some(driver) = DockerNetwork::load(&entry.data_dir, &network).await?
            && let Some(previous) = driver.saved_admin_operation(Some(&request)).await?
        {
            return Ok(previous);
        }

        let guard = Arc::clone(&entry.mutation)
            .try_lock_owned()
            .map_err(|_| Error::busy())?;

        if network.status != Status::Running {
            return Err(Error::Conflict {
                code: "admin_unavailable",
                message: "Start the full TON network before editing its state".into(),
            });
        }

        // Every managed node must share the observed head and receive the fork.
        // A stopped node can be behind and must not be restarted implicitly.
        if network.nodes.iter().any(|node| node.stopped) {
            return Err(Error::Conflict {
                code: "admin_node_stopped",
                message:
                    "Start all managed nodes and let them synchronize before editing network state"
                        .into(),
            });
        }

        let driver = self.driver(&entry).await?;
        let mut operation = AdminOperation {
            id: request.id().into(),
            phase: "preparing".into(),
            started_at: chrono::Utc::now().to_rfc3339(),
            finished_at: None,
            error: None,
            block_seqno: None,
        };

        driver.save_admin_operation(&request, &operation).await?;
        entry.record.write().await.status = Status::Starting;
        *entry.admin_request.write().await = Some(request.clone());

        if let Err(error) = Self::save(&entry).await {
            // No worker owns the operation yet. A persistence failure must not
            // leave an active record that blocks every later network action.
            entry.record.write().await.status = network.status;
            operation.phase = "failed".into();
            operation.finished_at = Some(chrono::Utc::now().to_rfc3339());
            operation.error = Some(error.to_string());
            *entry.admin_operation.write().await = Some(operation.clone());

            if let Err(save_error) = driver.save_admin_operation(&request, &operation).await {
                log::error!(
                    "operation=admin id={} target={} outcome=failed error={save_error}",
                    request.id(),
                    entry.data_dir.display()
                );
            }

            return Err(error);
        }

        *entry.admin_operation.write().await = Some(operation.clone());

        let runtime = self.clone();
        tokio::spawn(async move {
            let _guard = guard;
            let started = Instant::now();
            let nodes = entry.record.read().await.nodes.clone();
            let target = entry.data_dir.display();

            log::info!(
                "operation=admin id={} target={target} phase=started nodes={}",
                request.id(),
                nodes.len() + 1
            );

            let result = async {
                runtime.stop_activity().await?;
                driver
                    .apply_admin(&nodes, &request, &entry.admin_operation)
                    .await
            }
            .await;

            let running = result.is_ok() || driver.admin_is_running(&nodes).await;

            {
                let mut record = entry.record.write().await;
                record.status = if running {
                    Status::Running
                } else {
                    Status::Failed
                };
                record.error = result.as_ref().err().map(ToString::to_string);
            }

            log::info!(
                "operation=admin id={} target={target} duration_ms={} outcome={} running={running}",
                request.id(),
                started.elapsed().as_millis(),
                if result.is_ok() {
                    "completed"
                } else {
                    "failed"
                }
            );

            if let Some(op) = entry.admin_operation.write().await.as_mut() {
                op.phase = if result.is_ok() {
                    "completed"
                } else {
                    "failed"
                }
                .into();
                op.finished_at = Some(chrono::Utc::now().to_rfc3339());
                match result {
                    Ok(seqno) => op.block_seqno = Some(seqno),
                    Err(error) => op.error = Some(error.to_string()),
                }
                if let Err(error) = driver.save_admin_operation(&request, op).await {
                    log::error!("Failed to persist administrative operation: {error}");
                }
            }

            if let Err(error) = Self::save(&entry).await {
                log::error!("Failed to persist network after administrative operation: {error}");
            }
        });

        drop(admission);
        Ok(operation)
    }

    /// Reads progress without waiting for the deployment mutation lock.
    pub async fn admin_operation(&self) -> Result<Option<AdminOperation>, Error> {
        let entry = self.entry().await?;
        let current = entry.admin_operation.read().await.clone();
        if let Some(operation) = current {
            return Ok(Some(operation));
        }
        let Ok(_guard) = entry.mutation.try_lock() else {
            return Ok(None);
        };
        let network = entry.record.read().await.clone();
        let Some(driver) = DockerNetwork::load(&entry.data_dir, &network).await? else {
            return Ok(None);
        };
        driver.saved_admin_operation(None).await
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{CreateNetwork, Node, activity::ActivityConfig, catalog, storage};
    use std::time::Duration;
    use tokio::time::timeout;
    use uuid::Uuid;

    #[tokio::test]
    async fn retries_observe_the_owned_operation_while_mutations_are_locked() {
        let temp = tempfile::tempdir().unwrap();
        let location = catalog::create(
            temp.path(),
            CreateNetwork {
                name: "admin-retry".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let runtime = Runtime::open(&location.path).await.unwrap();
        let entry = &runtime.inner.entry;
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
        let mut operation = AdminOperation {
            id: request.id().into(),
            phase: "installing".into(),
            started_at: chrono::Utc::now().to_rfc3339(),
            finished_at: None,
            error: None,
            block_seqno: None,
        };
        *entry.admin_request.write().await = Some(request.clone());
        entry.record.write().await.status = Status::Starting;
        let _guard = entry.mutation.lock().await;
        for completed in [false, true] {
            if completed {
                operation.phase = "completed".into();
                operation.finished_at = Some(chrono::Utc::now().to_rfc3339());
                operation.block_seqno = Some(123);
            }
            *entry.admin_operation.write().await = Some(operation.clone());
            if !completed {
                for start in [false, true] {
                    let result = timeout(
                        Duration::from_secs(1),
                        runtime.configure_activity(ActivityConfig::default(), start),
                    )
                    .await
                    .expect("activity must reject an admin operation without waiting for its lock");
                    assert!(matches!(
                        result,
                        Err(Error::Conflict {
                            code: "operation_in_progress",
                            ..
                        })
                    ));
                }
            }
            let retry = runtime.start_admin(request.clone()).await.unwrap();
            assert_eq!(
                serde_json::to_value(retry).unwrap(),
                serde_json::to_value(&operation).unwrap()
            );
            let mut changed = serde_json::to_value(&request).unwrap();
            changed["edits"][0]["balance"] = "2".into();
            assert!(matches!(
                runtime
                    .start_admin(serde_json::from_value(changed.clone()).unwrap())
                    .await,
                Err(Error::Conflict {
                    code: "admin_id_reused",
                    ..
                })
            ));
            changed["id"] = Uuid::new_v4().to_string().into();
            assert!(matches!(
                runtime
                    .start_admin(serde_json::from_value(changed).unwrap())
                    .await,
                Err(Error::Conflict {
                    code: "operation_in_progress",
                    ..
                })
            ));
        }
        assert!(!location.path.join("runtime.json").exists());
    }

    #[tokio::test]
    async fn historical_retries_do_not_wait_for_or_replace_a_newer_operation() {
        let temp = tempfile::tempdir().unwrap();
        let location = catalog::create(
            temp.path(),
            CreateNetwork {
                name: "historical-retry".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        storage::write_json(
            &location.path.join("runtime.json"),
            &serde_json::json!({
                "version": 2,
                "image": "unused",
                "dockerTarget": {"kind": "context", "value": "unused"},
                "projectName": "unused"
            }),
        )
        .await
        .unwrap();
        let runtime = Runtime::open(&location.path).await.unwrap();
        let driver = DockerNetwork::load(&location.path, &runtime.get().await)
            .await
            .unwrap()
            .unwrap();
        let request = || {
            serde_json::from_value::<AdminRequest>(serde_json::json!({
                "kind": "accounts",
                "id": Uuid::new_v4().to_string(),
                "edits": [{
                    "address": format!("0:{}", "11".repeat(32)),
                    "type": "balance",
                    "balance": "1"
                }]
            }))
            .unwrap()
        };
        let old = request();
        let current = request();
        let operation = |id: &str| AdminOperation {
            id: id.into(),
            phase: "installing".into(),
            started_at: chrono::Utc::now().to_rfc3339(),
            finished_at: None,
            error: None,
            block_seqno: None,
        };
        *runtime.inner.entry.admin_request.write().await = Some(current.clone());
        *runtime.inner.entry.admin_operation.write().await = Some(operation(current.id()));
        let _guard = runtime.inner.entry.mutation.lock().await;
        for interrupted in [false, true] {
            let mut earlier = operation(old.id());
            if !interrupted {
                earlier.phase = "completed".into();
                earlier.finished_at = Some(chrono::Utc::now().to_rfc3339());
                earlier.block_seqno = Some(42);
            }
            driver.save_admin_operation(&old, &earlier).await.unwrap();
            driver
                .save_admin_operation(&current, &operation(current.id()))
                .await
                .unwrap();
            let retry = runtime.start_admin(old.clone()).await.unwrap();
            assert_eq!(retry.id, old.id());
            assert_eq!(
                retry.phase,
                if interrupted { "failed" } else { "completed" }
            );
            assert!(!retry.is_active());
            let mut changed = serde_json::to_value(&old).unwrap();
            changed["edits"][0]["balance"] = "2".into();
            assert!(matches!(
                runtime
                    .start_admin(serde_json::from_value(changed).unwrap())
                    .await,
                Err(Error::Conflict {
                    code: "admin_id_reused",
                    ..
                })
            ));
            let latest: String =
                storage::read_json(&location.path.join("admin-operations/latest.json"))
                    .await
                    .unwrap();
            assert_eq!(latest, current.id());
            assert!(
                runtime
                    .start_admin(current.clone())
                    .await
                    .unwrap()
                    .is_active()
            );
        }
    }

    #[tokio::test]
    async fn admin_rejects_stopped_nodes_before_creating_an_operation() {
        let temp = tempfile::tempdir().unwrap();
        let location = catalog::create(
            temp.path(),
            CreateNetwork {
                name: "admin-stopped-node".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let runtime = Runtime::open(&location.path).await.unwrap();
        {
            let mut network = runtime.inner.entry.record.write().await;
            network.status = Status::Running;
            network.nodes.push(Node {
                id: "node-1".into(),
                name: "replica".into(),
                validator: false,
                port_base: 19000,
                stopped: true,
            });
        }
        let request = serde_json::from_value(serde_json::json!({
            "kind": "accounts",
            "id": Uuid::new_v4().to_string(),
            "edits": [{
                "address": format!("0:{}", "11".repeat(32)),
                "type": "balance",
                "balance": "1"
            }]
        }))
        .unwrap();
        assert!(matches!(
            runtime.start_admin(request).await,
            Err(Error::Conflict {
                code: "admin_node_stopped",
                ..
            })
        ));
        assert!(runtime.admin_operation().await.unwrap().is_none());
        assert_eq!(runtime.get().await.status, Status::Running);
        assert!(!location.path.join("runtime.json").exists());
    }

    #[tokio::test]
    async fn failed_admission_persistence_does_not_leave_an_active_operation() {
        let temp = tempfile::tempdir_in("/tmp").unwrap();
        let location = catalog::create(
            temp.path(),
            CreateNetwork {
                name: "admin-persistence".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        storage::write_json(
            &location.path.join("runtime.json"),
            &serde_json::json!({
                "version": 2,
                "image": "unused",
                "dockerTarget": {"kind": "context", "value": "unused"},
                "projectName": "unused"
            }),
        )
        .await
        .unwrap();
        let runtime = Runtime::open(&location.path).await.unwrap();
        runtime.inner.entry.record.write().await.status = Status::Running;
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

        // A directory at the final filename makes the atomic rename fail on
        // every platform, without relying on permissions or a Docker daemon.
        let network_path = location.path.join("network.json");
        tokio::fs::remove_file(&network_path).await.unwrap();
        tokio::fs::create_dir(&network_path).await.unwrap();
        let failed = runtime.start_admin(request.clone()).await.is_err();
        let operation = runtime.admin_operation().await.unwrap().unwrap();
        let retry = runtime.start_admin(request).await.unwrap();

        let actual = format!(
            "submission failed: {failed}\nnetwork running: {}\noperation phase: {}\nretry active: {}",
            runtime.get().await.status == Status::Running,
            operation.phase,
            retry.is_active()
        );
        expect_test::expect![[r"
            submission failed: true
            network running: true
            operation phase: failed
            retry active: false"]]
        .assert_eq(&actual);
    }

    #[tokio::test]
    async fn admin_respects_service_admission_and_the_shared_mutation_lock() {
        let temp = tempfile::tempdir().unwrap();
        let location = catalog::create(
            temp.path(),
            CreateNetwork {
                name: "admin-locks".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let runtime = Runtime::open(&location.path).await.unwrap();
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
        assert!(matches!(
            runtime.start_admin(request.clone()).await,
            Err(Error::Conflict {
                code: "admin_unavailable",
                ..
            })
        ));
        assert!(!location.path.join("runtime.json").exists());
        {
            let _guard = runtime.inner.entry.mutation.lock().await;
            assert!(matches!(
                runtime.start_admin(request.clone()).await,
                Err(Error::Conflict {
                    code: "operation_in_progress",
                    ..
                })
            ));
            assert!(runtime.admin_operation().await.unwrap().is_none());
        }
        runtime.prepare_shutdown().await.unwrap();
        assert!(matches!(
            runtime.start_admin(request).await,
            Err(Error::Conflict {
                code: "service_stopping",
                ..
            })
        ));
    }
}
