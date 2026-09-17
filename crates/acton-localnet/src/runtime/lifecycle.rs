//! Startup readiness and graceful service shutdown.

use super::{Context, Runtime};
use crate::{Error, Status, docker::DockerNetwork};
use std::time::{Duration, Instant};

impl Context {
    pub(super) async fn start(&mut self, driver: &DockerNetwork) -> Result<(), Error> {
        {
            let mut record = self.entry.record.write().await;
            record.status = Status::Starting;
            record.startup_timings = Some(crate::StartupTimings::default());
        }
        self.phase("checkingImage").await?;
        if !self
            .wait_work(driver, driver.image_present(), Duration::from_secs(15))
            .await?
        {
            self.phase("pullingImage").await?;
            self.wait_work(driver, driver.pull(), Duration::from_secs(1800))
                .await?;
        }

        self.phase("startingContainers").await?;
        let result = self.start_containers(driver).await;
        if let Err(error) = result {
            return match driver.stop().await {
                Ok(()) => Err(error),
                Err(cleanup) => Err(Error::Internal {
                    code: "startup_cleanup_failed",
                    message: format!("{error}; graceful cleanup also failed: {cleanup}"),
                }),
            };
        }

        let mut record = self.entry.record.write().await;
        record.status = Status::Running;
        record.error = None;
        drop(record);
        Ok(())
    }

    async fn start_containers(&mut self, driver: &DockerNetwork) -> Result<(), Error> {
        let started = Instant::now();
        self.entry.record.write().await.startup_timings = Some(crate::StartupTimings::default());
        let (readiness, probe) = super::readiness::observe(std::sync::Arc::clone(&self.entry));
        // The probe runs alongside container startup so UI timings include services that
        // became usable before every container became healthy.
        let result = self.finish_startup(driver, started, readiness).await;
        probe.abort();
        let _ = probe.await;
        result
    }

    async fn finish_startup(
        &mut self,
        driver: &DockerNetwork,
        started: Instant,
        mut readiness: tokio::sync::watch::Receiver<crate::OperationProgress>,
    ) -> Result<(), Error> {
        if let Err(error) = self
            .wait_work(driver, driver.start_all(), Duration::from_secs(660))
            .await
        {
            return Err(Error::Internal {
                code: "network_start_failed",
                message: driver
                    .startup_failure_message("start network", &error)
                    .await,
            });
        }
        if let Some(timings) = &mut self.entry.record.write().await.startup_timings {
            timings.containers_ms = Some(started.elapsed().as_millis() as u64);
        }
        self.phase("waitingForApis").await?;
        let deadline = Instant::now() + Duration::from_secs(180);
        let mut closing = self.runtime.inner.closing.subscribe();

        loop {
            let progress = readiness.borrow_and_update().clone();
            let ready = progress.total == Some(progress.completed);
            self.progress(progress).await?;
            if ready {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(Error::Internal {
                    code: "readiness_timeout",
                    message:
                        "TON nodes, APIs or the indexer did not become ready within 180 seconds"
                            .to_owned(),
                });
            }
            tokio::select! {
                changed = readiness.changed() => {
                    changed.map_err(|_| Error::Internal { code: "readiness_failed", message: "Network readiness probe stopped unexpectedly".to_owned() })?;
                },
                _ = async { if !*closing.borrow() { let _ = closing.changed().await; } } => {
                    return Err(Error::Conflict { code: "service_stopping", message: "Readiness checks interrupted by graceful service shutdown".to_owned() });
                }
            }
        }
    }

    async fn wait_work<T>(
        &mut self,
        driver: &DockerNetwork,
        work: impl Future<Output = Result<T, Error>>,
        duration: Duration,
    ) -> Result<T, Error> {
        let mut closing = self.runtime.inner.closing.subscribe();
        self.observe(driver, async {
            tokio::select! {
                result = tokio::time::timeout(duration, work) => result.unwrap_or_else(|_| Err(Error::Internal {
                    code: "docker_timeout", message: format!("Docker operation exceeded {} seconds", duration.as_secs()),
                })),
                _ = async { if !*closing.borrow() { let _ = closing.changed().await; } } => Err(Error::Conflict {
                    code: "service_stopping", message: "Startup interrupted by graceful service shutdown".to_owned(),
                }),
            }
        }).await
    }
}

impl Runtime {
    /// Refreshes cached status for idle networks. Operation progress remains owned
    /// by the active task; this method never overwrites an in-flight transition.
    pub async fn reconcile(&self) {
        let entry = &self.inner.entry;
        let Ok(_guard) = entry.mutation.try_lock() else {
            return;
        };

        if entry.record.read().await.status == Status::Deleted {
            return;
        }

        if !entry.data_dir.join("runtime.json").exists() {
            let mut record = entry.record.write().await;
            if record.status != Status::Failed {
                record.status = Status::Stopped;
            }
            drop(record);
            return;
        }

        let nodes = entry.record.read().await.nodes.clone();
        let result = match self.driver(entry, false).await {
            Ok(driver) => driver.status(&nodes).await,
            Err(error) => Err(error),
        };
        {
            let mut record = entry.record.write().await;
            match result {
                Ok(status) => record.status = status,
                Err(error) => {
                    record.status = Status::Unknown;
                    record.error = Some(error.to_string());
                }
            }
        }

        if let Err(error) = Self::save(entry).await {
            log::error!("operation=reconcile outcome=failed error={error}");
        }
    }

    /// Stops this network while retaining its volumes. Startup processes are
    /// interrupted; snapshot writes reach a safe boundary before Docker stops.
    pub async fn shutdown(&self) -> Result<(), Error> {
        self.prepare_shutdown().await?;
        let entry = &self.inner.entry;
        let _guard = entry.mutation.lock().await;
        self.stop_activity().await?;
        if entry.record.read().await.status == Status::Deleted {
            return Ok(());
        }
        if !entry.data_dir.join("runtime.json").exists() {
            let mut record = entry.record.write().await;
            if record.status != Status::Failed {
                record.status = Status::Stopped;
            }
            drop(record);
            return Self::save(entry).await;
        }

        let started = Instant::now();
        let id = entry.record.read().await.id.clone();
        log::info!("operation=shutdown target={id} duration_ms=0 outcome=running");
        entry.record.write().await.status = Status::Stopping;
        let result = match self.driver(entry, false).await {
            Ok(driver) => driver.stop().await,
            Err(error) => Err(error),
        };
        {
            let mut record = entry.record.write().await;
            record.status = if result.is_ok() {
                Status::Stopped
            } else {
                Status::Failed
            };
            if let Err(error) = &result {
                record.error = Some(error.to_string());
            }
        }
        Self::save(entry).await?;
        log::info!(
            "operation=shutdown target={id} duration_ms={} outcome={}",
            started.elapsed().as_millis(),
            if result.is_ok() { "success" } else { "failed" }
        );
        result
    }
}
