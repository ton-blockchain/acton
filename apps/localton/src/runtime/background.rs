//! Periodic tasks that run while the local network is active.
//!
//! The block monitor records the latest masterchain seqno and observation time
//! in `runtime.json`. Validator maintenance periodically participates in
//! elections and reclaims stakes according to validation settings. All tasks
//! share one shutdown channel and are aborted when the launcher stops.

use std::time::Duration;

use anyhow::Result;
use tokio::{sync::watch, task::JoinHandle, time::sleep};
use tracing::warn;

use crate::{
    cli::StateArgs,
    operations::validators,
    storage::Layout,
    storage::Settings,
    storage::{RuntimeState, unix_time},
    ton::lite::LocalLiteClient,
};

pub struct BackgroundTasks {
    shutdown: watch::Sender<bool>,
    tasks: Vec<JoinHandle<()>>,
}

impl BackgroundTasks {
    pub async fn shutdown(self) {
        let _ = self.shutdown.send(true);
        for task in &self.tasks {
            task.abort();
        }
        for task in self.tasks {
            if let Err(error) = task.await
                && !error.is_cancelled()
            {
                warn!(%error, "background task failed during shutdown");
            }
        }
    }
}

pub fn start(layout: Layout, settings: &Settings) -> BackgroundTasks {
    let (shutdown, receiver) = watch::channel(false);
    let mut tasks = Vec::new();

    if settings.monitoring.enabled {
        let task_layout = layout.clone();
        let interval = settings.monitoring.poll_interval_seconds;
        tasks.push(tokio::spawn(monitor_loop(
            task_layout,
            interval,
            receiver.clone(),
        )));
    }
    if settings.validation.auto_participate || settings.validation.auto_reap {
        let state = StateArgs {
            state_dir: layout.root.clone(),
        };
        let interval = settings.validation.poll_interval_seconds;
        tasks.push(tokio::spawn(validation_loop(
            state,
            interval,
            receiver.clone(),
        )));
    }
    BackgroundTasks { shutdown, tasks }
}

async fn monitor_loop(layout: Layout, interval_seconds: u64, mut shutdown: watch::Receiver<bool>) {
    let interval = Duration::from_secs(interval_seconds);
    loop {
        if let Err(error) = monitor_once(&layout).await {
            warn!(%error, "blockchain monitor iteration failed");
        }
        tokio::select! {
            _ = sleep(interval) => {}
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
        }
    }
}

async fn monitor_once(layout: &Layout) -> Result<()> {
    let mut client = LocalLiteClient::connect(&layout.global_config).await?;
    let last = client.last().await?;
    let seen_at = unix_time();

    RuntimeState::update_atomic(&layout.runtime, |runtime| {
        runtime.masterchain_seqno = Some(last.seqno);
        runtime.last_block_at = Some(seen_at);
        Ok(())
    })?;
    Ok(())
}

async fn validation_loop(
    state: StateArgs,
    interval_seconds: u64,
    mut shutdown: watch::Receiver<bool>,
) {
    let interval = Duration::from_secs(interval_seconds);
    loop {
        if let Err(error) = validators::auto_tick(state.clone()).await {
            warn!(%error, "validator automation iteration failed");
        }
        tokio::select! {
            _ = sleep(interval) => {}
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
        }
    }
}
