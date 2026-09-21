//! Shared scheduling for interchangeable endpoints. Available since trunk.
//!
//! An attempt owns a complete, replayable operation on one endpoint. Adapters
//! validate responses and classify failures before returning them to the pool.
//! Connection management and protocol cancellation remain adapter responsibilities.

#[cfg(test)]
mod tests;

mod state;

use std::{
    collections::{BTreeMap, HashSet},
    future::Future,
    io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use futures::{
    StreamExt,
    stream::{self, FuturesUnordered},
};
use serde::{Deserialize, Serialize};
use tokio::{sync::Notify, task::JoinSet, time::Instant};
use tracing::{debug, info, warn};

use state::{Lease, ProbeKind, State};

/// Stable identity shared by all connections to the same upstream service.
/// The pool namespace separates networks; IDs must not depend on connection order.
pub trait Endpoint: Send + Sync {
    /// Identifies the upstream across reconnects and discovery refreshes.
    fn id(&self) -> &str;

    /// Describes its address for diagnostics and detects address changes.
    /// Changing this value invalidates latency estimates for the previous route.
    fn address(&self) -> String;
}

/// An adapter's decision about a failed attempt. Local storage or caller errors
/// must be fatal so they do not penalize unrelated upstream servers.
#[derive(Debug, Clone, thiserror::Error)]
pub enum Failure {
    /// This endpoint lacks the requested data. Other endpoints may have it.
    #[error("data unavailable: {0}")]
    Unavailable(String),
    /// A transport failure or timeout permits retry and temporarily suspends the endpoint.
    #[error("request failed: {0}")]
    Retryable(String),
    /// The response failed validation; suspend the endpoint longer than a transport error.
    #[error("invalid response: {0}")]
    Invalid(String),
    /// Retrying cannot resolve this failure. Stop all attempts immediately.
    #[error("operation failed: {0}")]
    Fatal(String),
}

/// Bounds endpoint attempts, including speculative copies. Callers retain
/// responsibility for bounding the number of logical operations they enqueue.
#[derive(Debug, Clone)]
pub struct Options {
    /// Shared across request classes and clones of the pool.
    pub max_in_flight: usize,
    /// Shared across all connections and request classes for an endpoint.
    pub max_in_flight_per_endpoint: usize,
    /// Minimum spacing between dispatches to one endpoint, shared across classes
    /// and clones. Zero disables pacing. Adapters choose this from upstream limits
    /// and the number of protocol requests in one operation.
    pub min_request_interval: Duration,
    /// Maximum distinct endpoints tried by one logical operation.
    pub max_attempts: usize,
    /// Includes the primary attempt and delayed speculative attempts.
    pub max_parallel_attempts: usize,
    /// Bounds one dispatched attempt; waiting for capacity is excluded.
    pub attempt_timeout: Duration,
    /// Bounds the entire operation, including waiting for capacity and retries.
    pub request_timeout: Duration,
    /// Delay before adding another endpoint while existing attempts are pending.
    pub hedge_after: Duration,
    /// Background measurements share normal admission, leaving one slot for callers.
    /// Zero disables probes. Each class permits one recovery and one discovery
    /// probe, so discovery timeouts cannot monopolize that class's measurements.
    pub max_background_probes: usize,
    /// Minimum spacing between probes of the same class and kind, and between
    /// measurements of an individual endpoint.
    pub probe_interval: Duration,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            max_in_flight: 16,
            max_in_flight_per_endpoint: 4,
            min_request_interval: Duration::ZERO,
            max_attempts: 16,
            max_parallel_attempts: 2,
            attempt_timeout: Duration::from_secs(30),
            request_timeout: Duration::from_secs(30),
            hedge_after: Duration::from_millis(100),
            max_background_probes: 4,
            probe_interval: Duration::from_secs(1),
        }
    }
}

/// Measurements for one endpoint and request class. Cancellation is counted
/// separately and never treated as a successful latency sample or a failure.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Metrics {
    pub successes: u64,
    pub failures: u64,
    pub unavailable: u64,
    pub cancelled: u64,
    /// Exponentially weighted mean of successful attempts, excluding admission
    /// and caller work. Each new sample contributes 20% after the first response.
    pub latency_ms: Option<f64>,
    /// A cancelled attempt did not finish within this time. This is a routing
    /// penalty, not a successful latency sample; a completed response clears it.
    #[serde(default)]
    pub latency_lower_bound_ms: Option<f64>,
    /// Unix timestamp of the latest latency sample or cancellation bound.
    pub updated_at: u64,
}

/// Diagnostic view of one upstream. Restoring a snapshot never restores active
/// requests, connections, or suspensions from a previous process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointSnapshot {
    pub id: String,
    pub address: String,
    pub in_flight: usize,
    pub classes: BTreeMap<String, Metrics>,
}

/// Portable statistics, scoped to a caller-defined network and protocol identity.
/// Discovery must still provide endpoints before restored statistics can be used.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub version: u32,
    pub namespace: String,
    pub endpoints: Vec<EndpointSnapshot>,
}

struct Inner<E> {
    namespace: String,
    options: Options,
    state: Mutex<State<E>>,
    changed: Notify,
    saved_at: tokio::sync::Mutex<Option<Instant>>,
}

/// Concurrent pool with one shared scheduler and per-class measurements.
/// Clones share limits and statistics. No scheduler lock is held across an await.
pub struct Pool<E> {
    inner: Arc<Inner<E>>,
    snapshot_path: Option<Arc<PathBuf>>,
    // Tasks retain Inner, not Pool: dropping the last pool cancels its probes.
    probes: Arc<Mutex<JoinSet<()>>>,
}

impl<E> Clone for Pool<E> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            snapshot_path: self.snapshot_path.clone(),
            probes: Arc::clone(&self.probes),
        }
    }
}

impl<E: Endpoint> Pool<E> {
    /// Creates an empty pool. Zero limits or timeouts are rejected.
    pub fn new(namespace: String, options: Options) -> Result<Self, Failure> {
        if options.max_in_flight == 0
            || options.max_in_flight_per_endpoint == 0
            || options.max_attempts == 0
            || options.max_parallel_attempts == 0
            || options.attempt_timeout.is_zero()
            || options.request_timeout.is_zero()
            || options.hedge_after.is_zero()
            || options.probe_interval.is_zero()
        {
            return Err(Failure::Fatal(
                "pool limits and timeouts must be positive".into(),
            ));
        }

        Ok(Self {
            inner: Arc::new(Inner {
                namespace,
                options,
                state: Mutex::new(State::default()),
                changed: Notify::new(),
                saved_at: tokio::sync::Mutex::new(None),
            }),
            snapshot_path: None,
            probes: Arc::new(Mutex::new(JoinSet::new())),
        })
    }

    /// Adds or refreshes an upstream without interrupting existing attempts.
    /// Discovery refreshes preserve statistics while the address is unchanged.
    pub fn upsert(&self, endpoint: E) {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .upsert(endpoint);
        self.inner.changed.notify_waiters();
    }

    /// Removes an endpoint from selection. Attempts already using it may finish.
    pub fn remove(&self, id: &str) {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(id);
        self.inner.changed.notify_waiters();
    }

    /// Counts discovered endpoints; historical snapshot entries are excluded.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Captures discovered endpoints for adapter-owned discovery persistence.
    /// Transport descriptors remain the adapter's responsibility to validate on restore.
    pub fn endpoints(&self) -> Vec<Arc<E>> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .endpoints()
    }

    /// Enables periodic atomic snapshots after completed operations. At most ten
    /// seconds of recent measurements may be lost on abrupt process termination.
    /// Call before cloning the pool. Incompatible snapshots are ignored; unreadable
    /// or malformed snapshots are reported to the caller.
    pub fn persist_to(&mut self, path: impl AsRef<Path>) -> io::Result<()> {
        let path = path.as_ref();
        match std::fs::read(path) {
            Ok(bytes) => {
                let snapshot: Snapshot = serde_json::from_slice(&bytes)?;
                if snapshot.version != 1 || snapshot.namespace != self.inner.namespace {
                    warn!(
                        operation = "service_pool_restore",
                        target = %path.display(),
                        namespace = %self.inner.namespace,
                        outcome = "ignored",
                        "saved endpoint statistics belong to another pool or schema version",
                    );
                    self.snapshot_path = Some(Arc::new(path.to_owned()));
                    return Ok(());
                }

                let count = snapshot.endpoints.len();
                self.inner
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .restore(snapshot.endpoints);
                info!(
                    operation = "service_pool_restore",
                    target = %path.display(),
                    namespace = %self.inner.namespace,
                    endpoints = count,
                    outcome = "restored",
                    "restored endpoint statistics",
                );
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }

        self.snapshot_path = Some(Arc::new(path.to_owned()));
        Ok(())
    }

    /// Imports measurements from an adapter-owned profile without changing where
    /// future snapshots are written. Endpoints must still be supplied by the adapter.
    /// A profile for another network or schema is rejected.
    pub fn restore_snapshot(&self, snapshot: Snapshot) -> Result<(), Failure> {
        if snapshot.version != 1 || snapshot.namespace != self.inner.namespace {
            return Err(Failure::Fatal(
                "endpoint profile belongs to another pool or schema version".into(),
            ));
        }

        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .restore(snapshot.endpoints);
        Ok(())
    }

    /// Measures every currently registered endpoint, independent of its rank.
    /// One warm-up request establishes the transport; successful measured requests
    /// then replace the class estimate with their arithmetic mean. Each endpoint
    /// stops at its first error. Admission and request deadlines remain in force.
    /// Use a dedicated pool: calibration intentionally replaces live estimates.
    pub async fn measure_all<T, F, Fut>(
        &self,
        class: &str,
        samples: usize,
        call: F,
    ) -> Result<(), Failure>
    where
        F: Fn(Arc<E>) -> Fut + Send + Sync,
        Fut: Future<Output = Result<T, Failure>> + Send,
        T: Send,
    {
        if samples == 0 {
            return Err(Failure::Fatal("measurement count must be positive".into()));
        }

        let endpoints = self.endpoints();
        let total = endpoints.len();
        let started = Instant::now();
        let mut completed = 0;
        let mut successful = 0;
        let mut progress_at = Instant::now();
        info!(
            operation = "service_pool_calibration",
            target = %self.inner.namespace,
            class,
            total,
            samples,
            parallel = self.inner.options.max_in_flight,
            outcome = "started",
            "measuring all registered endpoints",
        );

        let mut pending = stream::iter(endpoints)
            .map(|endpoint| {
                let call = &call;
                async move {
                    let mut sum_ms = 0.0;
                    let mut measured = 0;
                    let mut error = None;
                    for sample in 0..=samples {
                        let result = self
                            .execute_where(
                                class,
                                |candidate| candidate.id() == endpoint.id(),
                                |candidate| async {
                                    let started = Instant::now();
                                    call(candidate).await?;
                                    Ok(started.elapsed().as_secs_f64() * 1000.0)
                                },
                            )
                            .await;
                        match result {
                            Ok(ms) if sample != 0 => {
                                sum_ms += ms;
                                measured += 1;
                            }
                            Ok(_) => {}
                            Err(failure) => {
                                error = Some(failure);
                                break;
                            }
                        }
                    }

                    // A warm-up alone is not a measured sample. Do not leave its
                    // connection setup time in the exported response-time estimate.
                    self.inner
                        .state
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .set_latency(
                            endpoint.id(),
                            class,
                            (measured == samples).then(|| (sum_ms / measured as f64).max(0.001)),
                        );
                    (measured, error)
                }
            })
            .buffer_unordered(self.inner.options.max_in_flight);

        while let Some((measured, error)) = pending.next().await {
            completed += 1;
            successful += usize::from(measured == samples);
            if let Some(error @ Failure::Fatal(_)) = error {
                return Err(error);
            }
            if progress_at.elapsed() >= Duration::from_secs(1) || completed == total {
                info!(
                    operation = "service_pool_calibration",
                    target = %self.inner.namespace,
                    class,
                    completed,
                    total,
                    successful,
                    duration_ms = started.elapsed().as_millis(),
                    outcome = if completed == total { "completed" } else { "measuring" },
                    "endpoint calibration progress",
                );
                progress_at = Instant::now();
            }
        }

        self.flush()
            .await
            .map_err(|error| Failure::Fatal(format!("cannot save calibration: {error}")))
    }

    /// Captures measurements without retaining connections or locking their transports.
    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            version: 1,
            namespace: self.inner.namespace.clone(),
            endpoints: self
                .inner
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .snapshot(),
        }
    }

    /// Flushes a configured snapshot, including after an idle period or before shutdown.
    pub async fn flush(&self) -> io::Result<()> {
        let mut saved_at = self.inner.saved_at.lock().await;
        self.write_snapshot().await?;
        *saved_at = Some(Instant::now());
        drop(saved_at);
        Ok(())
    }

    async fn write_snapshot(&self) -> io::Result<()> {
        let Some(path) = self.snapshot_path.clone() else {
            return Ok(());
        };
        let snapshot = self.snapshot();
        let started = Instant::now();

        tokio::task::spawn_blocking(move || {
            let parent = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            std::fs::create_dir_all(parent)?;
            let mut file = tempfile::NamedTempFile::new_in(parent)?;
            serde_json::to_writer_pretty(&mut file, &snapshot)?;
            file.as_file().sync_all()?;
            file.persist(path.as_ref()).map_err(|error| error.error)?;
            debug!(
                operation = "service_pool_snapshot",
                target = %path.display(),
                duration_ms = started.elapsed().as_millis(),
                outcome = "stored",
                "saved endpoint statistics",
            );
            Ok(())
        })
        .await
        .map_err(io::Error::other)?
    }

    /// Runs a replayable operation and returns its first accepted response.
    /// The callback may run concurrently on distinct endpoints. It must complete
    /// response validation before returning `Ok` and safely release local resources
    /// when dropped. Use read-only or idempotent operations: cancellation cannot
    /// undo remote side effects. All attempts share one deadline and global limits.
    pub async fn execute<T, F, Fut>(&self, class: &str, call: F) -> Result<T, Failure>
    where
        F: Fn(Arc<E>) -> Fut,
        Fut: Future<Output = Result<T, Failure>>,
    {
        self.execute_where(class, |_| true, call).await
    }

    /// Executes a request and uses successful operations for background measurements.
    /// A probe replays the same validated operation on an under-sampled endpoint.
    /// It runs independently of foreground winners and discards its result after
    /// updating measurements. Operations must be read-only or idempotent, as with
    /// `execute`. Limits and timeouts apply; dropping all pool clones cancels probes.
    pub async fn execute_with_probes<T, F, Fut>(&self, class: &str, call: F) -> Result<T, Failure>
    where
        E: 'static,
        F: Fn(Arc<E>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T, Failure>> + Send,
        T: Send,
    {
        let result = self.execute(class, &call).await;

        if result.is_ok() {
            let mut probes = self
                .probes
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            while probes.try_join_next().is_some() {}

            let call = Arc::new(call);
            for kind in [ProbeKind::Recovery, ProbeKind::Discovery] {
                let endpoint = self
                    .inner
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .select_probe(class, kind, &self.inner.options);
                let Some(endpoint) = endpoint else {
                    continue;
                };
                let lease = Lease::new(
                    Arc::clone(&self.inner),
                    endpoint.id().to_owned(),
                    endpoint.address(),
                    class.to_owned(),
                    Some(kind),
                );
                let call = Arc::clone(&call);
                let deadline = self.inner.options.attempt_timeout;
                probes.spawn(async move {
                    let result = tokio::time::timeout(deadline, call(endpoint))
                        .await
                        .unwrap_or_else(|_| Err(Failure::Retryable("probe timed out".into())));
                    lease.finish(&result);
                });
            }
        }

        result
    }

    /// Filters endpoints using request-specific requirements before allocating an
    /// attempt. The predicate must be fast and must not call back into the pool.
    /// Availability changes are rechecked on each dispatch.
    pub async fn execute_where<T, P, F, Fut>(
        &self,
        class: &str,
        eligible: P,
        call: F,
    ) -> Result<T, Failure>
    where
        P: Fn(&E) -> bool,
        F: Fn(Arc<E>) -> Fut,
        Fut: Future<Output = Result<T, Failure>>,
    {
        let result = self.run(class, eligible, call).await;

        if self.snapshot_path.is_some()
            && let Ok(mut saved_at) = self.inner.saved_at.try_lock()
            && saved_at.is_none_or(|at| at.elapsed() >= Duration::from_secs(10))
        {
            match self.write_snapshot().await {
                Ok(()) => *saved_at = Some(Instant::now()),
                Err(error) => warn!(
                    operation = "service_pool_snapshot",
                    target = %self.inner.namespace,
                    outcome = "failed",
                    error = %error,
                    "could not save endpoint statistics",
                ),
            }
        }

        result
    }

    async fn run<T, P, F, Fut>(&self, class: &str, eligible: P, call: F) -> Result<T, Failure>
    where
        P: Fn(&E) -> bool,
        F: Fn(Arc<E>) -> Fut,
        Fut: Future<Output = Result<T, Failure>>,
    {
        let options = &self.inner.options;
        let deadline = Instant::now() + options.request_timeout;
        let mut next_hedge = Instant::now();
        let mut excluded = HashSet::new();
        let mut pending = FuturesUnordered::new();
        let mut last_error = Failure::Unavailable("no eligible endpoints".into());

        loop {
            if Instant::now() >= deadline {
                return Err(Failure::Retryable(format!(
                    "{class} deadline exceeded after {} attempts; last result: {last_error}",
                    excluded.len()
                )));
            }

            // Register before checking capacity to avoid missing a completed lease.
            let notified = self.inner.changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();

            let mut wake_at = deadline;
            if excluded.len() < options.max_attempts
                && pending.len() < options.max_parallel_attempts
                && (pending.is_empty() || Instant::now() >= next_hedge)
            {
                let selected = self
                    .inner
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .select(class, &excluded, &eligible, options);
                wake_at = selected.wake_at.unwrap_or(deadline).min(deadline);

                if let Some(endpoint) = selected.endpoint {
                    excluded.insert(endpoint.id().to_owned());
                    let lease = Lease::new(
                        Arc::clone(&self.inner),
                        endpoint.id().to_owned(),
                        endpoint.address(),
                        class.to_owned(),
                        None,
                    );
                    let future = call(endpoint);
                    pending.push(async move {
                        let result = tokio::time::timeout(options.attempt_timeout, future)
                            .await
                            .unwrap_or_else(|_| {
                                Err(Failure::Retryable("attempt timed out".into()))
                            });
                        lease.finish(&result);
                        result
                    });
                    next_hedge = Instant::now() + options.hedge_after;
                } else if !selected.waiting && pending.is_empty() {
                    return Err(last_error);
                }
            }

            if pending.is_empty() && excluded.len() >= options.max_attempts {
                return Err(last_error);
            }
            if pending.len() < options.max_parallel_attempts
                && excluded.len() < options.max_attempts
                && next_hedge > Instant::now()
            {
                wake_at = wake_at.min(next_hedge);
            }

            tokio::select! {
                result = pending.next(), if !pending.is_empty() => {
                    match result {
                        Some(Ok(response)) => return Ok(response),
                        Some(Err(error @ Failure::Fatal(_))) => return Err(error),
                        Some(Err(error)) => {
                            last_error = error;
                            // Failover need not wait for the speculative delay.
                            next_hedge = Instant::now();
                        }
                        None => {}
                    }
                }
                () = &mut notified => {}
                () = tokio::time::sleep_until(wake_at) => {
                    if Instant::now() >= deadline {
                        return Err(Failure::Retryable(format!("{class} deadline exceeded after {} attempts; last result: {last_error}", excluded.len())));
                    }
                }
            }
        }
    }
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
