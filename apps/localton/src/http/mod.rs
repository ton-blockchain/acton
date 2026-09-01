//! Starts and stops the instance's HTTP services.
//!
//! [`start`] creates the config, admin, and public V2 listeners enabled in
//! [`Settings`]. [`ServiceSet`] stores their task handles, published endpoints,
//! and one shutdown channel. Calling [`ServiceSet::shutdown`] notifies every
//! listener and waits until all Axum tasks finish.

use std::{collections::BTreeMap, net::Ipv4Addr};

use anyhow::Result;
use tokio::{sync::watch, task::JoinHandle};
use tracing::warn;

use crate::{
    runtime::ProcessRegistry,
    storage::{Layout, Settings},
    ton::toolchain::Toolchain,
};

mod admin;
mod config;
mod cors;
mod error;
mod faucet;
mod observability;
mod proxy;
mod server;
pub(crate) mod v2;

pub(super) const FUND_ACCOUNT_PATH: &str = "/acton_fundAccount";

pub struct ServiceSet {
    shutdown: watch::Sender<bool>,
    tasks: Vec<JoinHandle<()>>,
    endpoints: BTreeMap<String, String>,
}

impl ServiceSet {
    pub fn endpoints(&self) -> &BTreeMap<String, String> {
        &self.endpoints
    }

    pub async fn shutdown(mut self) {
        let _ = self.shutdown.send(true);
        // Await by mutable reference so cancellation leaves every handle owned by
        // `self`; Drop can then abort all unfinished tasks instead of detaching
        // handles already moved into this future.
        for task in &mut self.tasks {
            if let Err(error) = task.await {
                warn!(%error, "HTTP service task failed during shutdown");
            }
        }
        self.tasks.clear();
    }
}

impl Drop for ServiceSet {
    fn drop(&mut self) {
        // Startup futures are cancellation-safe, so no detached HTTP task survives
        // the cleanup boundary.
        let _ = self.shutdown.send(true);
        for task in &self.tasks {
            task.abort();
        }
    }
}

pub async fn start(
    layout: &Layout,
    toolchain: &Toolchain,
    processes: &ProcessRegistry,
    settings: &Settings,
    ton_http_api_bind: Ipv4Addr,
) -> Result<ServiceSet> {
    let (shutdown, receiver) = watch::channel(false);
    let mut tasks = Vec::new();
    let mut endpoints = BTreeMap::new();
    if settings.services.config_http.enabled {
        let running = config::start(
            layout.clone(),
            toolchain.clone(),
            settings.clone(),
            receiver.clone(),
        )
        .await?;
        tasks.push(running.task);
        endpoints.insert("config_http".to_owned(), running.endpoint);
    }

    if settings.services.admin_http.enabled {
        let running = admin::start(
            layout.clone(),
            processes.clone(),
            settings,
            receiver.clone(),
        )
        .await?;
        tasks.push(running.task);
        endpoints.insert("admin_http".to_owned(), running.endpoint);
    }

    if settings.services.ton_http_api.enabled {
        let running = proxy::start(settings, ton_http_api_bind, receiver.clone()).await?;
        tasks.push(running.task);
        endpoints.insert("ton_http_api".to_owned(), running.endpoint);
    }

    if settings.services.observability.enabled {
        let running = observability::start(
            layout.clone(),
            toolchain.clone(),
            settings,
            settings.node.public_ip,
            None,
            receiver,
        )
        .await?;
        tasks.push(running.service.task);
        tasks.extend(running.tasks);
        endpoints.insert("observability".to_owned(), running.service.endpoint);
    }

    Ok(ServiceSet {
        shutdown,
        tasks,
        endpoints,
    })
}

/// Starts the network dashboard and host telemetry service for a joined node.
///
/// Joined state does not own bootstrap HTTP services, so its lifecycle uses this
/// entry point to supervise direct network reads and best-effort telemetry delivery
/// through the same shutdown boundary as the node process.
pub async fn start_observability(
    layout: Layout,
    toolchain: Toolchain,
    settings: &Settings,
    advertised_ip: Ipv4Addr,
    collector: Option<String>,
) -> Result<ServiceSet> {
    let (shutdown, receiver) = watch::channel(false);
    let mut tasks = Vec::new();
    let mut endpoints = BTreeMap::new();

    if settings.services.observability.enabled {
        let running = observability::start(
            layout,
            toolchain,
            settings,
            advertised_ip,
            collector,
            receiver,
        )
        .await?;
        tasks.push(running.service.task);
        tasks.extend(running.tasks);
        endpoints.insert("observability".to_owned(), running.service.endpoint);
    }

    Ok(ServiceSet {
        shutdown,
        tasks,
        endpoints,
    })
}

pub(super) struct RunningService {
    task: JoinHandle<()>,
    endpoint: String,
}

#[cfg(test)]
mod tests;
