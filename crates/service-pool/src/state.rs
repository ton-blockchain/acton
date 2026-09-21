use super::{Endpoint, EndpointSnapshot, Failure, Inner, Metrics, Options, unix_seconds};
use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
    time::Duration,
};
use tokio::time::Instant;
use tracing::debug;

struct Record<E> {
    endpoint: Option<Arc<E>>,
    address: String,
    in_flight: usize,
    last_dispatch: u64,
    next_request_at: Option<Instant>,
    last_probe: BTreeMap<String, u64>,
    measured_at: BTreeMap<String, Instant>,
    failures: u32,
    suspended_until: Option<Instant>,
    classes: BTreeMap<String, Metrics>,
}

pub(super) struct State<E> {
    records: BTreeMap<String, Record<E>>,
    dispatches: u64,
    in_flight: usize,
    active_probes: HashSet<(String, ProbeKind)>,
    probed_at: BTreeMap<(String, ProbeKind), Instant>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) enum ProbeKind {
    Recovery,
    Discovery,
}

impl<E> Default for State<E> {
    fn default() -> Self {
        Self {
            records: BTreeMap::new(),
            dispatches: 0,
            in_flight: 0,
            active_probes: HashSet::new(),
            probed_at: BTreeMap::new(),
        }
    }
}

pub(super) struct Selection<E> {
    pub(super) endpoint: Option<Arc<E>>,
    pub(super) waiting: bool,
    pub(super) wake_at: Option<Instant>,
}

impl Metrics {
    /// A response-time mean cannot describe unfinished requests. Retain their
    /// lower bound separately so an obsolete fast mean does not keep winning.
    fn routing_latency(&self) -> Option<f64> {
        self.latency_ms
            .map(|mean| mean.max(self.latency_lower_bound_ms.unwrap_or_default()))
    }
}

impl<E: Endpoint> State<E> {
    pub(super) fn upsert(&mut self, endpoint: E) {
        let address = endpoint.address();
        let record = self
            .records
            .entry(endpoint.id().to_owned())
            .or_insert_with(|| Record {
                endpoint: None,
                address: address.clone(),
                in_flight: 0,
                last_dispatch: 0,
                next_request_at: None,
                last_probe: BTreeMap::new(),
                measured_at: BTreeMap::new(),
                failures: 0,
                suspended_until: None,
                classes: BTreeMap::new(),
            });

        if record.address != address {
            record.classes.clear();
            record.measured_at.clear();
            record.last_probe.clear();
            record.failures = 0;
            record.suspended_until = None;
            record.next_request_at = None;
            record.address = address;
        }
        record.endpoint = Some(Arc::new(endpoint));
    }

    pub(super) fn remove(&mut self, id: &str) {
        if let Some(record) = self.records.get_mut(id) {
            record.endpoint = None;
        }
    }

    pub(super) fn len(&self) -> usize {
        self.records
            .values()
            .filter(|record| record.endpoint.is_some())
            .count()
    }

    pub(super) fn endpoints(&self) -> Vec<Arc<E>> {
        self.records
            .values()
            .filter_map(|record| record.endpoint.clone())
            .collect()
    }

    pub(super) fn select(
        &mut self,
        class: &str,
        excluded: &HashSet<String>,
        eligible: &impl Fn(&E) -> bool,
        options: &Options,
    ) -> Selection<E> {
        let now = Instant::now();
        let mut selected: Option<(&str, u8, f64, u64)> = None;
        let mut selection = Selection {
            endpoint: None,
            waiting: false,
            wake_at: None,
        };

        for (id, record) in &self.records {
            let Some(endpoint) = &record.endpoint else {
                continue;
            };
            if excluded.contains(id) || !eligible(endpoint) {
                continue;
            }
            if let Some(until) = record.suspended_until.filter(|until| *until > now) {
                selection.waiting = true;
                selection.wake_at = Some(selection.wake_at.map_or(until, |at| at.min(until)));
                continue;
            }
            if record.in_flight >= options.max_in_flight_per_endpoint
                || self.in_flight >= options.max_in_flight
            {
                selection.waiting = true;
                continue;
            }

            let measured = record.classes.get(class).and_then(Metrics::routing_latency);
            // Before this class has measurements, prefer an endpoint that has
            // served another operation over one whose connectivity is unknown.
            let (tier, latency) = if let Some(latency) = measured {
                (0, latency)
            } else if let Some(latency) = record
                .classes
                .values()
                .filter_map(Metrics::routing_latency)
                .min_by(f64::total_cmp)
            {
                (1, latency)
            } else {
                (2, f64::INFINITY)
            };
            // Waiting briefly for a fast endpoint can finish sooner than an
            // immediate request to a slow one. Pacing also spreads sequential
            // traffic, where in-flight counts alone cannot express recent load.
            let wait = record
                .next_request_at
                .map_or(Duration::ZERO, |at| at.saturating_duration_since(now));
            let score = wait
                .as_secs_f64()
                .mul_add(1000.0, latency * (record.in_flight + 1) as f64);
            let better = selected.is_none_or(|(_, best_tier, best, last)| {
                tier < best_tier
                    || (tier == best_tier
                        && (score < best || (score == best && record.last_dispatch < last)))
            });
            if better {
                selected = Some((id, tier, score, record.last_dispatch));
            }
        }

        let Some((id, _, _, _)) = selected else {
            return selection;
        };
        let id = id.to_owned();
        let record = self.records.get_mut(&id).expect("selected endpoint exists");
        if let Some(at) = record.next_request_at.filter(|at| *at > now) {
            selection.waiting = true;
            selection.wake_at = Some(selection.wake_at.map_or(at, |wake| wake.min(at)));
            return selection;
        }

        self.dispatches = self.dispatches.saturating_add(1);
        self.in_flight += 1;
        record.in_flight += 1;
        record.last_dispatch = self.dispatches;
        record.next_request_at = Some(now + options.min_request_interval);
        selection.endpoint.clone_from(&record.endpoint);
        selection
    }

    pub(super) fn select_probe(
        &mut self,
        class: &str,
        kind: ProbeKind,
        options: &Options,
    ) -> Option<Arc<E>> {
        let key = (class.to_owned(), kind);
        if self.active_probes.len() >= options.max_background_probes
            || self.active_probes.contains(&key)
            || self.in_flight >= options.max_in_flight.saturating_sub(1)
            || self
                .probed_at
                .get(&key)
                .is_some_and(|at| at.elapsed() < options.probe_interval)
        {
            return None;
        }

        let now = Instant::now();
        // Recheck fast routes independently of discovery. A completed latency
        // spike can also strand an endpoint below the winners, without leaving
        // a cancellation bound. Other classes provide useful reachability hints.
        let record = self
            .records
            .values_mut()
            .filter(|record| {
                record.endpoint.is_some()
                    && record.in_flight == 0
                    && record.suspended_until.is_none_or(|until| until <= now)
                    && record.next_request_at.is_none_or(|at| at <= now)
                    && (kind == ProbeKind::Discovery
                        || record
                            .classes
                            .values()
                            .any(|metrics| metrics.latency_ms.is_some()))
                    && record
                        .measured_at
                        .get(class)
                        .is_none_or(|at| at.elapsed() >= options.probe_interval)
            })
            .min_by(|left, right| {
                let priority = |record: &Record<E>| {
                    if kind == ProbeKind::Recovery {
                        record
                            .classes
                            .values()
                            .filter_map(|metrics| metrics.latency_ms)
                            .min_by(f64::total_cmp)
                    } else {
                        None
                    }
                    .unwrap_or(f64::INFINITY)
                };
                let fairness = |record: &Record<E>| {
                    (
                        record.last_probe.get(class).copied().unwrap_or_default(),
                        record
                            .classes
                            .get(class)
                            .is_some_and(|m| m.latency_ms.is_some()),
                        record.last_dispatch,
                    )
                };

                priority(left)
                    .total_cmp(&priority(right))
                    .then_with(|| fairness(left).cmp(&fairness(right)))
            })?;

        self.dispatches = self.dispatches.saturating_add(1);
        record.last_dispatch = self.dispatches;
        record.next_request_at = Some(now + options.min_request_interval);
        record.last_probe.insert(class.to_owned(), self.dispatches);
        record.in_flight += 1;
        self.in_flight += 1;
        self.active_probes.insert(key.clone());
        self.probed_at.insert(key, now);
        record.endpoint.clone()
    }

    pub(super) fn snapshot(&self) -> Vec<EndpointSnapshot> {
        self.records
            .iter()
            .map(|(id, record)| EndpointSnapshot {
                id: id.clone(),
                address: record.address.clone(),
                in_flight: record.in_flight,
                classes: record.classes.clone(),
            })
            .collect()
    }

    pub(super) fn set_latency(&mut self, id: &str, class: &str, mean_ms: Option<f64>) {
        if let Some(metrics) = self
            .records
            .get_mut(id)
            .and_then(|record| record.classes.get_mut(class))
        {
            metrics.latency_ms = mean_ms;
            metrics.latency_lower_bound_ms = None;
        }
    }

    pub(super) fn restore(&mut self, endpoints: Vec<EndpointSnapshot>) {
        let now = unix_seconds();
        for mut entry in endpoints {
            for metrics in entry.classes.values_mut() {
                // Keep historical rankings as startup hints. Background probes
                // refresh them without forcing every restart through cold selection.
                if metrics.updated_at > now
                    || metrics
                        .latency_ms
                        .is_some_and(|ms| !ms.is_finite() || ms <= 0.0 || ms > 3_600_000.0)
                {
                    metrics.latency_ms = None;
                }
                if metrics
                    .latency_lower_bound_ms
                    .is_some_and(|ms| !ms.is_finite() || ms <= 0.0 || ms > 3_600_000.0)
                {
                    metrics.latency_lower_bound_ms = None;
                }
            }
            if let Some(record) = self.records.get_mut(&entry.id) {
                if record.address == entry.address {
                    for (class, metrics) in entry.classes {
                        // A supplied calibration and the local runtime cache can
                        // overlap. Keep the newest observation for each class.
                        let current = record.classes.entry(class).or_default();
                        if metrics.updated_at >= current.updated_at {
                            *current = metrics;
                        }
                    }
                }
            } else {
                self.records.insert(
                    entry.id,
                    Record {
                        endpoint: None,
                        address: entry.address,
                        in_flight: 0,
                        last_dispatch: 0,
                        next_request_at: None,
                        last_probe: BTreeMap::new(),
                        measured_at: BTreeMap::new(),
                        failures: 0,
                        suspended_until: None,
                        classes: entry.classes,
                    },
                );
            }
        }
    }
}

/// Reserves admission and records exactly one outcome, even when an operation
/// loses a race or its caller is cancelled. It never owns the transport future.
pub(super) struct Lease<E: Endpoint> {
    inner: Arc<Inner<E>>,
    id: String,
    class: String,
    address: String,
    started: Instant,
    finished: bool,
    probe_kind: Option<ProbeKind>,
}

impl<E: Endpoint> Lease<E> {
    pub(super) fn new(
        inner: Arc<Inner<E>>,
        id: String,
        address: String,
        class: String,
        probe_kind: Option<ProbeKind>,
    ) -> Self {
        Self {
            inner,
            id,
            address,
            class,
            started: Instant::now(),
            finished: false,
            probe_kind,
        }
    }

    pub(super) fn finish<T>(mut self, result: &Result<T, Failure>) {
        self.record(Some(result.as_ref().map(|_| ()).map_err(Clone::clone)));
        self.finished = true;
    }

    fn record(&self, result: Option<Result<(), Failure>>) {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.in_flight -= 1;
        if let Some(kind) = self.probe_kind {
            state.active_probes.remove(&(self.class.clone(), kind));
        }
        let record = state
            .records
            .get_mut(&self.id)
            .expect("active lease retains endpoint record");
        record.in_flight -= 1;
        if record.address != self.address {
            drop(state);
            self.inner.changed.notify_waiters();
            return;
        }

        let metrics = record.classes.entry(self.class.clone()).or_default();
        let elapsed = self.started.elapsed();
        let outcome;

        match &result {
            Some(Ok(())) => {
                metrics.successes = metrics.successes.saturating_add(1);
                let sample = (elapsed.as_secs_f64() * 1000.0).max(0.001);
                metrics.latency_ms = Some(
                    metrics
                        .latency_ms
                        .map_or(sample, |old| old.mul_add(0.8, sample * 0.2)),
                );
                metrics.latency_lower_bound_ms = None;
                metrics.updated_at = unix_seconds();
                record
                    .measured_at
                    .insert(self.class.clone(), Instant::now());
                record.failures = 0;
                outcome = "success";
            }
            Some(Err(Failure::Unavailable(_))) => {
                metrics.unavailable = metrics.unavailable.saturating_add(1);
                outcome = "unavailable";
            }
            Some(Err(Failure::Retryable(_) | Failure::Invalid(_))) => {
                metrics.failures = metrics.failures.saturating_add(1);
                record.failures = record.failures.saturating_add(1);
                let seconds = if matches!(result, Some(Err(Failure::Invalid(_)))) {
                    60
                } else {
                    1_u64 << record.failures.min(6)
                };
                let until = Instant::now() + Duration::from_secs(seconds);
                // Another in-flight response must not shorten an existing
                // suspension, especially one caused by invalid data.
                record.suspended_until =
                    Some(record.suspended_until.map_or(until, |old| old.max(until)));
                outcome = "failed";
            }
            Some(Err(Failure::Fatal(_))) => outcome = "fatal",
            None => {
                metrics.cancelled = metrics.cancelled.saturating_add(1);
                // A cancelled request has no measured response time, but its
                // elapsed time is a lower bound. Never let an old fast estimate
                // keep a repeatedly losing endpoint at the top of the ranking.
                let lower_bound = elapsed.as_secs_f64() * 1000.0;
                if lower_bound > 0.0 {
                    metrics.latency_lower_bound_ms = Some(
                        metrics
                            .latency_lower_bound_ms
                            .map_or(lower_bound, |old| old.max(lower_bound)),
                    );
                    metrics.updated_at = unix_seconds();
                }
                outcome = "cancelled";
            }
        }

        debug!(
            operation = "service_pool_attempt",
            target = %self.id,
            address = %record.address,
            class = %self.class,
            background = self.probe_kind.is_some(),
            duration_ms = elapsed.as_millis(),
            outcome,
            error = ?result.and_then(Result::err),
            "endpoint attempt completed",
        );
        drop(state);
        self.inner.changed.notify_waiters();
    }
}

impl<E: Endpoint> Drop for Lease<E> {
    fn drop(&mut self) {
        if !self.finished {
            self.record(None);
        }
    }
}
