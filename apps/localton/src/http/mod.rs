//! Starts and stops the launcher's HTTP services.
//!
//! [`start`] creates the config, admin, and public V2 listeners enabled in
//! [`Settings`]. [`ServiceSet`] stores their task handles, published endpoints,
//! and one shutdown channel. Calling [`ServiceSet::shutdown`] notifies every
//! listener and waits until all Axum tasks finish.

use std::{
    collections::{BTreeMap, BTreeSet},
    net::Ipv4Addr,
};

use anyhow::Result;
use tokio::{sync::watch, task::JoinHandle};
use tracing::warn;

use crate::{
    bootstrap::LauncherControl,
    storage::{Layout, Settings},
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

    pub async fn shutdown(self) {
        let _ = self.shutdown.send(true);
        for task in self.tasks {
            if let Err(error) = task.await {
                warn!(%error, "HTTP service task failed during shutdown");
            }
        }
    }
}

pub async fn start(
    control: LauncherControl,
    settings: &Settings,
    ton_http_api_bind: Ipv4Addr,
) -> Result<ServiceSet> {
    let (shutdown, receiver) = watch::channel(false);
    let mut tasks = Vec::new();
    let mut endpoints = BTreeMap::new();
    let layout = control.layout().clone();
    let toolchain = control.toolchain();

    if settings.services.config_http.enabled {
        let running = config::start(control.clone(), settings.clone(), receiver.clone()).await?;
        tasks.push(running.task);
        endpoints.insert("config_http".to_owned(), running.endpoint);
    }

    if settings.services.admin_http.enabled {
        let running = admin::start(control, settings, receiver.clone()).await?;
        tasks.push(running.task);
        endpoints.insert("admin_http".to_owned(), running.endpoint);
    }

    if settings.services.ton_http_api.enabled {
        let running = proxy::start(settings, ton_http_api_bind, receiver.clone()).await?;
        tasks.push(running.task);
        endpoints.insert("ton_http_api".to_owned(), running.endpoint);
    }

    if settings.services.observability.enabled {
        let owned_nodes = settings
            .nodes
            .iter()
            .filter(|node| node.enabled)
            .map(|node| node.name.clone())
            .collect();
        let advertised_ip = settings.node("genesis")?.public_ip;
        let running = observability::start(
            layout,
            toolchain,
            settings,
            owned_nodes,
            advertised_ip,
            Vec::new(),
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

pub async fn start_observability(
    layout: Layout,
    toolchain: crate::ton::toolchain::Toolchain,
    settings: &Settings,
    owned_nodes: BTreeSet<String>,
    advertised_ip: Ipv4Addr,
    peers: Vec<String>,
) -> Result<ServiceSet> {
    let (shutdown, receiver) = watch::channel(false);
    let mut tasks = Vec::new();
    let mut endpoints = BTreeMap::new();
    if settings.services.observability.enabled {
        let running = observability::start(
            layout,
            toolchain,
            settings,
            owned_nodes,
            advertised_ip,
            peers,
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
