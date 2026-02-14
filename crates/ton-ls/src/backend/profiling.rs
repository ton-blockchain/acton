use dashmap::DashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

#[derive(Debug, Default)]
pub struct FeatureStats {
    pub count: AtomicU64,
    pub total_duration_ns: AtomicU64,
}

#[derive(Debug, Default)]
pub struct ProfilingContext {
    pub feature_stats: DashMap<String, FeatureStats>,
    pub total_requests: AtomicU64,
}

impl ProfilingContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_feature(&self, feature_name: &str, duration: Duration) {
        let stats = self
            .feature_stats
            .entry(feature_name.to_string())
            .or_default();
        stats.count.fetch_add(1, Ordering::Relaxed);
        stats
            .total_duration_ns
            .fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
        self.total_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn log_stats(&self) {
        if self.total_requests.load(Ordering::Relaxed) == 0 {
            return;
        }

        log::info!("--- Tolk LS Profiling Stats ---");
        log::info!(
            "Total requests: {}",
            self.total_requests.load(Ordering::Relaxed)
        );

        for entry in self.feature_stats.iter() {
            let (name, stats) = entry.pair();
            let count = stats.count.load(Ordering::Relaxed);
            let total_ns = stats.total_duration_ns.load(Ordering::Relaxed);

            if count > 0 {
                let avg_duration = Duration::from_nanos(total_ns / count);
                let total_duration = Duration::from_nanos(total_ns);
                log::info!(
                    "Feature: {:<20} | Count: {:<5} | Avg: {:?} | Total: {:?}",
                    name,
                    count,
                    avg_duration,
                    total_duration
                );
            }
        }
        log::info!("-------------------------------");
    }
}

pub struct ProfileGuard<'a> {
    context: Arc<ProfilingContext>,
    feature_name: &'static str,
    start: Instant,
    _phantom: std::marker::PhantomData<&'a ()>,
}

impl<'a> ProfileGuard<'a> {
    pub fn new(context: Arc<ProfilingContext>, feature_name: &'static str) -> Self {
        Self {
            context,
            feature_name,
            start: Instant::now(),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<'a> Drop for ProfileGuard<'a> {
    fn drop(&mut self) {
        self.context
            .record_feature(self.feature_name, self.start.elapsed());
    }
}

#[macro_export]
macro_rules! profile {
    ($backend:expr, $name:expr) => {
        #[cfg(feature = "profiling")]
        let _guard =
            $crate::backend::profiling::ProfileGuard::new($backend.profiling.clone(), $name);
    };
}
