use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use web_time::{Duration, Instant};

use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileEvent {
    pub name: &'static str,
    pub elapsed: Duration,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProfileSummary {
    pub events: Vec<ProfileEvent>,
    pub counters: BTreeMap<&'static str, u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileReport {
    pub enabled: bool,
    pub counters: BTreeMap<String, u64>,
    pub spans: BTreeMap<String, ProfileSpan>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSpan {
    pub count: u64,
    pub total_ms: f64,
    pub average_ms: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Profiler {
    enabled: bool,
    summary: ProfileSummary,
}

#[must_use = "the profiling span must be kept alive for the measured scope"]
pub(crate) struct ProfileGuard<'a> {
    profiler: &'a mut Profiler,
    name: &'static str,
    started_at: Option<Instant>,
}

#[derive(Debug)]
pub(crate) struct BufferedProfiler {
    enabled: bool,
    events: RefCell<Vec<ProfileEvent>>,
}

impl BufferedProfiler {
    pub(crate) const fn new(profiler: &Profiler) -> Self {
        Self {
            enabled: profiler.is_enabled(),
            events: RefCell::new(Vec::new()),
        }
    }

    pub(crate) fn start(&self) -> Option<Instant> {
        self.enabled.then(Instant::now)
    }

    pub(crate) fn finish(&self, name: &'static str, started_at: Option<Instant>) {
        if let Some(started_at) = started_at {
            self.events.borrow_mut().push(ProfileEvent {
                name,
                elapsed: started_at.elapsed(),
            });
        }
    }

    pub(crate) fn flush_into(&self, profiler: &mut Profiler) {
        profiler
            .summary
            .events
            .append(&mut self.events.borrow_mut());
    }
}

impl Profiler {
    #[must_use]
    pub fn disabled() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            summary: ProfileSummary::default(),
        }
    }

    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub fn start(&self) -> Option<Instant> {
        self.enabled.then(Instant::now)
    }

    pub(crate) fn span(&mut self, name: &'static str) -> ProfileGuard<'_> {
        let started_at = self.start();
        ProfileGuard {
            profiler: self,
            name,
            started_at,
        }
    }

    pub fn finish(&mut self, name: &'static str, started_at: Option<Instant>) {
        if let Some(started_at) = started_at {
            self.summary.events.push(ProfileEvent {
                name,
                elapsed: started_at.elapsed(),
            });
        }
    }

    pub fn increment(&mut self, name: &'static str) {
        if self.enabled {
            *self.summary.counters.entry(name).or_default() += 1;
        }
    }

    #[must_use]
    pub const fn summary(&self) -> &ProfileSummary {
        &self.summary
    }

    #[must_use]
    pub fn report(&self) -> ProfileReport {
        ProfileReport::new(self.enabled, &self.summary)
    }
}

impl ProfileGuard<'_> {
    pub(crate) const fn profiler(&mut self) -> &mut Profiler {
        self.profiler
    }
}

impl Drop for ProfileGuard<'_> {
    fn drop(&mut self) {
        self.profiler.finish(self.name, self.started_at);
    }
}

#[must_use]
pub fn render_profile_summary(summary: &ProfileSummary) -> String {
    render_profile_report(&ProfileReport::new(true, summary))
}

impl ProfileReport {
    #[must_use]
    pub fn new(enabled: bool, summary: &ProfileSummary) -> Self {
        let counters = summary
            .counters
            .iter()
            .map(|(name, count)| ((*name).to_owned(), *count))
            .collect();
        let mut aggregated = BTreeMap::<&'static str, (u64, f64)>::new();
        for event in &summary.events {
            let entry = aggregated.entry(event.name).or_default();
            entry.0 += 1;
            entry.1 += event.elapsed.as_secs_f64() * 1000.0;
        }
        let spans = aggregated
            .into_iter()
            .map(|(name, (count, total_ms))| {
                (
                    name.to_owned(),
                    ProfileSpan {
                        count,
                        total_ms,
                        average_ms: total_ms / count as f64,
                    },
                )
            })
            .collect();

        Self {
            enabled,
            counters,
            spans,
        }
    }
}

#[must_use]
pub fn render_profile_report(report: &ProfileReport) -> String {
    if report.counters.is_empty() && report.spans.is_empty() {
        return "No profiling data".to_owned();
    }

    let mut output = String::new();
    if !report.counters.is_empty() {
        output.push_str("Counters\n");
        for (name, count) in &report.counters {
            let _ = writeln!(output, "  {name}: {count}");
        }
    }

    if !report.spans.is_empty() {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str("Spans\n");
        for (name, span) in &report.spans {
            let _ = writeln!(
                output,
                "  {name}: count={} total={:.3}ms avg={:.3}ms",
                span.count, span.total_ms, span.average_ms
            );
        }
    }

    output
}
