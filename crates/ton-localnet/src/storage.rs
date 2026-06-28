use crate::localnet::{LocalnetBlockId, LocalnetTransactionId};
use crate::types::{Addr, BocBytes, Hash256, Lt, Seqno};
use dashmap::DashMap;
use indexmap::IndexMap;
use rusqlite::{Connection, params};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::fmt::Display;
use std::sync::{Arc, Mutex};
use tycho_types::boc::Boc;
use tycho_types::cell::Cell;
use tycho_types::models::{StdAddr, StdAddrFormat};

pub struct CellStore {
    pub conn: Option<Arc<Mutex<Connection>>>,
    pub boc_by_hash: FxHashMap<Hash256, BocBytes>,
    cell_by_hash: DashMap<Hash256, Cell>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobalLibraryEntry {
    pub hash: Hash256,
    pub lib_boc: BocBytes,
    pub publishers: BTreeSet<Addr>,
    pub first_seen_lt: Lt,
    pub last_seen_lt: Lt,
}

impl CellStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            conn: None,
            boc_by_hash: FxHashMap::default(),
            cell_by_hash: DashMap::new(),
        }
    }

    pub fn with_conn(conn: Arc<Mutex<Connection>>) -> Self {
        Self {
            conn: Some(conn),
            boc_by_hash: FxHashMap::default(),
            cell_by_hash: DashMap::new(),
        }
    }

    pub fn put(&mut self, boc: BocBytes, hash: Hash256) -> Hash256 {
        self.cell_by_hash.remove(&hash);

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
    pub fn get_cell(&self, hash: &Hash256) -> Option<Cell> {
        if let Some(cell) = self.cached_cell(hash) {
            return Some(cell);
        }

        let boc = self.get(hash)?;
        self.decode_and_cache_cell(*hash, &boc)
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

    #[must_use]
    pub fn find_map_cell<T>(&self, mut f: impl FnMut(&Cell) -> Option<T>) -> Option<T> {
        let Some(conn) = &self.conn else {
            return self
                .boc_by_hash
                .keys()
                .filter_map(|hash| self.get_cell(hash))
                .find_map(|cell| f(&cell));
        };

        let conn_guard = conn.lock().expect("Failed to lock DB connection");
        let Ok(mut stmt) = conn_guard.prepare("SELECT hash, boc FROM cas") else {
            return None;
        };
        let Ok(rows) = stmt.query_map([], |row| {
            let hash_bytes: Vec<u8> = row.get(0)?;
            let boc: BocBytes = row.get(1)?;
            Ok((hash_bytes, boc))
        }) else {
            return None;
        };
        let mut rows = rows;
        let mut result = None;
        for (hash_bytes, boc) in rows.by_ref().filter_map(Result::ok) {
            let Ok(hash_bytes) = <[u8; 32]>::try_from(hash_bytes.as_slice()) else {
                continue;
            };
            let hash = Hash256(hash_bytes);
            if let Some(cell) = self.decode_and_cache_cell(hash, &boc)
                && let Some(value) = f(&cell)
            {
                result = Some(value);
                break;
            }
        }
        drop(rows);
        drop(stmt);
        drop(conn_guard);
        result
    }

    #[must_use]
    pub fn find_map_value<T>(&self, mut f: impl FnMut(&BocBytes) -> Option<T>) -> Option<T> {
        let Some(conn) = &self.conn else {
            return self.boc_by_hash.values().find_map(f);
        };

        let conn_guard = conn.lock().expect("Failed to lock DB connection");
        let Ok(mut stmt) = conn_guard.prepare("SELECT boc FROM cas") else {
            return None;
        };
        let Ok(rows) = stmt.query_map([], |row| row.get::<_, BocBytes>(0)) else {
            return None;
        };
        let mut rows = rows;
        let mut result = None;
        for boc in rows.by_ref().filter_map(Result::ok) {
            if let Some(value) = f(&boc) {
                result = Some(value);
                break;
            }
        }
        drop(rows);
        drop(stmt);
        drop(conn_guard);
        result
    }

    pub fn clear_cell_cache(&self) {
        self.cell_by_hash.clear();
    }

    fn decode_and_cache_cell(&self, hash: Hash256, boc: &BocBytes) -> Option<Cell> {
        if let Some(cell) = self.cached_cell(&hash) {
            return Some(cell);
        }

        let cell = Boc::decode(boc).ok()?;
        self.cell_by_hash.insert(hash, cell.clone());
        Some(cell)
    }

    fn cached_cell(&self, hash: &Hash256) -> Option<Cell> {
        let cell = self.cell_by_hash.get(hash)?;
        Some(cell.clone())
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
    #[serde(default)]
    pub balance: u128,
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
    pub admin_address: Option<Addr>,
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
    pub tx_hashes: Vec<Hash256>,
    pub block_hash: Hash256,
    pub file_hash: Hash256,
}

impl BlockMeta {
    #[must_use]
    pub const fn block_id(&self) -> LocalnetBlockId {
        LocalnetBlockId {
            workchain: 0,
            shard: -9223372036854775808,
            seqno: self.seqno,
            root_hash: self.block_hash,
            file_hash: self.file_hash,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MasterchainBlockMeta {
    pub seqno: Seqno,
    pub prev_seqno: Option<Seqno>,
    pub gen_utime: u32,
    pub start_lt: Lt,
    pub end_lt: Lt,
    pub shard_block: LocalnetBlockId,
    pub state_root_hash: Hash256,
    pub block_hash: Hash256,
    pub file_hash: Hash256,
}

impl MasterchainBlockMeta {
    #[must_use]
    pub const fn block_id(&self) -> LocalnetBlockId {
        LocalnetBlockId {
            workchain: -1,
            shard: -9223372036854775808,
            seqno: self.seqno,
            root_hash: self.block_hash,
            file_hash: self.file_hash,
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
    #[serde(default)]
    pub total_fees: u128,
    #[serde(default)]
    pub storage_fees: u128,
    #[serde(default)]
    pub other_fees: u128,
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
    pub account_state_before: Option<AccountStateSnapshot>,
    pub account_state_after: Option<AccountStateSnapshot>,
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
}

#[derive(Clone, Debug)]
pub struct AccountStateSnapshot {
    pub hash: Hash256,
    pub balance: u128,
    pub status: AccountStatus,
    pub code: Option<Cell>,
    pub data: Option<Cell>,
    pub frozen_hash: Option<Hash256>,
}

impl AccountStateSnapshot {
    #[must_use]
    pub fn code_hash(&self) -> Option<Hash256> {
        self.code.as_ref().map(cell_hash)
    }

    #[must_use]
    pub fn data_hash(&self) -> Option<Hash256> {
        self.data.as_ref().map(cell_hash)
    }
}

fn cell_hash(cell: &Cell) -> Hash256 {
    Hash256::from(cell.repr_hash())
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
    pub masterchain_blocks: Vec<MasterchainBlockMeta>,
    pub deltas_by_seqno: Vec<Vec<AccountDelta>>,
    pub tx_by_hash: HashMap<Hash256, TxMeta>,
    pub msg_by_hash: HashMap<Hash256, MsgMeta>,
    pub msg_to_tx: HashMap<Hash256, Hash256>,
    pub address_names: HashMap<Addr, String>,
    pub jetton_masters: IndexMap<Addr, JettonMasterMeta>,
    pub jetton_wallets: IndexMap<Addr, JettonWalletMeta>,
    pub nft_items: IndexMap<Addr, NftItemMeta>,
    pub asset_detection_checked: HashSet<Addr>,
    pub compiler_abis: HashMap<Hash256, Value>,
    pub verified_sources: HashMap<Hash256, Value>,
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
            masterchain_blocks: Vec::new(),
            deltas_by_seqno: Vec::new(),
            tx_by_hash: HashMap::new(),
            msg_by_hash: HashMap::new(),
            msg_to_tx: HashMap::new(),
            address_names,
            jetton_masters: IndexMap::new(),
            jetton_wallets: IndexMap::new(),
            nft_items: IndexMap::new(),
            asset_detection_checked: HashSet::new(),
            compiler_abis: HashMap::new(),
            verified_sources: HashMap::new(),
        }
    }

    pub fn with_conn(conn: Arc<Mutex<Connection>>) -> Self {
        let address_names = Self::build_address_names();

        Self {
            conn: Some(conn),
            blocks: Vec::new(),
            masterchain_blocks: Vec::new(),
            deltas_by_seqno: Vec::new(),
            tx_by_hash: HashMap::new(),
            msg_by_hash: HashMap::new(),
            msg_to_tx: HashMap::new(),
            address_names,
            jetton_masters: IndexMap::new(),
            jetton_wallets: IndexMap::new(),
            nft_items: IndexMap::new(),
            asset_detection_checked: HashSet::new(),
            compiler_abis: HashMap::new(),
            verified_sources: HashMap::new(),
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
        let aliases = compiler_abi_code_hashes(&compiler_abi, Some(code_hash));
        let stale_keys = self
            .compiler_abis
            .iter()
            .filter_map(|(existing_hash, existing_abi)| {
                let existing_aliases = compiler_abi_code_hashes(existing_abi, Some(*existing_hash));
                existing_aliases
                    .iter()
                    .any(|alias| aliases.contains(alias))
                    .then_some(*existing_hash)
            })
            .collect::<Vec<_>>();

        if let Some(conn) = &self.conn {
            let data = serde_json::to_vec(&compiler_abi)?;
            let conn = conn.lock().expect("Failed to lock DB connection");
            for stale_key in &stale_keys {
                conn.execute(
                    "DELETE FROM compiler_abis WHERE code_hash = ?1",
                    params![stale_key.0.to_vec()],
                )?;
            }
            conn.execute(
                "INSERT OR REPLACE INTO compiler_abis (code_hash, data) VALUES (?1, ?2)",
                params![code_hash.0.to_vec(), data],
            )?;
        }
        for stale_key in stale_keys {
            self.compiler_abis.remove(&stale_key);
        }
        self.compiler_abis.insert(code_hash, compiler_abi);
        Ok(())
    }

    #[must_use]
    pub fn get_compiler_abi(&self, code_hash: &Hash256) -> Option<Value> {
        self.compiler_abis.get(code_hash).cloned().or_else(|| {
            self.compiler_abis.iter().find_map(|(entry_hash, abi)| {
                compiler_abi_code_hashes(abi, Some(*entry_hash))
                    .contains(code_hash)
                    .then(|| abi.clone())
            })
        })
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

    pub fn set_verified_source(&mut self, code_hash: Hash256, source: Value) -> anyhow::Result<()> {
        if let Some(conn) = &self.conn {
            let data = serde_json::to_vec(&source)?;
            let conn = conn.lock().expect("Failed to lock DB connection");
            conn.execute(
                "INSERT OR REPLACE INTO verified_sources (code_hash, data) VALUES (?1, ?2)",
                params![code_hash.0.to_vec(), data],
            )?;
        }
        self.verified_sources.insert(code_hash, source);
        Ok(())
    }

    #[must_use]
    pub fn get_verified_source(&self, code_hash: &Hash256) -> Option<Value> {
        self.verified_sources.get(code_hash).cloned()
    }

    pub fn delete_compiler_abi(&mut self, code_hash: &Hash256) -> anyhow::Result<()> {
        let delete_key = self
            .compiler_abis
            .iter()
            .find_map(|(entry_hash, abi)| {
                compiler_abi_code_hashes(abi, Some(*entry_hash))
                    .contains(code_hash)
                    .then_some(*entry_hash)
            })
            .unwrap_or(*code_hash);

        if let Some(conn) = &self.conn {
            let conn = conn.lock().expect("Failed to lock DB connection");
            conn.execute(
                "DELETE FROM compiler_abis WHERE code_hash = ?1",
                params![delete_key.0.to_vec()],
            )?;
        }
        self.compiler_abis.remove(&delete_key);
        Ok(())
    }

    pub fn delete_verified_source(&mut self, code_hash: &Hash256) -> anyhow::Result<()> {
        if let Some(conn) = &self.conn {
            let conn = conn.lock().expect("Failed to lock DB connection");
            conn.execute(
                "DELETE FROM verified_sources WHERE code_hash = ?1",
                params![code_hash.0.to_vec()],
            )?;
        }
        self.verified_sources.remove(code_hash);
        Ok(())
    }
}

fn compiler_abi_code_hashes(compiler_abi: &Value, fallback: Option<Hash256>) -> Vec<Hash256> {
    let mut code_hashes = fallback.into_iter().collect::<Vec<_>>();
    if let Some(values) = compiler_abi.get("code_hashes").and_then(Value::as_array) {
        code_hashes.extend(
            values
                .iter()
                .filter_map(Value::as_str)
                .filter_map(parse_compiler_abi_code_hash),
        );
    }
    code_hashes.sort();
    code_hashes.dedup();
    code_hashes
}

fn parse_compiler_abi_code_hash(code_hash: &str) -> Option<Hash256> {
    Hash256::from_hex(code_hash)
        .or_else(|_| Hash256::from_base64(code_hash))
        .ok()
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct ReverseLtKey(pub core::cmp::Reverse<Lt>, pub Hash256);

pub struct Indexes {
    pub account_deltas_by_addr: HashMap<Addr, BTreeMap<Seqno, AccountDelta>>,
    pub tx_by_account: HashMap<Addr, BTreeMap<ReverseLtKey, Hash256>>,
    pub tx_by_block: HashMap<Seqno, Vec<Hash256>>,
    pub tx_by_out_msg: HashMap<Hash256, Hash256>,
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
            account_deltas_by_addr: HashMap::new(),
            tx_by_account: HashMap::new(),
            tx_by_block: HashMap::new(),
            tx_by_out_msg: HashMap::new(),
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
    pub masterchain_block_meta: Option<MasterchainBlockMeta>,
    pub tx_metas: Vec<TxMeta>,
    pub deltas: Vec<AccountDelta>,
    pub out_msg_hashes: Vec<Hash256>,
    pub msg_to_tx: Vec<(Hash256, Hash256)>,
    pub deferred_msg_hashes: Vec<Hash256>,
}
