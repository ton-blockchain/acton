use derivative::Derivative;
use tl_proto::{TlRead, TlWrite};

use super::common::*;
use super::utils::*;

/// liteServer.query data:bytes = Object;
#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
#[tl(
    boxed,
    id = "liteServer.query",
    scheme_inline = r##"liteServer.query data:bytes = Object;"##
)]
pub struct LiteQuery {
    #[tl(with = "struct_as_bytes")]
    pub wrapped_request: WrappedRequest,
}

/// liteServer.query data:bytes = Object;
#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
#[tl(
    boxed,
    id = "liteServer.query",
    scheme_inline = r##"liteServer.query data:bytes = Object;"##
)]
pub struct LiteQueryRaw {
    pub data: Vec<u8>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct WrappedRequest {
    #[tl(read_with = "lossy_read")]
    pub wait_masterchain_seqno: Option<WaitMasterchainSeqno>,
    pub request: Request,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RawWrappedRequest {
    pub wait_masterchain_seqno: Option<WaitMasterchainSeqno>,
    pub request: Vec<u8>,
}

impl RawWrappedRequest {
    pub fn into_lite_query(self) -> LiteQueryRaw {
        let mut data = Vec::new();
        if let Some(wait_masterchain_seqno) = self.wait_masterchain_seqno {
            data.extend(tl_proto::serialize(wait_masterchain_seqno));
        }
        data.extend(self.request);
        LiteQueryRaw { data }
    }
}

/// liteServer.waitMasterchainSeqno seqno:int timeout_ms:int = Object;
#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
#[tl(
    boxed,
    id = "liteServer.waitMasterchainSeqno",
    scheme_inline = r##"liteServer.waitMasterchainSeqno seqno:int timeout_ms:int = Object;"##
)]
pub struct WaitMasterchainSeqno {
    pub seqno: u32,
    pub timeout_ms: u32,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct GetMasterchainInfoExt {
    pub mode: u32,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct GetBlock {
    pub id: BlockIdExt,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct GetState {
    pub id: BlockIdExt,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct GetBlockHeader {
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
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct SendMessage {
    pub body: Vec<u8>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct GetAccountState {
    pub id: BlockIdExt,
    pub account: AccountId,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct RunSmcMethod {
    pub mode: u32,
    pub id: BlockIdExt,
    pub account: AccountId,
    pub method_id: u64,
    pub params: Vec<u8>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct GetShardInfo {
    pub id: BlockIdExt,
    pub workchain: i32,
    pub shard: u64,
    pub exact: bool,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct GetAllShardsInfo {
    pub id: BlockIdExt,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct GetOneTransaction {
    pub id: BlockIdExt,
    pub account: AccountId,
    pub lt: u64,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct GetTransactions {
    pub count: u32,
    pub account: AccountId,
    pub lt: u64,
    pub hash: Int256,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct LookupBlock {
    #[tl(flags)]
    pub mode: (),
    pub id: BlockId,
    #[tl(flags_bit = "mode.0")]
    pub seqno: Option<()>,
    #[tl(flags_bit = "mode.1")]
    pub lt: Option<u64>,
    #[tl(flags_bit = "mode.2")]
    pub utime: Option<u32>,
    #[tl(flags_bit = "mode.4")]
    pub with_state_update: Option<()>,
    #[tl(flags_bit = "mode.5")]
    pub with_value_flow: Option<()>,
    #[tl(flags_bit = "mode.8")]
    pub with_extra: Option<()>,
    #[tl(flags_bit = "mode.9")]
    pub with_shard_hashes: Option<()>,
    #[tl(flags_bit = "mode.10")]
    pub with_prev_blk_signatures: Option<()>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct LookupBlockWithProof {
    #[tl(flags)]
    pub mode: (),
    pub id: BlockId,
    pub mc_block_id: BlockIdExt,
    #[tl(flags_bit = "mode.1")]
    pub lt: Option<u64>,
    #[tl(flags_bit = "mode.2")]
    pub utime: Option<u32>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct ListBlockTransactions {
    pub id: BlockIdExt,
    #[tl(flags)]
    pub mode: (),
    pub count: u32,
    #[tl(flags_bit = "mode.7")]
    pub after: Option<TransactionId3>,
    #[tl(flags_bit = "mode.6")]
    pub reverse_order: Option<()>,
    #[tl(flags_bit = "mode.5")]
    pub want_proof: Option<()>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct GetBlockProof {
    #[tl(flags)]
    pub mode: (),
    pub known_block: BlockIdExt,
    #[tl(flags_bit = "mode.0")]
    pub target_block: Option<BlockIdExt>,
    #[tl(flags_bit = "mode.1")]
    pub allow_weak_target: Option<()>,
    #[tl(flags_bit = "mode.12")]
    pub base_block_from_request: Option<()>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct GetConfigAll {
    #[tl(flags)]
    pub mode: (),
    pub id: BlockIdExt,
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
pub struct GetConfigParams {
    #[tl(flags)]
    pub mode: (),
    pub id: BlockIdExt,
    pub param_list: Vec<i32>,
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
pub struct GetValidatorStats {
    #[tl(flags)]
    pub mode: (),
    pub id: BlockIdExt,
    pub limit: u32,
    #[tl(flags_bit = "mode.0")]
    pub start_after: Option<Int256>,
    #[tl(flags_bit = "mode.2")]
    pub modified_after: Option<u32>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct GetLibraries {
    pub library_list: Vec<Int256>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct GetLibrariesWithProof {
    pub id: BlockIdExt,
    #[tl(flags)]
    pub mode: (),
    pub library_list: Vec<Int256>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct GetShardBlockProof {
    pub id: BlockIdExt,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct GetOutMsgQueueSizes {
    #[tl(flags)]
    pub mode: (),
    #[tl(flags_bit = "mode.0")]
    pub wc: Option<i32>,
    #[tl(flags_bit = "mode.0")]
    pub shard: Option<u64>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct GetBlockOutMsgQueueSize {
    #[tl(flags)]
    pub mode: (),
    pub id: BlockIdExt,
    #[tl(flags_bit = "mode.0")]
    pub want_proof: Option<()>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct GetDispatchQueueInfo {
    #[tl(flags)]
    pub mode: (),
    pub id: BlockIdExt,
    #[tl(flags_bit = "mode.0")]
    pub want_proof: Option<()>,
    #[tl(flags_bit = "mode.1")]
    pub after_addr: Option<Int256>,
    pub max_accounts: u32,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct GetDispatchQueueMessages {
    #[tl(flags)]
    pub mode: (),
    pub id: BlockIdExt,
    pub addr: Int256,
    pub after_lt: u64,
    pub max_messages: u32,
    #[tl(flags_bit = "mode.0")]
    pub want_proof: Option<()>,
    #[tl(flags_bit = "mode.1")]
    pub one_account: Option<()>,
    #[tl(flags_bit = "mode.2")]
    pub message_boc: Option<()>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct NonfinalGetValidatorGroups {
    #[tl(flags)]
    pub mode: (),
    #[tl(flags_bit = "mode.0")]
    pub wc: Option<i32>,
    #[tl(flags_bit = "mode.0")]
    pub shard: Option<u64>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct NonfinalGetCandidate {
    pub id: NonfinalCandidateId,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
pub struct NonfinalGetPendingShardBlocks {
    #[tl(flags)]
    pub mode: (),
    #[tl(flags_bit = "mode.0")]
    pub wc: Option<i32>,
    #[tl(flags_bit = "mode.0")]
    pub shard: Option<u64>,
}

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
#[tl(boxed)]
pub enum Request {
    /// liteServer.getMasterchainInfo = liteServer.MasterchainInfo;
    #[tl(id = 0x89b5e62e)]
    GetMasterchainInfo,

    /// liteServer.getMasterchainInfoExt mode:# = liteServer.MasterchainInfoExt;
    #[tl(id = 0x70a671df)]
    GetMasterchainInfoExt(GetMasterchainInfoExt),

    /// liteServer.getTime = liteServer.CurrentTime;
    #[tl(id = 0x16ad5a34)]
    GetTime,

    /// liteServer.getVersion = liteServer.Version;
    #[tl(id = 0x232b940b)]
    GetVersion,

    /// liteServer.getBlock id:tonNode.blockIdExt = liteServer.BlockData;
    #[tl(id = 0x6377cf0d)]
    GetBlock(GetBlock),

    /// liteServer.getState id:tonNode.blockIdExt = liteServer.BlockState;
    #[tl(id = 0xba6e2eb6)]
    GetState(GetState),

    /// liteServer.getBlockHeader id:tonNode.blockIdExt mode:# = liteServer.BlockHeader;
    #[tl(id = 0x21ec069e)]
    GetBlockHeader(GetBlockHeader),

    /// liteServer.sendMessage body:bytes = liteServer.SendMsgStatus;
    #[tl(id = 0x690ad482)]
    SendMessage(SendMessage),

    /// liteServer.getAccountState id:tonNode.blockIdExt account:liteServer.accountId = liteServer.AccountState;
    #[tl(id = 0x6b890e25)]
    GetAccountState(GetAccountState),

    /// liteServer.getAccountStatePrunned id:tonNode.blockIdExt account:liteServer.accountId = liteServer.AccountState;
    #[tl(id = 0x5a698507)]
    GetAccountStatePrunned(GetAccountState),

    /// liteServer.runSmcMethod mode:# id:tonNode.blockIdExt account:liteServer.accountId method_id:long params:bytes = liteServer.RunMethodResult;
    #[tl(id = 0x5cc65dd2)]
    RunSmcMethod(RunSmcMethod),

    /// liteServer.getShardInfo id:tonNode.blockIdExt workchain:int shard:long exact:Bool = liteServer.ShardInfo;
    #[tl(id = 0x46a2f425)]
    GetShardInfo(GetShardInfo),

    /// liteServer.getAllShardsInfo id:tonNode.blockIdExt = liteServer.AllShardsInfo;
    #[tl(id = 0x74d3fd6b)]
    GetAllShardsInfo(GetAllShardsInfo),

    /// liteServer.getOneTransaction id:tonNode.blockIdExt account:liteServer.accountId lt:long = liteServer.TransactionInfo;
    #[tl(id = 0xd40f24ea)]
    GetOneTransaction(GetOneTransaction),

    /// liteServer.getTransactions count:# account:liteServer.accountId lt:long hash:int256 = liteServer.TransactionList;
    #[tl(id = 0x1c40e7a1)]
    GetTransactions(GetTransactions),

    /// liteServer.lookupBlock mode:# id:tonNode.blockId lt:mode.1?long utime:mode.2?int = liteServer.BlockHeader;
    #[tl(id = 0xfac8f71e)]
    LookupBlock(LookupBlock),

    /// liteServer.lookupBlockWithProof mode:# id:tonNode.blockId mc_block_id:tonNode.blockIdExt lt:mode.1?long utime:mode.2?int = liteServer.LookupBlockResult;
    #[tl(id = 0x9c045ff8)]
    LookupBlockWithProof(LookupBlockWithProof),

    /// liteServer.listBlockTransactions id:tonNode.blockIdExt mode:# count:# after:mode.7?liteServer.transactionId3 reverse_order:mode.6?true want_proof:mode.5?true = liteServer.BlockTransactions;
    #[tl(id = 0xadfcc7da)]
    ListBlockTransactions(ListBlockTransactions),

    /// liteServer.listBlockTransactionsExt id:tonNode.blockIdExt mode:# count:# after:mode.7?liteServer.transactionId3 reverse_order:mode.6?true want_proof:mode.5?true = liteServer.BlockTransactionsExt;
    #[tl(id = 0x0079dd5c)]
    ListBlockTransactionsExt(ListBlockTransactions),

    /// liteServer.getBlockProof mode:# known_block:tonNode.blockIdExt target_block:mode.0?tonNode.blockIdExt = liteServer.PartialBlockProof;
    #[tl(id = 0x8aea9c44)]
    GetBlockProof(GetBlockProof),

    /// liteServer.getConfigAll mode:# id:tonNode.blockIdExt = liteServer.ConfigInfo;
    #[tl(id = 0x911b26b7)]
    GetConfigAll(GetConfigAll),

    /// liteServer.getConfigParams mode:# id:tonNode.blockIdExt param_list:(vector int) = liteServer.ConfigInfo;
    #[tl(id = 0x2a111c19)]
    GetConfigParams(GetConfigParams),

    /// liteServer.getValidatorStats#091a58bc mode:# id:tonNode.blockIdExt limit:int start_after:mode.0?int256 modified_after:mode.2?int = liteServer.ValidatorStats;
    #[tl(id = 0x091a58bc)]
    GetValidatorStats(GetValidatorStats),

    /// liteServer.getLibraries library_list:(vector int256) = liteServer.LibraryResult;
    #[tl(id = 0xd122b662)]
    GetLibraries(GetLibraries),

    /// liteServer.getLibrariesWithProof id:tonNode.blockIdExt mode:# library_list:(vector int256) = liteServer.LibraryResultWithProof;
    #[tl(id = 0xd97693bd)]
    GetLibrariesWithProof(GetLibrariesWithProof),

    /// liteServer.getShardBlockProof id:tonNode.blockIdExt = liteServer.ShardBlockProof;
    #[tl(id = 0x4ca60350)]
    GetShardBlockProof(GetShardBlockProof),

    /// liteServer.getOutMsgQueueSizes mode:# wc:mode.0?int shard:mode.0?long = liteServer.OutMsgQueueSizes;
    #[tl(id = 0x7bc19c36)]
    GetOutMsgQueueSizes(GetOutMsgQueueSizes),

    /// liteServer.getBlockOutMsgQueueSize mode:# id:tonNode.blockIdExt want_proof:mode.0?true = liteServer.BlockOutMsgQueueSize;
    #[tl(id = 0x8f6c7779)]
    GetBlockOutMsgQueueSize(GetBlockOutMsgQueueSize),

    /// liteServer.getDispatchQueueInfo mode:# id:tonNode.blockIdExt after_addr:mode.1?int256 max_accounts:int want_proof:mode.0?true = liteServer.DispatchQueueInfo;
    #[tl(id = 0x01e66bf3)]
    GetDispatchQueueInfo(GetDispatchQueueInfo),

    /// liteServer.getDispatchQueueMessages mode:# id:tonNode.blockIdExt addr:int256 after_lt:long max_messages:int want_proof:mode.0?true one_account:mode.1?true messages_boc:mode.2?true = liteServer.DispatchQueueMessages;
    #[tl(id = 0xbbfd6439)]
    GetDispatchQueueMessages(GetDispatchQueueMessages),

    /// liteServer.nonfinal.getValidatorGroups mode:# wc:mode.0?int shard:mode.0?long = liteServer.nonfinal.ValidatorGroups;
    #[tl(id = 0xa59915e3)]
    NonfinalGetValidatorGroups(NonfinalGetValidatorGroups),

    /// liteServer.nonfinal.getCandidate id:liteServer.nonfinal.candidateId = liteServer.nonfinal.Candidate;
    #[tl(id = 0x300794de)]
    NonfinalGetCandidate(NonfinalGetCandidate),

    /// liteServer.nonfinal.getPendingShardBlocks mode:# wc:mode.0?int shard:mode.0?long = liteServer.nonfinal.PendingShardBlocks;
    #[tl(id = 0x5a8ee82c)]
    NonfinalGetPendingShardBlocks(NonfinalGetPendingShardBlocks),
}
