//! Serializes generator commands with network lifecycle operations. The scheduler
//! owns workers and drains cancellation before Docker stops or a snapshot begins.

use super::Runtime;
use crate::AdminOperation;
use crate::{
    Error, Status,
    activity::{
        ActivityConfig, ActivityOutcome, ActivityRun, ActivityState, ActivityStatus, Engine,
        Scenario,
    },
    storage,
};
use rand::Rng;
use std::{
    path::Path,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::{
    sync::watch,
    task::{JoinHandle, JoinSet},
};

#[derive(Default)]
pub(super) struct Control {
    cancel: Option<watch::Sender<bool>>,
    task: Option<JoinHandle<()>>,
}

// Funding occupies the same bounded slots as execution. Prepared wallets retain
// those slots while individual completions immediately release their capacity.
enum Work {
    Ready(Vec<Scenario>),
    Finished(Vec<ActivityRun>),
}

pub(super) async fn load(root: &Path) -> Result<ActivityState, Error> {
    let path = root.join("activity.json");
    let mut state = if path.exists() {
        storage::read_json::<ActivityState>(&path).await?
    } else {
        ActivityState::default()
    };
    state.config.validate()?;
    if matches!(
        state.status,
        ActivityStatus::Running | ActivityStatus::Stopping
    ) {
        state.status = ActivityStatus::Interrupted;
        state.finished_at = Some(now());
        state.active = 0;
        storage::write_json(&path, &state).await?;
    }
    Ok(state)
}

impl Runtime {
    /// Returns saved settings and measured activity without starting a worker.
    pub async fn activity(&self) -> Result<ActivityState, Error> {
        self.entry().await?;
        Ok(self.inner.activity.read().await.clone())
    }

    /// Saves settings, optionally starting a fresh run. The same network lock used
    /// by snapshots and shutdown closes the race between admission and worker spawn.
    pub async fn configure_activity(
        &self,
        config: ActivityConfig,
        start: bool,
    ) -> Result<ActivityState, Error> {
        config.validate()?;
        let admission = self.inner.admission.lock().await;
        if !*admission {
            return Err(Error::Conflict {
                code: "service_stopping",
                message: "The localnet service is stopping".to_owned(),
            });
        }
        let entry = self.entry().await?;
        if entry
            .record
            .read()
            .await
            .operation
            .as_ref()
            .is_some_and(|operation| operation.status == crate::OperationStatus::Running)
        {
            return Err(Error::busy());
        }
        if entry
            .admin_operation
            .read()
            .await
            .as_ref()
            .is_some_and(AdminOperation::is_active)
        {
            return Err(Error::busy());
        }
        // Reconciliation briefly holds this lock for read-only Docker probes.
        // Wait for those probes; admission prevents a new mutation from overtaking us.
        let _mutation = entry.mutation.lock().await;
        let mut control = self.inner.activity_control.lock().await;
        let mut state = self.inner.activity.write().await;
        if matches!(
            state.status,
            ActivityStatus::Running | ActivityStatus::Stopping
        ) {
            return Err(Error::Conflict {
                code: "activity_running",
                message: "Stop activity before changing its settings".to_owned(),
            });
        }
        let network = entry.record.read().await;
        if start && network.status != Status::Running {
            return Err(Error::Conflict {
                code: "network_not_running",
                message: "Start the network before generating activity".to_owned(),
            });
        }
        let engine = if start {
            Some(
                Engine::new(network.endpoints.clone()).map_err(|error| Error::Internal {
                    code: "activity_initialization_failed",
                    message: format!("Could not initialize activity: {error:#}"),
                })?,
            )
        } else {
            None
        };
        let target = network.id.clone();
        drop(network);

        let next = if start {
            ActivityState {
                config,
                status: ActivityStatus::Running,
                run_id: Some(uuid::Uuid::new_v4().to_string()),
                started_at: Some(now()),
                ..Default::default()
            }
        } else {
            ActivityState {
                config,
                ..state.clone()
            }
        };
        storage::write_json(&self.inner.root.join("activity.json"), &next).await?;
        *state = next;
        let response = state.clone();
        drop(state);

        if let Some(engine) = engine {
            // Finished supervisors cannot be reused. Reap them before replacing
            // the cancellation channel so every accepted run has exactly one owner.
            if let Some(task) = control.task.take() {
                let _ = task.await;
            }
            let (cancel, receiver) = watch::channel(false);
            control.cancel = Some(cancel);
            let runtime = self.clone();
            control.task = Some(tokio::spawn(async move {
                runtime.generate_activity(target, engine, receiver).await;
            }));
        }
        drop(control);
        // Admission remains locked until the scheduler owns every accepted start.
        drop(admission);
        Ok(response)
    }

    /// Stops admitting work and cancels outstanding waits. Already submitted
    /// blockchain messages remain valid; cancellation never replays or undoes them.
    pub async fn stop_activity(&self) -> Result<ActivityState, Error> {
        let mut control = self.inner.activity_control.lock().await;
        if let Some(cancel) = control.cancel.take() {
            {
                let mut state = self.inner.activity.write().await;
                if state.status == ActivityStatus::Running {
                    state.status = ActivityStatus::Stopping;
                }
            }
            let _ = cancel.send(true);
        }
        if let Some(task) = control.task.take() {
            task.await.map_err(|error| Error::Internal {
                code: "activity_task_failed",
                message: format!("Activity supervisor stopped unexpectedly: {error}"),
            })?;
        }
        drop(control);
        Ok(self.inner.activity.read().await.clone())
    }

    async fn generate_activity(
        &self,
        target: String,
        engine: Engine,
        mut cancel: watch::Receiver<bool>,
    ) {
        let config = self.inner.activity.read().await.config.clone();
        let started = Instant::now();
        let mut interval =
            tokio::time::interval(Duration::from_secs(config.interval_seconds.into()));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut workers = JoinSet::<Work>::new();
        let mut persistence = tokio::time::interval(Duration::from_secs(1));
        persistence.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut dirty = false;
        let mut next_id = 0u64;
        let mut accepting = true;
        let mut terminal = ActivityStatus::Completed;
        let deadline = tokio::time::sleep(Duration::from_secs(config.duration_seconds.into()));
        tokio::pin!(deadline);
        log::info!("operation=activity target={target} duration_ms=0 outcome=running");

        loop {
            tokio::select! {
                biased;
                _ = cancel.changed(), if terminal != ActivityStatus::Stopped => {
                    accepting = false;
                    terminal = ActivityStatus::Stopped;
                }
                _ = &mut deadline, if accepting && config.duration_seconds != 0 => {
                    accepting = false;
                }
                Some(result) = workers.join_next() => {
                    match result {
                        Ok(Work::Ready(scenarios)) => {
                            for mut scenario in scenarios {
                                let engine = engine.clone();
                                let config = config.clone();
                                let target = target.clone();
                                let mut worker_cancel = cancel.clone();

                                workers.spawn(async move {
                                    let result = tokio::select! {
                                        biased;
                                        _ = async {
                                            if !*worker_cancel.borrow() {
                                                let _ = worker_cancel.changed().await;
                                            }
                                        } => None,
                                        result = engine.run(&config, &mut scenario) => Some(result),
                                    };

                                    Work::Finished(vec![scenario.finish(result, &target)])
                                });
                            }
                        }
                        Ok(Work::Finished(runs)) => {
                            let mut state = self.inner.activity.write().await;
                            for run in runs {
                                state.active = state.active.saturating_sub(1);
                                state.confirmed_messages += run.confirmed_messages;
                                match run.outcome {
                                    ActivityOutcome::Completed => state.completed += 1,
                                    ActivityOutcome::Failed => state.failed += 1,
                                    ActivityOutcome::Cancelled => {}
                                }
                                state.recent.insert(0, run);
                                state.recent.truncate(40);
                            }
                            drop(state);
                            dirty = true;
                        }
                        Err(error) => {
                            log::error!("operation=activity target={target} outcome=worker_failed error={error}");
                            accepting = false;
                            terminal = ActivityStatus::Interrupted;
                            workers.abort_all();
                        }
                    }
                }
                _ = persistence.tick(), if dirty => {
                    // High-rate runs update memory immediately, but checkpoint at
                    // most once per second instead of serializing disk I/O per worker.
                    if let Err(error) = self.save_activity().await {
                        log::error!("operation=activity target={target} outcome=persist_failed error={error}");
                        accepting = false;
                        terminal = ActivityStatus::Interrupted;
                    }
                    dirty = false;
                }
                _ = interval.tick(), if accepting => {
                    let mut state = self.inner.activity.write().await;
                    let capacity = config.concurrency.saturating_sub(state.active);
                    let count = config.scenarios_per_launch.min(capacity);
                    state.skipped += u64::from(config.scenarios_per_launch - count);
                    state.active += count;
                    drop(state);
                    dirty = true;

                    if count == 0 {
                        continue;
                    }

                    let entries = config.scenarios.entries();
                    let total: u16 = entries.iter().map(|(_, weight)| weight).sum();
                    let mut scenarios = Vec::with_capacity(usize::from(count));
                    for _ in 0..count {
                        let mut choice = rand::thread_rng().gen_range(0..total);
                        let scenario = entries.into_iter().find_map(|(scenario, weight)| {
                            if choice < weight {
                                Some(scenario)
                            } else {
                                choice -= weight;
                                None
                            }
                        }).expect("validated nonzero scenario weights");
                        next_id += 1;

                        log::info!("operation=activity_scenario target={target} scenario={scenario:?} id={next_id} duration_ms=0 outcome=running");
                        scenarios.push(Scenario::new(ActivityRun {
                            id: next_id,
                            scenario,
                            started_at: now(),
                            duration_ms: 0,
                            address: None,
                            confirmed_messages: 0,
                            batch_size: None,
                            outcome: ActivityOutcome::Cancelled,
                            error: None,
                        }));
                    }

                    let engine = engine.clone();
                    let config = config.clone();
                    let mut worker_cancel = cancel.clone();
                    let target = target.clone();
                    workers.spawn(async move {
                        let result = engine.fund(&mut scenarios, &config, &mut worker_cancel).await;

                        if matches!(result, Ok(true)) {
                            return Work::Ready(scenarios);
                        }

                        let error = result.err().map(|error| format!("{error:#}"));
                        let mut runs = Vec::with_capacity(scenarios.len());
                        for scenario in scenarios {
                            let result = error.as_ref().map(|error| Err(anyhow::anyhow!(error.clone())));
                            runs.push(scenario.finish(result, &target));
                        }
                        Work::Finished(runs)
                    });
                }
            }
            if !accepting && workers.is_empty() {
                break;
            }
        }

        {
            let mut state = self.inner.activity.write().await;
            state.status = terminal;
            state.active = 0;
            state.finished_at = Some(now());
        }
        if let Err(error) = self.save_activity().await {
            log::error!("operation=activity target={target} outcome=persist_failed error={error}");
        }
        log::info!(
            "operation=activity target={target} duration_ms={} outcome={terminal:?}",
            started.elapsed().as_millis()
        );
    }

    async fn save_activity(&self) -> Result<(), Error> {
        let state = self.inner.activity.read().await.clone();
        storage::write_json(&self.inner.root.join("activity.json"), &state).await
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
