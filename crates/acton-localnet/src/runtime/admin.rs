//! Administrative edits share the same admission and mutation locks as lifecycle commands.
use super::Runtime;
use crate::{AdminOperation, AdminRequest, Error, Status};
use std::sync::Arc;

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
        let guard = Arc::clone(&entry.mutation)
            .try_lock_owned()
            .map_err(|_| Error::busy())?;
        let network = entry.record.read().await.clone();
        if let Some(driver) = crate::docker::DockerNetwork::load(&entry.data_dir, &network).await?
            && let Some(previous) = driver.saved_admin_operation(Some(&request)).await?
        {
            return Ok(previous);
        }
        if network.status != Status::Running {
            return Err(Error::Conflict {
                code: "admin_unavailable",
                message: "Start the full TON network before editing its state".into(),
            });
        }
        let driver = self.driver(&entry).await?;
        let operation = AdminOperation {
            id: request.id().into(),
            phase: "preparing".into(),
            started_at: chrono::Utc::now().to_rfc3339(),
            finished_at: None,
            error: None,
            block_seqno: None,
        };
        driver.save_admin_operation(&request, &operation).await?;
        *entry.admin_operation.write().await = Some(operation.clone());
        entry.record.write().await.status = Status::Starting;
        Self::save(&entry).await?;
        tokio::spawn(async move {
            let _guard = guard;
            let nodes = entry.record.read().await.nodes.clone();
            let result = driver
                .apply_admin(&nodes, &request, &entry.admin_operation)
                .await;
            let running = result.is_ok() || driver.admin_is_running().await;
            {
                let mut record = entry.record.write().await;
                record.status = if running {
                    Status::Running
                } else {
                    Status::Failed
                };
                record.error = result.as_ref().err().map(ToString::to_string);
            }
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
        let Some(driver) = crate::docker::DockerNetwork::load(&entry.data_dir, &network).await?
        else {
            return Ok(None);
        };
        driver.saved_admin_operation(None).await
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn admin_respects_service_admission_and_the_shared_mutation_lock() {
        let temp = tempfile::tempdir().unwrap();
        let location = crate::catalog::create(
            temp.path(),
            crate::CreateNetwork {
                name: "admin-locks".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let runtime = Runtime::open(&location.path).await.unwrap();
        let request: AdminRequest = serde_json::from_value(serde_json::json!({
            "kind":"accounts", "id":uuid::Uuid::new_v4().to_string(),
            "edits":[{"address":format!("0:{}", "11".repeat(32)), "type":"balance", "balance":"1"}]
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
