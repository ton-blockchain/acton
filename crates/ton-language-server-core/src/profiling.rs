use std::cell::RefCell;
use std::collections::BTreeMap;
use web_time::{Duration, Instant};

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

#[derive(Clone, Debug, Default)]
pub struct Profiler {
    enabled: bool,
    summary: ProfileSummary,
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
}
