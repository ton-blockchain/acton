use crate::localnet::{LocalnetBlockId, LocalnetTransactionId};
use crate::types::{Addr, BocBytes, Hash256, Lt, Seqno};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::fmt::Display;
use std::sync::{Arc, Mutex};
use tycho_types::models::{StdAddr, StdAddrFormat};

pub const EMPTY_CELL_BASE64: &str = "te6cckEBAQEAAgAAAEysuc0=";

pub struct CellStore {
    pub conn: Option<Arc<Mutex<Connection>>>,
    pub boc_by_hash: HashMap<Hash256, BocBytes>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobalLibraryEntry {
    pub hash: Hash256,
    pub lib_boc: BocBytes,
    pub publishers: BTreeSet<Addr>,
    pub first_seen_lt: Lt,
    pub last_seen_lt: Lt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobalLibraryLookup {
    pub hash: Hash256,
    pub entry: Option<GlobalLibraryEntry>,
}

impl CellStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            conn: None,
            boc_by_hash: HashMap::new(),
        }
    }

    pub fn with_conn(conn: Arc<Mutex<Connection>>) -> Self {
        Self {
            conn: Some(conn),
            boc_by_hash: HashMap::new(),
        }
    }

    pub fn put(&mut self, boc: BocBytes, hash: Hash256) -> Hash256 {
        if let Some(conn) = &self.conn {
            let conn = conn.lock().expect("Failed to lock DB connection");
            let _ = conn.execute(
                "INSERT OR IGNORE INTO cas (hash, boc) VALUES (?1, ?2)",
                params![hash.0.to_vec(), boc],
            );
        } else {
            self.boc_by_hash.insert(hash, boc);
        }
        hash
    }

    #[must_use]
    pub fn get(&self, hash: &Hash256) -> Option<BocBytes> {
        if let Some(conn) = &self.conn {
            let conn = conn.lock().expect("Failed to lock DB connection");
            conn.query_row(
                "SELECT boc FROM cas WHERE hash = ?1",
                params![hash.0.to_vec()],
                |row| row.get(0),
            )
            .ok()
        } else {
            self.boc_by_hash.get(hash).cloned()
        }
    }
}

impl Default for CellStore {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountStatus {
    Active,
    Uninit,
    Frozen,
    Nonexist,
}

impl Display for AccountStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            AccountStatus::Active => "active".to_owned(),
            AccountStatus::Uninit => "uninitialized".to_owned(),
            AccountStatus::Frozen => "frozen".to_owned(),
            AccountStatus::Nonexist => "nonexist".to_owned(),
        };
        write!(f, "{str}")
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccountMeta {
    pub account_hash: Hash256,
    pub status: AccountStatus,
    pub cached_balance: Option<u128>,
    pub last_trans_lt: Option<Lt>,
    pub last_trans_hash: Option<Hash256>,
    pub code_hash: Option<Hash256>,
    pub data_hash: Option<Hash256>,
    pub frozen_hash: Option<Hash256>,
}

impl AccountMeta {
    #[must_use]
    pub fn last_tx_id(&self) -> LocalnetTransactionId {
        LocalnetTransactionId {
            lt: self.last_trans_lt.unwrap_or(0),
            hash: self.last_trans_hash.unwrap_or(Hash256([0; 32])),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JettonMasterMeta {
    pub address: Addr,
    pub admin_address: Addr,
    pub code_hash: Hash256,
    pub data_hash: Hash256,
    pub jetton_content: Value,
    pub jetton_wallet_code_hash: Hash256,
    pub last_transaction_lt: Lt,
    pub mintable: bool,
    pub total_supply: u128,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JettonWalletMeta {
    pub address: Addr,
    pub balance: u128,
    pub code_hash: Hash256,
    pub data_hash: Hash256,
    pub jetton_address: Addr,
    pub last_transaction_lt: Lt,
    pub owner_address: Addr,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NftItemMeta {
    pub address: Addr,
    pub code_hash: Hash256,
    pub data_hash: Hash256,
    pub collection_address: Option<Addr>,
    pub owner_address: Option<Addr>,
    pub content: Value,
    pub index: String,
    pub init: bool,
    pub last_transaction_lt: Lt,
}

pub struct LatestState {
    pub accounts: HashMap<Addr, AccountMeta>,
}

impl LatestState {
    #[must_use]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockMeta {
    pub seqno: Seqno,
    pub prev_seqno: Option<Seqno>,
    pub gen_utime: u32,
    pub start_lt: Lt,
    pub end_lt: Lt,
    pub tx_hash: Hash256,
    pub block_boc_hash: Hash256,
}

impl BlockMeta {
    #[must_use]
    pub const fn block_id(&self) -> LocalnetBlockId {
        LocalnetBlockId {
            workchain: 0,
            shard: -9223372036854775808,
            seqno: self.seqno,
            root_hash: self.block_boc_hash,
            file_hash: self.block_boc_hash,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TxMeta {
    pub tx_hash: Hash256,
    pub account: Addr,
    pub lt: Lt,
    pub now: u32,
    pub success: bool,
    pub compute_exit_code: Option<i32>,
    pub action_result_code: Option<i32>,
    pub total_fees: Option<u128>,
    pub storage_fees: Option<u128>,
    pub other_fees: Option<u128>,
    pub in_msg_hash: Option<Hash256>,
    pub out_msg_hashes: Vec<Hash256>,
    pub block_seqno: Seqno,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
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
pub struct MessageInfo {
    pub meta: MsgMeta,
    pub boc: BocBytes,
}

#[derive(Clone, Debug)]
pub struct TransactionInfo {
    pub meta: TxMeta,
    pub in_msg: Option<MessageInfo>,
    pub out_msgs: Vec<MessageInfo>,
    pub tx_boc: BocBytes,
}

#[derive(Clone, Debug)]
pub struct TraceNode {
    pub transaction: TransactionInfo,
    pub children: Vec<TraceNode>,
    pub external_hash: Option<Hash256>,
}

#[derive(Clone, Debug)]
pub struct EmulateTraceResult {
    pub trace: TraceNode,
    pub code_cells: HashMap<Hash256, BocBytes>,
    pub data_cells: HashMap<Hash256, BocBytes>,
    pub vm_log: String,
    pub trace_records: Vec<EmulateTraceRecord>,
}

#[derive(Clone, Debug)]
pub struct EmulateTraceRecord {
    pub raw_transaction: BocBytes,
    pub shard_account_before: BocBytes,
    pub shard_account: BocBytes,
    pub parent_transaction: Option<u64>,
    pub code: Option<BocBytes>,
    pub vm_log: String,
    pub executor_logs: String,
    pub actions: Option<String>,
}

impl TraceNode {
    #[must_use]
    pub fn max_lt(&self) -> u64 {
        let mut max = self.transaction.meta.lt;
        for child in &self.children {
            max = max.max(child.max_lt());
        }
        max
    }

    #[must_use]
    pub fn max_utime(&self) -> u32 {
        let mut max = self.transaction.meta.now;
        for child in &self.children {
            max = max.max(child.max_utime());
        }
        max
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccountDelta {
    pub addr: Addr,
    pub old_hash: Option<Hash256>,
    pub new_hash: Option<Hash256>,
    pub old_meta: Option<AccountMeta>,
    pub new_meta: Option<AccountMeta>,
}

pub struct History {
    pub conn: Option<Arc<Mutex<Connection>>>,
    pub blocks: Vec<BlockMeta>,
    pub deltas_by_seqno: Vec<Vec<AccountDelta>>,
    pub tx_by_hash: HashMap<Hash256, TxMeta>,
    pub msg_by_hash: HashMap<Hash256, MsgMeta>,
    pub msg_to_tx: HashMap<Hash256, Hash256>,
    pub address_names: HashMap<Addr, String>,
    pub jetton_masters: HashMap<Addr, JettonMasterMeta>,
    pub jetton_wallets: HashMap<Addr, JettonWalletMeta>,
    pub nft_items: HashMap<Addr, NftItemMeta>,
    pub compiler_abis: HashMap<Hash256, Value>,
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

impl History {
    #[must_use]
    pub fn new() -> Self {
        let address_names = Self::build_address_names();

        Self {
            conn: None,
            blocks: Vec::new(),
            deltas_by_seqno: Vec::new(),
            tx_by_hash: HashMap::new(),
            msg_by_hash: HashMap::new(),
            msg_to_tx: HashMap::new(),
            address_names,
            jetton_masters: HashMap::new(),
            jetton_wallets: HashMap::new(),
            nft_items: HashMap::new(),
            compiler_abis: HashMap::new(),
        }
    }

    pub fn with_conn(conn: Arc<Mutex<Connection>>) -> Self {
        let address_names = Self::build_address_names();

        Self {
            conn: Some(conn),
            blocks: Vec::new(),
            deltas_by_seqno: Vec::new(),
            tx_by_hash: HashMap::new(),
            msg_by_hash: HashMap::new(),
            msg_to_tx: HashMap::new(),
            address_names,
            jetton_masters: HashMap::new(),
            jetton_wallets: HashMap::new(),
            nft_items: HashMap::new(),
            compiler_abis: HashMap::new(),
        }
    }

    fn build_address_names() -> HashMap<Addr, String> {
        let mut address_names = HashMap::new();
        if let Ok((addr, _)) = StdAddr::from_str_ext(
            "kQBVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVfil",
            StdAddrFormat::any(),
        ) {
            address_names.insert(
                Addr {
                    workchain: i32::from(addr.workchain),
                    addr: addr.address.0,
                },
                "Faucet".to_string(),
            );
        }
        address_names
    }

    pub fn set_compiler_abi(
        &mut self,
        code_hash: Hash256,
        compiler_abi: Value,
    ) -> anyhow::Result<()> {
        if let Some(conn) = &self.conn {
            let data = serde_json::to_vec(&compiler_abi)?;
            let conn = conn.lock().expect("Failed to lock DB connection");
            conn.execute(
                "INSERT OR REPLACE INTO compiler_abis (code_hash, data) VALUES (?1, ?2)",
                params![code_hash.0.to_vec(), data],
            )?;
        }
        self.compiler_abis.insert(code_hash, compiler_abi);
        Ok(())
    }

    #[must_use]
    pub fn get_compiler_abi(&self, code_hash: &Hash256) -> Option<Value> {
        self.compiler_abis.get(code_hash).cloned()
    }

    pub fn replace_compiler_abis(
        &mut self,
        compiler_abis: HashMap<Hash256, Value>,
    ) -> anyhow::Result<()> {
        if let Some(conn) = &self.conn {
            let conn = conn.lock().expect("Failed to lock DB connection");
            conn.execute("DELETE FROM compiler_abis", [])?;
            for (code_hash, compiler_abi) in &compiler_abis {
                let data = serde_json::to_vec(compiler_abi)?;
                conn.execute(
                    "INSERT OR REPLACE INTO compiler_abis (code_hash, data) VALUES (?1, ?2)",
                    params![code_hash.0.to_vec(), data],
                )?;
            }
        }

        self.compiler_abis = compiler_abis;
        Ok(())
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
    #[must_use]
    pub fn new() -> Self {
        Self {
            tx_by_account: HashMap::new(),
            tx_by_block: HashMap::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
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
    #[must_use]
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
    #[must_use]
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
