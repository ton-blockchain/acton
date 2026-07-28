use crate::localnet::{LocalnetBlockId, LocalnetTransactionId};
use crate::types::{Addr, BocBytes, ExtraCurrency, Hash256, Lt, Seqno};
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
                params![hash.to_bytes(), boc],
            );
        } else {
            self.boc_by_hash.insert(hash, boc);
        }
        hash
    }

    pub fn put_cell(&mut self, cell: Cell) -> Hash256 {
        let hash = Hash256::from(cell.repr_hash());
        self.put(Boc::encode(cell).into(), hash)
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
                params![hash.to_bytes()],
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
    #[serde(default)]
    pub extra_currencies: Vec<ExtraCurrency>,
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
    #[serde(default)]
    pub jetton_wallet_code_hash: Hash256,
    pub last_transaction_lt: Lt,
    #[serde(default)]
    pub mintless_is_claimed: Option<bool>,
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

#[derive(Clone, Debug)]
pub struct DnsRecordMeta {
    pub nft_item_address: Addr,
    pub nft_item_owner: Option<Addr>,
    pub domain: String,
    pub next_resolver: Option<Addr>,
    pub wallet: Option<Addr>,
    pub site_adnl: Option<Hash256>,
    pub storage_bag_id: Option<Hash256>,
}

#[derive(Clone, Debug)]
pub struct NftCollectionMeta {
    pub address: Addr,
    pub owner_address: Option<Addr>,
    pub first_transaction_lt: Lt,
    pub last_transaction_lt: Lt,
    pub next_item_index: String,
    pub collection_content: Value,
    pub data_hash: Hash256,
    pub code_hash: Hash256,
}

#[derive(Clone, Debug)]
pub struct NftSaleMeta {
    pub kind: String,
    pub address: Addr,
    pub nft_address: Addr,
    pub nft_owner_address: Option<Addr>,
    pub marketplace_address: Option<Addr>,
    pub created_at: Option<i64>,
    pub last_transaction_lt: Lt,
    pub code_hash: Hash256,
    pub data_hash: Hash256,
    pub details: Value,
    pub related_addresses: Vec<Addr>,
}

#[derive(Clone, Debug)]
pub struct MultisigOrderMeta {
    pub address: Addr,
    pub multisig_address: Addr,
    pub first_transaction_lt: Lt,
    pub order_seqno: String,
    pub threshold: i32,
    pub sent_for_execution: bool,
    pub approvals_mask: String,
    pub approvals_num: i32,
    pub expiration_date: u64,
    pub order_boc: BocBytes,
    pub signers: Vec<Addr>,
    pub last_transaction_lt: Lt,
    pub code_hash: Hash256,
    pub data_hash: Hash256,
}

#[derive(Clone, Debug)]
pub struct MultisigMeta {
    pub address: Addr,
    pub first_transaction_lt: Lt,
    pub next_order_seqno: String,
    pub threshold: i32,
    pub signers: Vec<Addr>,
    pub proposers: Vec<Addr>,
    pub last_transaction_lt: Lt,
    pub code_hash: Hash256,
    pub data_hash: Hash256,
}

#[derive(Clone, Debug)]
pub struct VestingMeta {
    pub address: Addr,
    pub first_transaction_lt: Lt,
    pub start_time: i64,
    pub total_duration: i64,
    pub unlock_period: i64,
    pub cliff_duration: i64,
    pub sender_address: Addr,
    pub owner_address: Addr,
    pub total_amount: String,
    pub whitelist: Vec<Addr>,
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
    #[serde(default)]
    pub config_boc_hash: Hash256,
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

#[derive(Clone, Debug, Serialize)]
pub struct TxMeta {
    pub tx_hash: Hash256,
    pub account: Addr,
    pub lt: Lt,
    pub now: u32,
    pub aborted: bool,
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

#[derive(Deserialize)]
struct TxMetaWire {
    tx_hash: Hash256,
    account: Addr,
    lt: Lt,
    now: u32,
    #[serde(default)]
    aborted: Option<bool>,
    #[serde(default)]
    success: Option<bool>,
    compute_exit_code: Option<i32>,
    action_result_code: Option<i32>,
    #[serde(default)]
    total_fees: u128,
    #[serde(default)]
    storage_fees: u128,
    #[serde(default)]
    other_fees: u128,
    in_msg_hash: Option<Hash256>,
    out_msg_hashes: Vec<Hash256>,
    block_seqno: Seqno,
}

impl<'de> Deserialize<'de> for TxMeta {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = TxMetaWire::deserialize(deserializer)?;
        let aborted = wire
            .aborted
            .or_else(|| wire.success.map(|success| !success))
            .ok_or_else(|| serde::de::Error::missing_field("aborted"))?;

        Ok(Self {
            tx_hash: wire.tx_hash,
            account: wire.account,
            lt: wire.lt,
            now: wire.now,
            aborted,
            compute_exit_code: wire.compute_exit_code,
            action_result_code: wire.action_result_code,
            total_fees: wire.total_fees,
            storage_fees: wire.storage_fees,
            other_fees: wire.other_fees,
            in_msg_hash: wire.in_msg_hash,
            out_msg_hashes: wire.out_msg_hashes,
            block_seqno: wire.block_seqno,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MsgMeta {
    pub msg_hash: Hash256,
    #[serde(default)]
    pub hash_norm: Option<Hash256>,
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
    pub external_hash_norm: Option<Hash256>,
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
    pub fn effective_external_hash_norm(&self) -> Hash256 {
        self.external_hash_norm
            .or(self.external_hash)
            .unwrap_or(self.transaction.meta.tx_hash)
    }

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
    pub blocks: Vec<BlockMeta>,
    pub masterchain_blocks: Vec<MasterchainBlockMeta>,
    pub deltas_by_seqno: Vec<Vec<AccountDelta>>,
    pub tx_by_hash: HashMap<Hash256, TxMeta>,
    pub msg_by_hash: HashMap<Hash256, MsgMeta>,
    pub msg_to_tx: HashMap<Hash256, Hash256>,
    pub jetton_masters: IndexMap<Addr, JettonMasterMeta>,
    pub jetton_wallets: IndexMap<Addr, JettonWalletMeta>,
    pub nft_items: IndexMap<Addr, NftItemMeta>,
    pub asset_detection_checked: HashSet<Addr>,
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

impl History {
    #[must_use]
    pub fn new() -> Self {
        Self {
            blocks: Vec::new(),
            masterchain_blocks: Vec::new(),
            deltas_by_seqno: Vec::new(),
            tx_by_hash: HashMap::new(),
            msg_by_hash: HashMap::new(),
            msg_to_tx: HashMap::new(),
            jetton_masters: IndexMap::new(),
            jetton_wallets: IndexMap::new(),
            nft_items: IndexMap::new(),
            asset_detection_checked: HashSet::new(),
        }
    }
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
    /// Sequence number of the state from which this local history starts.
    ///
    /// Clean localnets use zero. Forked localnets use the pinned remote
    /// masterchain block, while `History` stores only blocks mined locally after
    /// that point.
    pub origin_seqno: Seqno,
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
            origin_seqno: 0,
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

    pub fn pop_next(&mut self, policy: QueuePolicy) -> Option<Hash256> {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(byte: u8) -> Addr {
        Addr {
            workchain: 0,
            addr: [byte; 32],
        }
    }

    #[test]
    fn tx_meta_deserializes_legacy_success_field() {
        let tx = TxMeta {
            tx_hash: Hash256([1; 32]),
            account: addr(2),
            lt: 3,
            now: 4,
            aborted: false,
            compute_exit_code: Some(0),
            action_result_code: Some(0),
            total_fees: 5,
            storage_fees: 2,
            other_fees: 3,
            in_msg_hash: Some(Hash256([6; 32])),
            out_msg_hashes: vec![Hash256([7; 32])],
            block_seqno: 8,
        };
        let mut value = serde_json::to_value(&tx).expect("transaction metadata must serialize");
        let object = value
            .as_object_mut()
            .expect("transaction metadata must serialize as an object");
        object.remove("aborted");
        object.insert("success".to_owned(), Value::Bool(true));

        let restored: TxMeta = serde_json::from_value(value.clone())
            .expect("legacy transaction metadata must deserialize");

        assert!(!restored.aborted);
        assert_eq!(restored.tx_hash, tx.tx_hash);
        assert_eq!(restored.total_fees, tx.total_fees);

        value
            .as_object_mut()
            .expect("transaction metadata must remain an object")
            .insert("success".to_owned(), Value::Bool(false));
        let restored_failure: TxMeta = serde_json::from_value(value)
            .expect("legacy failed transaction metadata must deserialize");
        assert!(restored_failure.aborted);
    }

    #[test]
    fn added_persisted_fields_default_when_reading_legacy_metadata() {
        let masterchain_block = MasterchainBlockMeta {
            seqno: 1,
            prev_seqno: None,
            gen_utime: 2,
            start_lt: 3,
            end_lt: 4,
            shard_block: LocalnetBlockId {
                workchain: 0,
                shard: i64::MIN,
                seqno: 1,
                root_hash: Hash256([5; 32]),
                file_hash: Hash256([6; 32]),
            },
            config_boc_hash: Hash256([7; 32]),
            state_root_hash: Hash256([8; 32]),
            block_hash: Hash256([9; 32]),
            file_hash: Hash256([10; 32]),
        };
        let mut block_value =
            serde_json::to_value(masterchain_block).expect("block metadata must serialize");
        block_value
            .as_object_mut()
            .expect("block metadata must serialize as an object")
            .remove("config_boc_hash");
        let restored_block: MasterchainBlockMeta = serde_json::from_value(block_value)
            .expect("legacy masterchain metadata must deserialize");
        assert_eq!(restored_block.config_boc_hash, Hash256::default());

        let wallet = JettonWalletMeta {
            address: addr(11),
            balance: 12,
            code_hash: Hash256([13; 32]),
            data_hash: Hash256([14; 32]),
            jetton_address: addr(15),
            jetton_wallet_code_hash: Hash256([16; 32]),
            last_transaction_lt: 17,
            mintless_is_claimed: Some(true),
            owner_address: addr(18),
        };
        let mut wallet_value =
            serde_json::to_value(wallet).expect("wallet metadata must serialize");
        let wallet_object = wallet_value
            .as_object_mut()
            .expect("wallet metadata must serialize as an object");
        wallet_object.remove("jetton_wallet_code_hash");
        wallet_object.remove("mintless_is_claimed");
        let restored_wallet: JettonWalletMeta = serde_json::from_value(wallet_value)
            .expect("legacy jetton wallet metadata must deserialize");
        assert_eq!(restored_wallet.jetton_wallet_code_hash, Hash256::default());
        assert_eq!(restored_wallet.mintless_is_claimed, None);
    }
}
