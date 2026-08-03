use std::{collections::VecDeque, sync::Arc};

use serde::Serialize;
use tokio::sync::RwLock;
use ton_indexer_core::Batch;
use utoipa::ToSchema;

/// Rolling TPS windows exposed by the public API.
pub const TPS_WINDOWS_SECONDS: [u64; 3] = [60, 300, 900];
pub(crate) const SAMPLE_RETENTION_SECONDS: u64 = TPS_WINDOWS_SECONDS[2] + 60;

/// Shared in-memory TPS accumulator.
#[derive(Clone, Default)]
pub struct TpsStats {
    inner: Arc<RwLock<TpsAccumulator>>,
}

/// Readiness of the TPS indexer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TpsStatus {
    /// Historical samples are still being loaded.
    Syncing,
    /// Every configured rolling window is covered and the startup tip was reached.
    Ready,
}

/// TPS calculated over one rolling time window.
#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
pub struct TpsWindow {
    /// Requested window duration in seconds.
    pub window_seconds: u64,
    /// Actual chain-time coverage currently available for this window.
    pub coverage_seconds: u64,
    /// Transactions observed in the window.
    pub transactions: u64,
    /// Transactions per second over the requested or currently covered duration.
    pub tps: f64,
    /// Whether the entire requested window is covered.
    pub complete: bool,
}

/// Current rolling network TPS snapshot.
#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
pub struct TpsSnapshot {
    /// Whether historical backfill has reached a fully covered live snapshot.
    pub status: TpsStatus,
    /// Latest indexed masterchain sequence number.
    pub latest_masterchain_seqno: Option<u32>,
    /// Generation time of the latest indexed masterchain block.
    pub latest_block_time: Option<u64>,
    /// Rolling TPS values ordered from shortest to longest window.
    pub windows: Vec<TpsWindow>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TpsSample {
    pub(crate) masterchain_seqno: u32,
    pub(crate) timestamp: u64,
    pub(crate) transactions: u64,
}

#[derive(Default)]
struct TpsAccumulator {
    samples: VecDeque<TpsSample>,
    startup_tip_seqno: Option<u32>,
}

impl TpsStats {
    pub(crate) fn from_samples(samples: impl IntoIterator<Item = TpsSample>) -> Self {
        let mut inner = TpsAccumulator::default();
        for sample in samples {
            inner.record(sample);
        }
        Self {
            inner: Arc::new(RwLock::new(inner)),
        }
    }

    pub(crate) async fn set_startup_tip(&self, seqno: u32) {
        let mut inner = self.inner.write().await;
        inner.startup_tip_seqno = Some(
            inner
                .startup_tip_seqno
                .map_or(seqno, |current| current.max(seqno)),
        );
    }

    pub(crate) fn sample_from_batch(batch: &Batch) -> TpsSample {
        let transactions = batch
            .blocks()
            .map(|block| u64::try_from(block.transactions().len()).unwrap_or(u64::MAX))
            .sum();
        let masterchain = batch.masterchain();
        TpsSample {
            masterchain_seqno: masterchain.id().seqno,
            timestamp: u64::from(masterchain.info().gen_utime),
            transactions,
        }
    }

    pub(crate) async fn record_sample(&self, sample: TpsSample) {
        let mut inner = self.inner.write().await;
        inner.record(sample);
    }

    /// Returns the current public TPS snapshot.
    pub async fn snapshot(&self) -> TpsSnapshot {
        self.inner.read().await.snapshot()
    }
}

impl TpsAccumulator {
    fn record(&mut self, sample: TpsSample) {
        if self
            .samples
            .back()
            .is_some_and(|latest| sample.masterchain_seqno <= latest.masterchain_seqno)
        {
            return;
        }

        self.samples.push_back(sample);
        let cutoff = sample.timestamp.saturating_sub(SAMPLE_RETENTION_SECONDS);
        while self
            .samples
            .front()
            .is_some_and(|sample| sample.timestamp < cutoff)
        {
            self.samples.pop_front();
        }
    }

    fn snapshot(&self) -> TpsSnapshot {
        let latest = self.samples.back().copied();
        let oldest_timestamp = self.samples.front().map(|sample| sample.timestamp);
        let caught_up = latest.is_some_and(|sample| {
            self.startup_tip_seqno
                .is_some_and(|tip| sample.masterchain_seqno >= tip)
        });
        let windows = TPS_WINDOWS_SECONDS
            .into_iter()
            .map(|window_seconds| self.window(latest, oldest_timestamp, window_seconds))
            .collect::<Vec<_>>();
        let status = if caught_up && windows.iter().all(|window| window.complete) {
            TpsStatus::Ready
        } else {
            TpsStatus::Syncing
        };

        TpsSnapshot {
            status,
            latest_masterchain_seqno: latest.map(|sample| sample.masterchain_seqno),
            latest_block_time: latest.map(|sample| sample.timestamp),
            windows,
        }
    }

    fn window(
        &self,
        latest: Option<TpsSample>,
        oldest_timestamp: Option<u64>,
        window_seconds: u64,
    ) -> TpsWindow {
        let Some(latest) = latest else {
            return TpsWindow {
                window_seconds,
                coverage_seconds: 0,
                transactions: 0,
                tps: 0.0,
                complete: false,
            };
        };
        let oldest_timestamp = oldest_timestamp.unwrap_or(latest.timestamp);
        let available_coverage = latest.timestamp.saturating_sub(oldest_timestamp);
        let complete = available_coverage >= window_seconds;
        let coverage_seconds = available_coverage.min(window_seconds);
        let cutoff = latest.timestamp.saturating_sub(window_seconds);
        let transactions = self
            .samples
            .iter()
            .filter(|sample| sample.timestamp > cutoff)
            .map(|sample| sample.transactions)
            .sum();
        let divisor = if complete {
            window_seconds
        } else {
            coverage_seconds.max(1)
        };

        TpsWindow {
            window_seconds,
            coverage_seconds,
            transactions,
            tps: transactions as f64 / divisor as f64,
            complete,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn calculates_complete_rolling_windows() {
        let stats = TpsStats::default();
        stats.set_startup_tip(90).await;
        for seqno in 0..=90 {
            stats
                .record_sample(TpsSample {
                    masterchain_seqno: seqno,
                    timestamp: u64::from(seqno) * 10,
                    transactions: 10,
                })
                .await;
        }

        let snapshot = stats.snapshot().await;
        assert_eq!(snapshot.status, TpsStatus::Ready);
        assert_eq!(snapshot.latest_masterchain_seqno, Some(90));
        assert_eq!(snapshot.windows.len(), 3);
        for window in snapshot.windows {
            assert!(window.complete);
            assert_eq!(window.transactions, window.window_seconds);
            assert_eq!(window.tps, 1.0);
        }
    }

    #[tokio::test]
    async fn reports_partial_coverage_during_backfill() {
        let stats = TpsStats::default();
        stats.set_startup_tip(20).await;
        stats
            .record_sample(TpsSample {
                masterchain_seqno: 10,
                timestamp: 100,
                transactions: 12,
            })
            .await;
        stats
            .record_sample(TpsSample {
                masterchain_seqno: 11,
                timestamp: 110,
                transactions: 8,
            })
            .await;

        let snapshot = stats.snapshot().await;
        assert_eq!(snapshot.status, TpsStatus::Syncing);
        assert_eq!(
            snapshot.windows[0],
            TpsWindow {
                window_seconds: 60,
                coverage_seconds: 10,
                transactions: 20,
                tps: 2.0,
                complete: false,
            }
        );
    }
}
