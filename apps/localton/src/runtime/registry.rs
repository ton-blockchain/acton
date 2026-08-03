//! Shared registry of long-running child processes.
//!
//! Processes are keyed by their stable launcher name. The registry prevents
//! duplicate names, reports current PIDs, detects early exits during readiness
//! polling, stops individual processes, and drains every managed process during
//! launcher shutdown.

use std::{collections::BTreeMap, sync::Arc};

use anyhow::{Result, bail};
use serde::Serialize;
use tokio::sync::Mutex;
use utoipa::ToSchema;

use super::ManagedProcess;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ProcessInfo {
    pub name: String,
    pub pid: Option<u32>,
}

#[derive(Clone, Default)]
pub struct ProcessRegistry {
    inner: Arc<Mutex<BTreeMap<String, ManagedProcess>>>,
}

impl ProcessRegistry {
    pub async fn insert(&self, process: ManagedProcess) -> Result<()> {
        let name = process.name().to_owned();
        let mut processes = self.inner.lock().await;
        if processes.contains_key(&name) {
            bail!("process `{name}` is already running");
        }
        processes.insert(name, process);
        Ok(())
    }

    pub async fn contains(&self, name: &str) -> bool {
        self.inner.lock().await.contains_key(name)
    }

    pub async fn info(&self) -> Vec<ProcessInfo> {
        self.inner
            .lock()
            .await
            .values()
            .map(|process| ProcessInfo {
                name: process.name().to_owned(),
                pid: process.id(),
            })
            .collect()
    }

    pub async fn ensure_alive(&self) -> Result<()> {
        let mut processes = self.inner.lock().await;
        for process in processes.values_mut() {
            if let Some(status) = process.try_status()? {
                bail!(
                    "required process `{}` exited early with {status}",
                    process.name()
                );
            }
        }
        Ok(())
    }

    pub async fn stop(&self, name: &str) -> Result<bool> {
        let process = self.inner.lock().await.remove(name);
        if let Some(mut process) = process {
            process.stop().await?;
            return Ok(true);
        }
        Ok(false)
    }

    pub async fn stop_all(&self) -> Result<()> {
        let mut owned = {
            let mut processes = self.inner.lock().await;
            std::mem::take(&mut *processes)
                .into_values()
                .collect::<Vec<_>>()
        };
        let mut first_error = None;
        for process in &mut owned {
            if let Err(error) = process.stop().await
                && first_error.is_none()
            {
                first_error = Some(error);
            }
        }
        if let Some(error) = first_error {
            return Err(error);
        }
        Ok(())
    }
}
