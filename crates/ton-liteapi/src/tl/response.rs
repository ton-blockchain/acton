use derivative::Derivative;
use tl_proto::{TlRead, TlWrite};

use super::common::*;
use super::utils::*;

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct MasterchainInfo {
    pub last: BlockIdExt,
    pub state_root_hash: Int256,
    pub init: ZeroStateIdExt,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct MasterchainInfoExt {
    #[tl(flags)]
    pub mode: (),
    pub version: u32,
    pub capabilities: u64,
    pub last: BlockIdExt,
    pub last_utime: u32,
    pub now: u32,
    pub state_root_hash: Int256,
    pub init: ZeroStateIdExt,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct CurrentTime {
    pub now: u32,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct Version {
    pub mode: u32,
    pub version: u32,
    pub capabilities: u64,
    pub now: u32,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct BlockData {
    pub id: BlockIdExt,
    #[derivative(Debug(format_with = "fmt_bytes"))]
    pub data: Vec<u8>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct BlockState {
    pub id: BlockIdExt,
    pub root_hash: Int256,
    pub file_hash: Int256,
    #[derivative(Debug(format_with = "fmt_bytes"))]
    pub data: Vec<u8>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct BlockHeader {
    pub id: BlockIdExt,
    #[tl(flags)]
    pub mode: (),
    #[tl(flags_bit = "mode.0")]
    pub with_state_update: Option<()>,
    #[tl(flags_bit = "mode.1")]
    pub with_value_flow: Option<()>,
    #[tl(flags_bit = "mode.4")]
    pub with_extra: Option<()>,
    #[tl(flags_bit = "mode.5")]
    pub with_shard_hashes: Option<()>,
    #[tl(flags_bit = "mode.6")]
    pub with_prev_blk_signatures: Option<()>,
    #[derivative(Debug(format_with = "fmt_bytes"))]
    pub header_proof: Vec<u8>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct SendMsgStatus {
    pub status: u32,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct AccountState {
    pub id: BlockIdExt,
    pub shardblk: BlockIdExt,
    #[derivative(Debug(format_with = "fmt_bytes"))]
    pub shard_proof: Vec<u8>,
    #[derivative(Debug(format_with = "fmt_bytes"))]
    pub proof: Vec<u8>,
    #[derivative(Debug(format_with = "fmt_bytes"))]
    pub state: Vec<u8>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct RunMethodResult {
    #[tl(flags)]
    pub mode: (),
    pub id: BlockIdExt,
    pub shardblk: BlockIdExt,
    #[tl(flags_bit = "mode.0")]
    #[derivative(Debug(format_with = "fmt_opt_bytes"))]
    pub shard_proof: Option<Vec<u8>>,
    #[tl(flags_bit = "mode.0")]
    #[derivative(Debug(format_with = "fmt_opt_bytes"))]
    pub proof: Option<Vec<u8>>,
    #[tl(flags_bit = "mode.1")]
    #[derivative(Debug(format_with = "fmt_opt_bytes"))]
    pub state_proof: Option<Vec<u8>>,
    #[tl(flags_bit = "mode.3")]
    #[derivative(Debug(format_with = "fmt_opt_bytes"))]
    pub init_c7: Option<Vec<u8>>,
    #[tl(flags_bit = "mode.4")]
    #[derivative(Debug(format_with = "fmt_opt_bytes"))]
    pub lib_extras: Option<Vec<u8>>,
    pub exit_code: i32,
    #[tl(flags_bit = "mode.2")]
    #[derivative(Debug(format_with = "fmt_opt_bytes"))]
    pub result: Option<Vec<u8>>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct ShardInfo {
    pub id: BlockIdExt,
    pub shardblk: BlockIdExt,
    #[derivative(Debug(format_with = "fmt_bytes"))]
    pub shard_proof: Vec<u8>,
    #[derivative(Debug(format_with = "fmt_bytes"))]
    pub shard_descr: Vec<u8>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct AllShardsInfo {
    pub id: BlockIdExt,
    #[derivative(Debug(format_with = "fmt_bytes"))]
    pub proof: Vec<u8>,
    #[derivative(Debug(format_with = "fmt_bytes"))]
    pub data: Vec<u8>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct TransactionInfo {
    pub id: BlockIdExt,
    #[derivative(Debug(format_with = "fmt_bytes"))]
    pub proof: Vec<u8>,
    #[derivative(Debug(format_with = "fmt_bytes"))]
    pub transaction: Vec<u8>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct TransactionList {
    pub ids: Vec<BlockIdExt>,
    #[derivative(Debug(format_with = "fmt_bytes"))]
    pub transactions: Vec<u8>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct TransactionMetadata {
    #[tl(flags)]
    mode: (),
    depth: u32,
    initiator: AccountId,
    initiator_lt: u64,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct TransactionId {
    #[tl(flags)]
    pub mode: (),
    #[tl(flags_bit = "mode.0")]
    pub account: Option<Int256>,
    #[tl(flags_bit = "mode.1")]
    pub lt: Option<u64>,
    #[tl(flags_bit = "mode.2")]
    pub hash: Option<Int256>,
    #[tl(flags_bit = "mode.8")]
    pub metadata: Option<TransactionMetadata>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct BlockTransactions {
    pub id: BlockIdExt,
    pub req_count: u32,
    pub incomplete: bool,
    pub ids: Vec<TransactionId>,
    #[derivative(Debug(format_with = "fmt_bytes"))]
    pub proof: Vec<u8>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct BlockTransactionsExt {
    pub id: BlockIdExt,
    pub req_count: u32,
    pub incomplete: bool,
    #[derivative(Debug(format_with = "fmt_bytes"))]
    pub transactions: Vec<u8>,
    #[derivative(Debug(format_with = "fmt_bytes"))]
    pub proof: Vec<u8>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct PartialBlockProof {
    pub complete: bool,
    pub from: BlockIdExt,
    pub to: BlockIdExt,
    pub steps: Vec<BlockLink>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct ConfigInfo {
    #[tl(flags)]
    pub mode: (),
    pub id: BlockIdExt,
    #[derivative(Debug(format_with = "fmt_bytes"))]
    pub state_proof: Vec<u8>,
    #[derivative(Debug(format_with = "fmt_bytes"))]
    pub config_proof: Vec<u8>,
    #[tl(flags_bit = "mode.0")]
    pub with_state_root: Option<()>,
    #[tl(flags_bit = "mode.1")]
    pub with_libraries: Option<()>,
    #[tl(flags_bit = "mode.2")]
    pub with_state_extra_root: Option<()>,
    #[tl(flags_bit = "mode.3")]
    pub with_shard_hashes: Option<()>,
    #[tl(flags_bit = "mode.4")]
    pub with_validator_set: Option<()>,
    #[tl(flags_bit = "mode.5")]
    pub with_special_smc: Option<()>,
    #[tl(flags_bit = "mode.6")]
    pub with_accounts_root: Option<()>,
    #[tl(flags_bit = "mode.7")]
    pub with_prev_blocks: Option<()>,
    #[tl(flags_bit = "mode.8")]
    pub with_workchain_info: Option<()>,
    #[tl(flags_bit = "mode.9")]
    pub with_capabilities: Option<()>,
    #[tl(flags_bit = "mode.15")]
    pub extract_from_key_block: Option<()>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct ValidatorStats {
    #[tl(flags)]
    pub mode: (),
    pub id: BlockIdExt,
    pub count: u32,
    pub complete: bool,
    #[derivative(Debug(format_with = "fmt_bytes"))]
    pub state_proof: Vec<u8>,
    #[derivative(Debug(format_with = "fmt_bytes"))]
    pub data_proof: Vec<u8>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct LibraryResult {
    pub result: Vec<LibraryEntry>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct LibraryResultWithProof {
    pub id: BlockIdExt,
    #[tl(flags)]
    pub mode: (),
    pub result: Vec<LibraryEntry>,
    #[derivative(Debug(format_with = "fmt_bytes"))]
    pub state_proof: Vec<u8>,
    #[derivative(Debug(format_with = "fmt_bytes"))]
    pub data_proof: Vec<u8>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct ShardBlockLink {
    pub id: BlockIdExt,
    #[derivative(Debug(format_with = "fmt_bytes"))]
    pub proof: Vec<u8>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct ShardBlockProof {
    pub masterchain_id: BlockIdExt,
    pub links: Vec<ShardBlockLink>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct LookupBlockResult {
    pub id: BlockIdExt,
    #[tl(flags)]
    pub mode: (),
    pub mc_block_id: BlockIdExt,
    #[derivative(Debug(format_with = "fmt_bytes"))]
    pub client_mc_state_proof: Vec<u8>,
    #[derivative(Debug(format_with = "fmt_bytes"))]
    pub mc_block_proof: Vec<u8>,
    pub shard_links: Vec<ShardBlockLink>,
    #[derivative(Debug(format_with = "fmt_bytes"))]
    pub header: Vec<u8>,
    #[derivative(Debug(format_with = "fmt_bytes"))]
    pub prev_header: Vec<u8>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct OutMsgQueueSize {
    pub id: BlockIdExt,
    pub size: u32,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct OutMsgQueueSizes {
    pub shards: Vec<OutMsgQueueSize>,
    pub ext_msg_queue_size_limit: u32,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct BlockOutMsgQueueSize {
    #[tl(flags)]
    pub mode: (),
    pub id: BlockIdExt,
    pub size: u64,
    #[tl(flags_bit = "mode.0")]
    #[derivative(Debug(format_with = "fmt_opt_bytes"))]
    pub proof: Option<Vec<u8>>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct AccountDispatchQueueInfo {
    pub addr: Int256,
    pub size: u64,
    pub min_lt: u64,
    pub max_lt: u64,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct DispatchQueueInfo {
    #[tl(flags)]
    pub mode: (),
    pub id: BlockIdExt,
    pub account_dispatch_queues: Vec<AccountDispatchQueueInfo>,
    pub complete: bool,
    #[tl(flags_bit = "mode.0")]
    #[derivative(Debug(format_with = "fmt_opt_bytes"))]
    pub proof: Option<Vec<u8>>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct DispatchQueueMessage {
    pub addr: Int256,
    pub lt: u64,
    pub hash: Int256,
    pub metadata: TransactionMetadata,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct DispatchQueueMessages {
    #[tl(flags)]
    pub mode: (),
    pub id: BlockIdExt,
    pub messages: Vec<DispatchQueueMessage>,
    pub complete: bool,
    #[tl(flags_bit = "mode.0")]
    #[derivative(Debug(format_with = "fmt_opt_bytes"))]
    pub proof: Option<Vec<u8>>,
    #[tl(flags_bit = "mode.2")]
    #[derivative(Debug(format_with = "fmt_opt_bytes"))]
    pub messages_boc: Option<Vec<u8>>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct NonfinalCandidate {
    pub id: NonfinalCandidateId,
    #[derivative(Debug(format_with = "fmt_bytes"))]
    pub data: Vec<u8>,
    #[derivative(Debug(format_with = "fmt_bytes"))]
    pub collated_data: Vec<u8>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct NonfinalValidatorGroupInfo {
    pub next_block_id: BlockId,
    pub cc_seqno: i32,
    pub prev: Vec<BlockIdExt>,
    pub candidates: Vec<NonfinalCandidateInfo>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct NonfinalValidatorGroups {
    pub groups: Vec<NonfinalValidatorGroupInfo>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct NonfinalPendingShardBlocks {
    pub signed_blocks: Vec<BlockIdExt>,
    pub candidates: Vec<BlockIdExt>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct Error {
    pub code: i32,
    #[derivative(Debug(format_with = "String::fmt"))]
    pub message: String,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "code={}, message={}", self.code, self.message)
    }
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
#[tl(boxed)]
pub enum Response {
    /// liteServer.masterchainInfo last:tonNode.blockIdExt state_root_hash:int256 init:tonNode.zeroStateIdExt = liteServer.MasterchainInfo;
    #[tl(id = 0x85832881)]
    MasterchainInfo(MasterchainInfo),

    /// liteServer.masterchainInfoExt mode:# version:int capabilities:long last:tonNode.blockIdExt last_utime:int now:int state_root_hash:int256 init:tonNode.zeroStateIdExt = liteServer.MasterchainInfoExt;
    #[tl(id = 0xa8cce0f5)]
    MasterchainInfoExt(MasterchainInfoExt),

    /// liteServer.currentTime now:int = liteServer.CurrentTime;
    #[tl(id = 0xe953000d)]
    CurrentTime(CurrentTime),

    /// liteServer.version mode:# version:int capabilities:long now:int = liteServer.Version;
    #[tl(id = 0x5a0491e5)]
    Version(Version),

    /// liteServer.blockData id:tonNode.blockIdExt data:bytes = liteServer.BlockData;
    #[tl(id = 0xa574ed6c)]
    BlockData(BlockData),

    /// liteServer.blockState id:tonNode.blockIdExt root_hash:int256 file_hash:int256 data:bytes = liteServer.BlockState;
    #[tl(id = 0xabaddc0c)]
    BlockState(BlockState),

    /// liteServer.blockHeader id:tonNode.blockIdExt mode:# header_proof:bytes = liteServer.BlockHeader;
    #[tl(id = 0x752d8219)]
    BlockHeader(BlockHeader),

    /// liteServer.sendMsgStatus status:int = liteServer.SendMsgStatus;
    #[tl(id = 0x3950e597)]
    SendMsgStatus(SendMsgStatus),

    /// liteServer.accountState id:tonNode.blockIdExt shardblk:tonNode.blockIdExt shard_proof:bytes proof:bytes state:bytes = liteServer.AccountState;
    #[tl(id = 0x7079c751)]
    AccountState(AccountState),

    /// liteServer.runMethodResult mode:# id:tonNode.blockIdExt shardblk:tonNode.blockIdExt shard_proof:mode.0?bytes proof:mode.0?bytes state_proof:mode.1?bytes init_c7:mode.3?bytes lib_extras:mode.4?bytes exit_code:int result:mode.2?bytes = liteServer.RunMethodResult;
    #[tl(id = 0xa39a616b)]
    RunMethodResult(RunMethodResult),

    /// liteServer.shardInfo id:tonNode.blockIdExt shardblk:tonNode.blockIdExt shard_proof:bytes shard_descr:bytes = liteServer.ShardInfo;
    #[tl(id = 0x9fe6cd84)]
    ShardInfo(ShardInfo),

    /// liteServer.allShardsInfo id:tonNode.blockIdExt proof:bytes data:bytes = liteServer.AllShardsInfo;
    #[tl(id = 0x098fe72d)]
    AllShardsInfo(AllShardsInfo),

    /// liteServer.transactionInfo id:tonNode.blockIdExt proof:bytes transaction:bytes = liteServer.TransactionInfo;
    #[tl(id = 0x0edeed47)]
    TransactionInfo(TransactionInfo),

    /// liteServer.transactionList ids:(vector tonNode.blockIdExt) transactions:bytes = liteServer.TransactionList;
    #[tl(id = 0x6f26c60b)]
    TransactionList(TransactionList),

    /// liteServer.transactionId mode:# account:mode.0?int256 lt:mode.1?long hash:mode.2?int256 metadata:mode.8?liteServer.transactionMetadata = liteServer.TransactionId;
    #[tl(id = 0xab101c41)]
    TransactionId(TransactionId),

    /// liteServer.blockTransactions id:tonNode.blockIdExt req_count:# incomplete:Bool ids:(vector liteServer.transactionId) proof:bytes = liteServer.BlockTransactions;
    #[tl(id = 0xbd8cad2b)]
    BlockTransactions(BlockTransactions),

    /// liteServer.blockTransactionsExt id:tonNode.blockIdExt req_count:# incomplete:Bool transactions:bytes proof:bytes = liteServer.BlockTransactionsExt;
    #[tl(id = 0xfb8ffce4)]
    BlockTransactionsExt(BlockTransactionsExt),

    /// liteServer.partialBlockProof complete:Bool from:tonNode.blockIdExt to:tonNode.blockIdExt steps:(vector liteServer.BlockLink) = liteServer.PartialBlockProof;
    #[tl(id = 0x8ed0d2c1)]
    PartialBlockProof(PartialBlockProof),

    /// liteServer.configInfo mode:# id:tonNode.blockIdExt state_proof:bytes config_proof:bytes = liteServer.ConfigInfo;
    #[tl(id = 0xae7b272f)]
    ConfigInfo(ConfigInfo),

    /// liteServer.validatorStats mode:# id:tonNode.blockIdExt count:int complete:Bool state_proof:bytes data_proof:bytes = liteServer.ValidatorStats;
    #[tl(id = 0xb9f796d8)]
    ValidatorStats(ValidatorStats),

    /// liteServer.libraryResult result:(vector liteServer.libraryEntry) = liteServer.LibraryResult;
    #[tl(id = 0x117ab96b)]
    LibraryResult(LibraryResult),

    /// liteServer.libraryResult result:(vector liteServer.libraryEntry) = liteServer.LibraryResult;
    #[tl(id = 0x10a927bf)]
    LibraryResultWithProof(LibraryResultWithProof),

    /// liteServer.shardBlockProof masterchain_id:tonNode.blockIdExt links:(vector liteServer.shardBlockLink) = liteServer.ShardBlockProof;
    #[tl(id = 0x1d62a07a)]
    ShardBlockProof(ShardBlockProof),

    /// liteServer.lookupBlockResult id:tonNode.blockIdExt mode:# mc_block_id:tonNode.blockIdExt client_mc_state_proof:bytes mc_block_proof:bytes shard_links:(vector liteServer.shardBlockLink) header:bytes prev_header:bytes = liteServer.LookupBlockResult;
    #[tl(id = 0x99786be7)]
    LookupBlockResult(LookupBlockResult),

    /// liteServer.outMsgQueueSizes shards:(vector liteServer.outMsgQueueSize) ext_msg_queue_size_limit:int = liteServer.OutMsgQueueSizes;
    #[tl(id = 0xf8504a03)]
    OutMsgQueueSizes(OutMsgQueueSizes),

    /// liteServer.blockOutMsgQueueSize mode:# id:tonNode.blockIdExt size:long proof:mode.0?bytes = liteServer.BlockOutMsgQueueSize;
    #[tl(id = 0x8acdbe1b)]
    BlockOutMsgQueueSize(BlockOutMsgQueueSize),

    /// liteServer.dispatchQueueInfo mode:# id:tonNode.blockIdExt account_dispatch_queues:(vector liteServer.accountDispatchQueueInfo) complete:Bool proof:mode.0?bytes = liteServer.DispatchQueueInfo;
    #[tl(id = 0x5d1132d0)]
    DispatchQueueInfo(DispatchQueueInfo),

    /// liteServer.dispatchQueueMessages mode:# id:tonNode.blockIdExt messages:(vector liteServer.dispatchQueueMessage) complete:Bool proof:mode.0?bytes messages_boc:mode.2?bytes = liteServer.DispatchQueueMessages;
    #[tl(id = 0x4b407931)]
    DispatchQueueMessages(DispatchQueueMessages),

    /// liteServer.nonfinal.candidate id:liteServer.nonfinal.candidateId data:bytes collated_data:bytes = liteServer.nonfinal.Candidate;
    #[tl(id = 0x80c3468c)]
    NonfinalCandidate(NonfinalCandidate),

    /// liteServer.nonfinal.validatorGroups groups:(vector liteServer.nonfinal.validatorGroupInfo) = liteServer.nonfinal.ValidatorGroups;
    #[tl(id = 0x8d0b9dfe)]
    NonfinalValidatorGroups(NonfinalValidatorGroups),

    /// liteServer.nonfinal.pendingShardBlocks signed_blocks:(vector tonNode.blockIdExt) candidates:(vector tonNode.blockIdExt) = liteServer.nonfinal.PendingShardBlocks;
    #[tl(id = 0x1fd06e4d)]
    NonfinalPendingShardBlocks(NonfinalPendingShardBlocks),

    /// liteServer.error code:int message:string = liteServer.Error;
    #[tl(id = 0xbba9e148)]
    Error(Error),
}
