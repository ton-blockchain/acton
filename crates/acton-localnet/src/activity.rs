//! Background traffic belongs to the localnet service, independently of Studio tabs.
//! Settings persist between runs; interrupted work is never automatically replayed.

mod contracts;
mod engine;
mod metadata;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub(crate) use engine::{Engine, Scenario};

/// Control requests are handled by the network owner; reading state uses no command.
/// Start includes the exact settings the user reviewed, avoiding a save/start race.
pub enum ActivityCommand {
    Save(ActivityConfig),
    Start(ActivityConfig),
    Stop,
}

/// Relative frequencies for independent, real on-chain scenarios. Zero disables
/// a family; weights describe scenario starts, not the number of transactions.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScenarioWeights {
    pub transfers: u16,
    pub batches: u16,
    pub jettons: u16,
    pub nfts: u16,
}

/// A bounded workload. The scheduler skips occupied slots instead of accumulating
/// a backlog, so a slow network cannot cause an unbounded burst of submissions.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivityConfig {
    pub interval_seconds: u32,
    /// Number of scenarios admitted together at each interval, subject to capacity.
    pub scenarios_per_launch: u16,
    pub concurrency: u16,
    /// Zero keeps the generator running until explicitly stopped.
    pub duration_seconds: u32,
    /// Fixed size, or the inclusive upper bound when batch randomization is enabled.
    pub max_batch_size: u16,
    pub randomize_batch_size: bool,
    /// Integer nanotons avoid floating-point rounding in signed messages.
    pub transfer_amount: u64,
    pub wallet_versions: Vec<ActivityWalletVersion>,
    pub scenarios: ScenarioWeights,
}

impl Default for ActivityConfig {
    fn default() -> Self {
        Self {
            interval_seconds: 10,
            scenarios_per_launch: 1,
            concurrency: 2,
            duration_seconds: 0,
            max_batch_size: 16,
            randomize_batch_size: false,
            transfer_amount: 100_000_000,
            wallet_versions: vec![
                ActivityWalletVersion::V3r2,
                ActivityWalletVersion::V4r2,
                ActivityWalletVersion::V5r1,
            ],
            scenarios: ScenarioWeights {
                transfers: 50,
                batches: 20,
                jettons: 20,
                nfts: 10,
            },
        }
    }
}

impl ActivityConfig {
    /// Checks limits before saving or starting work. Both HTTP and native callers
    /// pass through this validation; invalid settings never replace saved settings.
    pub fn validate(&self) -> Result<(), crate::Error> {
        let invalid = if !(1..=3600).contains(&self.interval_seconds) {
            Some("Choose a scenario interval between 1 and 3600 seconds")
        } else if !(1..=1000).contains(&self.scenarios_per_launch) {
            Some("Choose between 1 and 1000 scenarios per launch")
        } else if !(1..=1024).contains(&self.concurrency) {
            Some("Choose between 1 and 1024 concurrent scenarios")
        } else if self.duration_seconds > 86_400 {
            Some("Choose a duration up to 24 hours, or 0 to run until stopped")
        } else if !(2..=128).contains(&self.max_batch_size) {
            Some("Choose between 2 and 128 transfers per batch")
        } else if !(1_000_000..=1_000_000_000_000).contains(&self.transfer_amount) {
            Some("Choose a transfer amount between 0.001 and 1000 GRAM")
        } else if self.wallet_versions.is_empty() || self.wallet_versions.len() > 3 {
            Some("Select at least one wallet version")
        } else if self
            .wallet_versions
            .iter()
            .enumerate()
            .any(|(index, version)| self.wallet_versions[..index].contains(version))
        {
            Some("Select each wallet version only once")
        } else {
            let weights = self.scenarios.entries();
            if weights.iter().all(|(_, weight)| *weight == 0) {
                Some("Enable at least one activity scenario")
            } else if weights.iter().any(|(_, weight)| *weight > 100) {
                Some("Scenario weights must be between 0 and 100")
            } else {
                None
            }
        };

        invalid.map_or(Ok(()), |message| Err(crate::Error::invalid(message)))
    }
}

/// Standard wallets supported by the shared native TON library. Batch scenarios
/// use V5's action list regardless of the single-transfer wallet selection.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum ActivityWalletVersion {
    V3r2,
    V4r2,
    V5r1,
}

/// A family of complete interactions, including contract deployment where needed.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum ActivityScenario {
    Transfers,
    Batches,
    Jettons,
    Nfts,
}

impl ScenarioWeights {
    pub(crate) const fn entries(&self) -> [(ActivityScenario, u16); 4] {
        [
            (ActivityScenario::Transfers, self.transfers),
            (ActivityScenario::Batches, self.batches),
            (ActivityScenario::Jettons, self.jettons),
            (ActivityScenario::Nfts, self.nfts),
        ]
    }
}

/// Run state is separate from network status: a failed scenario never marks a
/// healthy blockchain failed. A process restart moves an active run to Interrupted.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum ActivityStatus {
    #[default]
    Stopped,
    Running,
    Stopping,
    Completed,
    Interrupted,
}

/// One bounded history entry. Links use the generator's wallet address so users
/// can inspect the actual transactions without exposing any signing material.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActivityRun {
    pub id: u64,
    pub scenario: ActivityScenario,
    pub started_at: u64,
    pub duration_ms: u64,
    pub address: Option<String>,
    pub confirmed_messages: u64,
    pub batch_size: Option<u16>,
    pub outcome: ActivityOutcome,
    pub error: Option<String>,
}

/// Cancellation is not successful completion or a failed blockchain transaction.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum ActivityOutcome {
    Completed,
    Failed,
    Cancelled,
}

/// Last run and saved settings. Counts describe confirmed generator submissions,
/// not network TPS; resulting internal transactions can be inspected in Explorer.
#[derive(Clone, Debug, Default, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActivityState {
    pub config: ActivityConfig,
    pub status: ActivityStatus,
    pub run_id: Option<String>,
    pub started_at: Option<u64>,
    pub finished_at: Option<u64>,
    pub active: u16,
    pub completed: u64,
    pub failed: u64,
    pub confirmed_messages: u64,
    pub skipped: u64,
    pub recent: Vec<ActivityRun>,
}
