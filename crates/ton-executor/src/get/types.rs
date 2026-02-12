use crate::common::ExecutorVerbosity;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Result of a get-method execution.
#[derive(Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(untagged)]
pub enum GetMethodResult {
    Success(GetMethodResultSuccess),
    Error(GetMethodResultError),
}

/// Successful get-method execution details.
#[derive(Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct GetMethodResultSuccess {
    /// Whether the execution was successful (always true in this variant).
    pub success: bool,
    /// Base64 encoded stack `BoC`.
    pub stack: Arc<str>,
    /// Gas consumed during execution (as a string).
    pub gas_used: String,
    /// VM exit code.
    pub vm_exit_code: i32,
    /// Virtual Machine execution logs.
    pub vm_log: Arc<str>,
    /// Base64 encoded missing library hash (if any).
    pub missing_library: Option<String>,

    #[serde(skip)]
    /// Base64 encoded code of contract.
    pub code: Arc<str>,
}

/// Get-method execution error details.
#[derive(Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct GetMethodResultError {
    /// Whether the execution was successful (always false in this variant).
    pub success: bool,
    /// Error message.
    pub error: Arc<str>,
}

/// Arguments for running a get-method.
#[derive(Serialize, Clone, Debug)]
pub struct RunGetMethodArgs {
    /// Base64 encoded contract code `BoC`.
    pub code: String,
    /// Base64 encoded contract data `BoC`.
    pub data: String,
    /// Verbosity level for the emulator logs.
    pub verbosity: ExecutorVerbosity,
    /// Base64 encoded libraries `BoC`.
    pub libs: String,
    /// Contract address (as a string).
    pub address: String,
    /// Current unix time.
    pub unixtime: i64,
    /// Contract balance (as a string).
    pub balance: String,
    /// Optional random seed.
    pub rand_seed: String,
    /// Gas limit for execution (as a string).
    pub gas_limit: String,
    /// Method identifier (CRC32 of method name or numeric ID).
    pub method_id: i32,
    /// Whether to enable debug logs.
    pub debug_enabled: bool,
    /// Extra currencies (map of currency ID to amount).
    #[serde(default)]
    pub extra_currencies: HashMap<String, String>,
    /// Information about previous blocks.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev_blocks_info: Option<String>,
}

impl Default for RunGetMethodArgs {
    fn default() -> Self {
        Self {
            code: "te6ccgEBAQEAAgAAAA==".to_owned(),
            data: "te6ccgEBAQEAAgAAAA==".to_owned(),
            verbosity: ExecutorVerbosity::Short,
            libs: String::new(),
            address: "EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c".to_owned(),
            unixtime: 0,
            balance: "100000000".to_string(),
            rand_seed: "0000000000000000000000000000000000000000000000000000000000000000"
                .to_owned(),
            gas_limit: "1000000".to_owned(),
            method_id: 0,
            debug_enabled: true,
            extra_currencies: HashMap::new(),
            prev_blocks_info: None,
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
pub(crate) enum GetInternalResult {
    Success {
        output: GetMethodResult,
    },
    Fail {
        #[allow(dead_code)] // used only for decoding
        fail: bool,
        message: String,
    },
}
