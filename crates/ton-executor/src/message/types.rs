use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Result of a transaction emulation.
#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum EmulationResult {
    Success(RunTransactionResultSuccess),
    Error(RunTransactionResultError),
}

#[derive(Deserialize, Debug, Clone)]
pub struct RunTransactionResultSuccess {
    /// Base64 encoded transaction `BoC`.
    pub transaction: Arc<str>,
    /// Base64 encoded updated shard account `BoC`.
    pub shard_account: Arc<str>,
    /// Virtual Machine execution logs.
    pub vm_log: Arc<str>,
    /// Base64 encoded actions `BoC` (if any).
    pub actions: Option<Arc<str>>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct RunTransactionResultError {
    /// Error message.
    pub error: String,
    /// Virtual Machine execution logs (if available).
    pub vm_log: Option<String>,
    /// VM exit code (if available).
    pub vm_exit_code: Option<i64>,
    /// Set by executor.
    pub executor_logs: Option<Arc<str>>,
}

/// Information about previous blocks.
#[derive(Debug, Clone)]
pub struct PrevBlocksInfo {
    // TODO: Add fields based on actual requirements
}

/// Arguments for running a transaction emulation.
#[derive(Debug, Clone)]
pub struct RunTransactionArgs {
    /// Base64 encoded libraries `BoC`.
    pub libs: Option<String>,
    /// Base64 encoded shard account `BoC`.
    pub shard_account: String,
    /// Current unix time.
    pub now: u32,
    /// Current logical time.
    pub lt: u64,
    /// Optional random seed for the VM.
    pub random_seed: Option<[u8; 32]>,
    /// Whether to ignore signature checks.
    pub ignore_chksig: bool,
    /// Whether to enable debug logs.
    pub debug_enabled: bool,
    /// Information about previous blocks.
    pub prev_blocks_info: Option<PrevBlocksInfo>,
    /// Whether this is a tick-tock transaction.
    pub is_tick_tock: Option<bool>,
    /// Whether this is a "tock" transaction (if `is_tick_tock` is true).
    pub is_tock: Option<bool>,
}

impl Default for RunTransactionArgs {
    fn default() -> Self {
        Self {
            libs: None,
            shard_account: String::new(),
            now: 0,
            lt: 0,
            random_seed: None,
            ignore_chksig: false,
            debug_enabled: true,
            prev_blocks_info: None,
            is_tick_tock: None,
            is_tock: None,
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
pub(crate) enum EmulationInternalResult {
    Success {
        output: EmulationResult,
        logs: Arc<str>,
    },
    Fail {
        #[allow(dead_code)] // used only for decoding
        fail: bool,
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EmulationInternalParams {
    pub utime: u32,
    pub lt: String, // For some reason this field is a String in C++ code treated as u64
    pub rand_seed: String,
    pub ignore_chksig: bool,
    pub debug_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_tick_tock: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_tock: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev_blocks_info: Option<String>,
}

impl From<&RunTransactionArgs> for EmulationInternalParams {
    fn from(args: &RunTransactionArgs) -> Self {
        let rand_seed = match &args.random_seed {
            Some(seed) => hex::encode(seed),
            None => String::new(),
        };

        let prev_blocks_info = args
            .prev_blocks_info
            .as_ref()
            .map(|_| panic!("TODO: Implement prev_blocks_info serialization"));

        Self {
            utime: args.now,
            lt: args.lt.to_string(),
            rand_seed,
            ignore_chksig: args.ignore_chksig,
            debug_enabled: args.debug_enabled,
            is_tick_tock: args.is_tick_tock,
            is_tock: args.is_tock,
            prev_blocks_info,
        }
    }
}
