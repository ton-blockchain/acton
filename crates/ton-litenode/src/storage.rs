use crate::types::{Addr, BocBytes, Hash256, Lt, Seqno};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, VecDeque};

pub struct CellStore {
    pub boc_by_hash: HashMap<Hash256, BocBytes>,
}

impl CellStore {
    pub fn new() -> Self {
        Self {
            boc_by_hash: HashMap::new(),
        }
    }

    pub fn put(&mut self, boc: BocBytes, hash: Hash256) -> Hash256 {
        self.boc_by_hash.insert(hash, boc);
        hash
    }

    pub fn get(&self, hash: &Hash256) -> Option<&BocBytes> {
        self.boc_by_hash.get(hash)
    }
}

impl Default for CellStore {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccountStatus {
    Active,
    Uninit,
    Frozen,
    Nonexist,
}

#[derive(Clone, Debug)]
pub struct AccountMeta {
    pub account_hash: Hash256,
    pub status: AccountStatus,
    pub balance_cache: Option<u128>,
    pub last_trans_lt: Option<Lt>,
    pub last_trans_hash: Option<Hash256>,
    pub code_hash: Option<Hash256>,
    pub data_hash: Option<Hash256>,
}

pub struct LatestState {
    pub accounts: HashMap<Addr, AccountMeta>,
}

impl LatestState {
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
        }
    }
}

impl Default for LatestState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub struct BlockMeta {
    pub seqno: Seqno,
    pub prev_seqno: Option<Seqno>,
    pub gen_utime: u32,
    pub start_lt: Lt,
    pub end_lt: Lt,
    pub tx_hash: Hash256,
    pub block_boc_hash: Hash256,
}

#[derive(Clone, Debug)]
pub struct TxMeta {
    pub tx_hash: Hash256,
    pub tx_boc_hash: Hash256,
    pub account: Addr,
    pub lt: Lt,
    pub now: u32,
    pub success: bool,
    pub compute_exit_code: Option<i32>,
    pub action_result_code: Option<i32>,
    pub total_fees: Option<u128>,
    pub in_msg_hash: Option<Hash256>,
    pub out_msg_hashes: Vec<Hash256>,
    pub block_seqno: Seqno,
}

#[derive(Clone, Debug)]
pub struct MsgMeta {
    pub msg_hash: Hash256,
    pub msg_boc_hash: Hash256,
    pub src: Option<Addr>,
    pub dst: Option<Addr>,
    pub value: Option<u128>,
    pub bounce: Option<bool>,
    pub created_lt: Option<Lt>,
    pub created_at: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct ExtendedMessage {
    pub meta: MsgMeta,
    pub boc: BocBytes,
}

#[derive(Clone, Debug)]
pub struct TransactionInfo {
    pub meta: TxMeta,
    pub in_msg: Option<ExtendedMessage>,
    pub out_msgs: Vec<ExtendedMessage>,
}

#[derive(Clone, Debug)]
pub struct AccountDelta {
    pub addr: Addr,
    pub old_hash: Option<Hash256>,
    pub new_hash: Option<Hash256>,
    pub old_meta: Option<AccountMeta>,
    pub new_meta: Option<AccountMeta>,
}

pub struct History {
    pub blocks: Vec<BlockMeta>,
    pub deltas_by_seqno: Vec<Vec<AccountDelta>>,
    pub tx_by_hash: HashMap<Hash256, TxMeta>,
    pub msg_by_hash: HashMap<Hash256, MsgMeta>,
    pub msg_to_tx: HashMap<Hash256, Hash256>,
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

impl History {
    pub fn new() -> Self {
        Self {
            blocks: Vec::new(),
            deltas_by_seqno: Vec::new(),
            tx_by_hash: HashMap::new(),
            msg_by_hash: HashMap::new(),
            msg_to_tx: HashMap::new(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct ReverseLtKey(pub core::cmp::Reverse<Lt>, pub Hash256);

pub struct Indexes {
    pub tx_by_account: HashMap<Addr, BTreeMap<ReverseLtKey, Hash256>>,
    pub tx_by_block: HashMap<Seqno, Hash256>,
}

impl Default for Indexes {
    fn default() -> Self {
        Self::new()
    }
}

impl Indexes {
    pub fn new() -> Self {
        Self {
            tx_by_account: HashMap::new(),
            tx_by_block: HashMap::new(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum QueuePolicy {
    ExternalFirstFifo,
    InternalFirstFifo,
    RoundRobinQueues,
}

pub struct Globals {
    pub head_seqno: Seqno,
    pub global_lt: Lt,
    pub lt_step: Lt,
    pub config_boc_hash: Hash256,
    pub queue_policy: QueuePolicy,
    /// Number of blocks between checkpoints (currently unused)
    pub checkpoint_every: u32,
}

impl Globals {
    pub const fn new(config_boc_hash: Hash256) -> Self {
        Self {
            head_seqno: 0,
            global_lt: 0,
            lt_step: 1,
            config_boc_hash,
            queue_policy: QueuePolicy::ExternalFirstFifo,
            checkpoint_every: 1000,
        }
    }
}

pub struct MessagePool {
    pub external: VecDeque<Hash256>,
    pub internal: VecDeque<Hash256>,
    pub rr_turn: bool,
}

impl Default for MessagePool {
    fn default() -> Self {
        Self::new()
    }
}

impl MessagePool {
    pub const fn new() -> Self {
        Self {
            external: VecDeque::new(),
            internal: VecDeque::new(),
            rr_turn: false,
        }
    }

    pub fn push_external(&mut self, msg_hash: Hash256) {
        self.external.push_back(msg_hash);
    }

    pub fn push_internal(&mut self, msg_hash: Hash256) {
        self.internal.push_back(msg_hash);
    }

    pub fn pop_next(
        &mut self,
        policy: QueuePolicy,
        _msg_meta: &HashMap<Hash256, MsgMeta>,
    ) -> Option<Hash256> {
        match policy {
            QueuePolicy::ExternalFirstFifo => self
                .external
                .pop_front()
                .or_else(|| self.internal.pop_front()),
            QueuePolicy::InternalFirstFifo => self
                .internal
                .pop_front()
                .or_else(|| self.external.pop_front()),
            QueuePolicy::RoundRobinQueues => {
                if self.rr_turn {
                    self.rr_turn = false;
                    self.internal.pop_front().or_else(|| {
                        self.rr_turn = true;
                        self.external.pop_front()
                    })
                } else {
                    self.rr_turn = true;
                    self.external.pop_front().or_else(|| {
                        self.rr_turn = false;
                        self.internal.pop_front()
                    })
                }
            }
        }
    }
}

pub struct PendingCommit {
    pub block_meta: BlockMeta,
    pub tx_meta: TxMeta,
    pub delta: AccountDelta,
    pub out_msg_hashes: Vec<Hash256>,
    pub msg_to_tx: Vec<(Hash256, Hash256)>,
}
