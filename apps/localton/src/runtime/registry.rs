//! Shared registry of long-running child processes.
//!
//! Processes are keyed by their stable service name. The registry prevents
//! duplicate names, reports current PIDs, detects early exits during readiness
//! polling, stops individual processes, and drains every managed process during
//! instance shutdown.

use std::{collections::BTreeMap, sync::Arc, time::Instant};

use anyhow::{Result, bail};
use serde::Serialize;
use tokio::sync::Mutex;
use tracing::{error, info};
use utoipa::ToSchema;

use super::ServiceHandle;

/// One process that a Localton instance supervises
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ProcessInfo {
    /// Stable process name
    pub name: String,
    /// Current process ID
    pub pid: Option<u32>,
}

/// Shared owner and supervisor of all instance-managed long-running services.
///
/// The registry serializes name registration and health inspection, but removes
/// handles before awaiting shutdown so a slow service cannot block unrelated
/// registry access. Clones refer to the same ownership map and are safe to pass
/// to readiness checks, HTTP control handlers, and instance teardown.
#[derive(Clone, Default)]
pub struct ProcessRegistry {
    inner: Arc<Mutex<BTreeMap<String, ServiceHandle>>>,
}

impl ProcessRegistry {
    /// Registers exclusive lifecycle ownership under the service's stable name.
    ///
    /// Accepting `Into<ServiceHandle>` keeps existing `ManagedProcess` call sites
    /// source-compatible while allowing adapters and tests to provide other
    /// implementations. Duplicate rejection happens before ownership is changed,
    /// so one service name can never ambiguously refer to two live services.
    pub async fn insert(&self, service: impl Into<ServiceHandle>) -> Result<()> {
        let service = service.into();
        let name = service.name().to_owned();
        let mut processes = self.inner.lock().await;
        if processes.contains_key(&name) {
            bail!("process `{name}` is already running");
        }
        processes.insert(name, service);
        Ok(())
    }

    /// Returns a stable name-sorted snapshot without probing service liveness.
    ///
    /// Status probing is kept separate because some implementations require
    /// mutable access and may return an inspection error.
    pub async fn info(&self) -> Vec<ProcessInfo> {
        self.inner
            .lock()
            .await
            .values()
            .map(|process| ProcessInfo {
                name: process.name().to_owned(),
                pid: process.pid(),
            })
            .collect()
    }

    /// Verifies that every required service remains alive without waiting.
    ///
    /// Any exit, including exit code zero, is an early failure because registered
    /// services are expected to outlive instance supervision. The structured log
    /// intentionally contains only the stable name, PID, and log-safe exit value.
    pub async fn ensure_alive(&self) -> Result<()> {
        let mut processes = self.inner.lock().await;
        for process in processes.values_mut() {
            if let Some(status) = process.try_status()? {
                error!(
                    service = %process.name(),
                    pid = process.pid(),
                    status = %status,
                    success = status.success(),
                    code = status.code(),
                    outcome = "early_exit",
                    "required managed service exited early"
                );
                bail!(
                    "required process `{}` exited early with {status}",
                    process.name()
                );
            }
        }
        Ok(())
    }

    /// Drains and stops every registered service in stable name order.
    ///
    /// Shutdown continues after individual failures so one broken implementation
    /// cannot orphan the remaining services. The first error is returned after all
    /// stop attempts, matching the previous process-registry behavior.
    pub async fn stop_all(&self) -> Result<()> {
        let started_at = Instant::now();
        let mut owned = {
            let mut processes = self.inner.lock().await;
            std::mem::take(&mut *processes)
                .into_values()
                .collect::<Vec<_>>()
        };
        let service_count = owned.len();
        info!(services = service_count, "stopping all managed services");
        let mut first_error = None;
        let mut error_count = 0_usize;
        for process in &mut owned {
            if let Err(error) = process.stop().await {
                error_count += 1;
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
        if let Some(error) = first_error {
            error!(
                services = service_count,
                errors = error_count,
                duration_ms = started_at.elapsed().as_millis(),
                outcome = "completed_with_errors",
                "managed service shutdown completed with errors"
            );
            return Err(error);
        }
        info!(
            services = service_count,
            duration_ms = started_at.elapsed().as_millis(),
            outcome = "completed",
            "managed service shutdown completed"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex as StdMutex};

    use anyhow::bail;
    use async_trait::async_trait;

    use super::*;
    use crate::runtime::{ManagedService, ServiceExit};

    struct ControlledService {
        name: String,
        pid: Option<u32>,
        exit: Option<ServiceExit>,
        stop_error: Option<&'static str>,
        stopped: Arc<StdMutex<Vec<String>>>,
    }

    #[async_trait]
    impl ManagedService for ControlledService {
        fn name(&self) -> &str {
            &self.name
        }

        fn pid(&self) -> Option<u32> {
            self.pid
        }

        fn try_status(&mut self) -> Result<Option<ServiceExit>> {
            Ok(self.exit.clone())
        }

        async fn stop(&mut self) -> Result<()> {
            self.stopped.lock().unwrap().push(self.name.clone());
            if let Some(error) = self.stop_error {
                bail!(error);
            }
            Ok(())
        }
    }

    fn controlled(
        name: &str,
        pid: Option<u32>,
        exit: Option<ServiceExit>,
        stop_error: Option<&'static str>,
        stopped: Arc<StdMutex<Vec<String>>>,
    ) -> ServiceHandle {
        ServiceHandle::new(ControlledService {
            name: name.to_owned(),
            pid,
            exit,
            stop_error,
            stopped,
        })
    }

    #[tokio::test]
    async fn registry_supervises_process_independent_service() {
        let stopped = Arc::new(StdMutex::new(Vec::new()));
        let registry = ProcessRegistry::default();
        registry
            .insert(controlled(
                "validator",
                Some(42),
                None,
                None,
                Arc::clone(&stopped),
            ))
            .await
            .unwrap();

        registry.ensure_alive().await.unwrap();
        let info = registry.info().await;
        assert_eq!(info.len(), 1);
        assert_eq!(info[0].name, "validator");
        assert_eq!(info[0].pid, Some(42));
        registry.stop_all().await.unwrap();
        assert!(registry.info().await.is_empty());
        assert_eq!(*stopped.lock().unwrap(), ["validator"]);
    }

    #[tokio::test]
    async fn ensure_alive_reports_process_independent_exit() {
        let registry = ProcessRegistry::default();
        registry
            .insert(controlled(
                "dht",
                None,
                Some(ServiceExit::new(false, Some(7), "exit status: 7")),
                None,
                Arc::new(StdMutex::new(Vec::new())),
            ))
            .await
            .unwrap();

        let error = registry.ensure_alive().await.unwrap_err();
        assert_eq!(
            error.to_string(),
            "required process `dht` exited early with exit status: 7"
        );
    }

    #[tokio::test]
    async fn stop_all_drains_services_even_after_error() {
        let stopped = Arc::new(StdMutex::new(Vec::new()));
        let registry = ProcessRegistry::default();
        registry
            .insert(controlled(
                "a-failing",
                None,
                None,
                Some("stop failed"),
                Arc::clone(&stopped),
            ))
            .await
            .unwrap();
        registry
            .insert(controlled(
                "b-healthy",
                None,
                None,
                None,
                Arc::clone(&stopped),
            ))
            .await
            .unwrap();

        let error = registry.stop_all().await.unwrap_err();
        assert_eq!(error.to_string(), "stop failed");
        assert_eq!(*stopped.lock().unwrap(), ["a-failing", "b-healthy"]);
        assert!(registry.info().await.is_empty());
    }
}
