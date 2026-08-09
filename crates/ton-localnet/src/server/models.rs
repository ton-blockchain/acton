pub use acton_source_trace::{
    BuildSourceTraceRequest, SourceTraceBundleRequest, SourceTraceCompilerRequest,
    SourceTraceContextRequest, SourceTraceFileRequest, SourceTraceInMessageContextRequest,
    SourceTraceResponse,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct GetVerifiedSourceRequest {
    pub address: Option<String>,
    pub code_hash: Option<String>,
}

#[derive(Deserialize)]
pub struct FaucetRequest {
    pub address: String,
    pub amount: u128,
}

#[derive(Deserialize)]
pub struct JettonFaucetRequest {
    pub address: String,
    pub jetton_master: String,
    pub amount: String,
}

#[derive(Deserialize)]
pub struct SetShardAccountRequest {
    pub address: String,
    pub shard_account: String,
}

#[derive(Deserialize)]
pub struct ChangeAccountStateRequest {
    pub address: String,
    pub state: ChangeAccountStatePayload,
    #[serde(default = "default_true")]
    pub mine: bool,
}

const fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum ChangeAccountStatePayload {
    #[serde(rename = "nonexist")]
    Nonexist,
    #[serde(rename = "uninit")]
    Uninit { balance: Option<String> },
    #[serde(rename = "frozen")]
    Frozen {
        source: Option<String>,
        frozen_hash: Option<String>,
        balance: Option<String>,
    },
}

#[derive(Deserialize)]
pub struct SetNetworkConditionsRequest {
    pub response_delay_ms: u64,
}

#[derive(Default, Deserialize)]
pub struct MineBlocksRequest {
    pub blocks: Option<u32>,
}

#[derive(Deserialize)]
pub struct SetMiningModeRequest {
    pub skip_empty_blocks: bool,
}

#[derive(Deserialize)]
pub struct CreateCheckpointRequest {
    pub name: String,
    #[serde(default)]
    pub force: bool,
}

#[derive(Deserialize)]
pub struct CheckpointRequest {
    pub name: String,
}

#[derive(Deserialize)]
pub struct ImportCheckpointQuery {
    pub name: String,
    #[serde(default)]
    pub force: bool,
}

#[derive(Deserialize)]
pub struct IncreaseTimeRequest {
    pub seconds: u64,
}

#[derive(Deserialize)]
pub struct SetTimeRequest {
    pub timestamp: u32,
}

#[derive(Deserialize)]
pub struct SetNextBlockTimestampRequest {
    pub timestamp: u32,
}
