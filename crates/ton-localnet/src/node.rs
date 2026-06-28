use crate::LocalnetError;
use crate::block::{
    create_block_boc, create_masterchain_block_boc, create_masterchain_state_cell,
    create_shard_state_cell, file_hash as block_file_hash,
    types::{
        BlockBuildContext, BlockTransaction, BuiltShardState, MASTERCHAIN_PREV_BLOCKS_LIMIT,
        MasterchainBlockBuildContext,
    },
};
use crate::executor::{ExecContext, TvmExecutor};
use crate::localnet::{
    LocalnetAccountStateChange, LocalnetBlockId, compute_normalized_ext_in_hash,
};
use crate::remote::{
    RemoteProvider, account_meta_from_shard_account, fetch_remote_library,
    fetch_remote_shard_account,
};
use crate::storage::{self, GlobalLibraryEntry, JettonMasterMeta, NftItemMeta};
use crate::storage::{
    AccountDelta, AccountMeta, AccountStateSnapshot, AccountStatus, BlockMeta, CellStore, Globals,
    History, Indexes, LatestState, MasterchainBlockMeta, MessageInfo, MessagePool, MsgMeta,
    PendingCommit, ReverseLtKey, TraceNode, TransactionInfo, TxMeta,
};
use crate::streaming::StreamingCommitEvent;
use crate::types::{Addr, BocBytes, Hash256, Lt, Seqno};
use anyhow::Context;
use core::cmp;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;
use ton_executor::message::{PrevBlockId, PrevBlocksInfo};
use tycho_types::boc::Boc;
use tycho_types::boc::BocRepr;
use tycho_types::cell::{Cell, CellBuilder, CellFamily, Lazy, Store};
use tycho_types::models::transaction::{
    ComputePhase, ComputePhaseSkipReason, HashUpdate, OrdinaryTxInfo, SkippedComputePhase,
    Transaction,
};
use tycho_types::models::{
    Account, AccountState, AccountStatusChange, CurrencyCollection, IntAddr, IntMsgInfo, LibDescr,
    Message, MsgInfo, OptionalAccount, OwnedMessage, ShardAccount, StdAddr, StoragePhase, TxInfo,
};
use tycho_types::prelude::HashBytes;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StateSource {
    Local,
    Remote(RemoteProvider),
}

pub struct Node {
    pub cas: CellStore,
    pub latest: LatestState,
    pub history: History,
    pub indexes: Indexes,
    pub globals: Globals,
    pub pool: MessagePool,
    pub executor: Box<dyn TvmExecutor>,
    pub state_source: StateSource,
    pub conn: Option<Arc<std::sync::Mutex<rusqlite::Connection>>>,
    pub global_libraries: HashMap<Hash256, GlobalLibraryEntry>,
    pub global_libs_boc: Option<BocBytes>,
    pub global_libs_dirty: bool,
    pub streaming_events: Option<broadcast::Sender<StreamingCommitEvent>>,
    pub time_offset_seconds: i64,
    pub next_block_timestamp: Option<u32>,
    pub config_cell: Cell,
    pub latest_masterchain_state: Option<Cell>,
    pub(crate) latest_shard_state: Option<BuiltShardState>,
    pub pending_freeze_current: VecDeque<Addr>,
}

const BASECHAIN_BLOCK_LIMITS: BlockLimits = BlockLimits {
    bytes_hard_limit: 2_097_152,
    gas_hard_limit: 20_000_000,
    lt_delta_hard_limit: 10_000,
};
const CASCADE_TX_HARD_LIMIT: usize = BASECHAIN_BLOCK_LIMITS.lt_delta_hard_limit;

#[derive(Clone, Copy)]
struct BlockLimits {
    bytes_hard_limit: usize,
    gas_hard_limit: u64,
    lt_delta_hard_limit: usize,
}

#[derive(Clone, Copy, Default)]
struct BlockResourceUsage {
    bytes: usize,
    gas: u64,
    lt_delta: usize,
}

impl BlockResourceUsage {
    const fn hard_limit_reached(self, limits: BlockLimits) -> bool {
        self.bytes >= limits.bytes_hard_limit
            || self.gas >= limits.gas_hard_limit
            || self.lt_delta >= limits.lt_delta_hard_limit
    }

    const fn add_transaction(&mut self, usage: TransactionResourceUsage) {
        self.bytes = self.bytes.saturating_add(usage.bytes);
        self.gas = self.gas.saturating_add(usage.gas);
        self.lt_delta = self.lt_delta.saturating_add(1);
    }
}

#[derive(Clone, Copy)]
struct TransactionResourceUsage {
    bytes: usize,
    gas: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeClockInfo {
    pub current_unix_time: u32,
    pub time_offset_seconds: i64,
    pub next_block_timestamp: Option<u32>,
}

struct TransactionCommit {
    tx_meta: TxMeta,
    delta: AccountDelta,
    out_msg_hashes: Vec<Hash256>,
    msg_to_tx: Vec<(Hash256, Hash256)>,
    block_tx: BlockTransaction,
    resource_usage: TransactionResourceUsage,
}

const fn compute_exit_code_from_tx_info(tx_info: Option<&TxInfo>) -> Option<i32> {
    let Some(TxInfo::Ordinary(info)) = tx_info else {
        return None;
    };
    let ComputePhase::Executed(phase) = &info.compute_phase else {
        return None;
    };
    Some(phase.exit_code)
}

fn action_result_code_from_tx_info(tx_info: Option<&TxInfo>) -> Option<i32> {
    let Some(TxInfo::Ordinary(info)) = tx_info else {
        return None;
    };
    info.action_phase.as_ref().map(|phase| phase.result_code)
}

fn gas_used_from_tx_info(tx_info: Option<&TxInfo>) -> u64 {
    let Some(TxInfo::Ordinary(info)) = tx_info else {
        return 0;
    };
    let ComputePhase::Executed(phase) = &info.compute_phase else {
        return 0;
    };
    u64::from(phase.gas_used)
}

fn transaction_fee_breakdown(tx: &Transaction, tx_info: Option<&TxInfo>) -> (u128, u128) {
    let total: u128 = tx.total_fees.tokens.into();
    let storage = if let Some(TxInfo::Ordinary(info)) = tx_info {
        info.storage_phase
            .as_ref()
            .map_or(0, |phase| phase.storage_fees_collected.into())
    } else {
        0
    };
    (storage, total.saturating_sub(storage))
}

pub const GIVER_ADDR: Addr = Addr {
    workchain: 0,
    addr: [0x55; 32],
};

pub const GIVER_BALANCE: u128 = 1_000_000_000_000_000_000; // 1B GRAM

impl Node {
    pub fn new(
        executor: Box<dyn TvmExecutor>,
        config_boc: BocBytes,
        state_source: StateSource,
    ) -> anyhow::Result<Self> {
        Self::with_db_path(executor, config_boc, state_source, None::<&str>)
    }

    pub fn with_db_path<P: AsRef<std::path::Path>>(
        executor: Box<dyn TvmExecutor>,
        config_boc: BocBytes,
        state_source: StateSource,
        db_path: Option<P>,
    ) -> anyhow::Result<Self> {
        let conn_obj = if let Some(path) = db_path {
            let path = path.as_ref();
            if let Some(parent) = path.parent()
                && !parent.as_os_str().is_empty()
            {
                std::fs::create_dir_all(parent)?;
            }
            let conn = rusqlite::Connection::open(path)?;
            conn.execute(
                "CREATE TABLE IF NOT EXISTS cas (hash BLOB PRIMARY KEY, boc BLOB)",
                [],
            )?;
            conn.execute(
                "CREATE TABLE IF NOT EXISTS blocks (seqno INTEGER PRIMARY KEY, data BLOB)",
                [],
            )?;
            conn.execute(
                "CREATE TABLE IF NOT EXISTS masterchain_blocks (seqno INTEGER PRIMARY KEY, data BLOB)",
                [],
            )?;
            conn.execute(
                "CREATE TABLE IF NOT EXISTS transactions (hash BLOB PRIMARY KEY, data BLOB, account BLOB, lt INTEGER, seqno INTEGER)",
                [],
            )?;
            conn.execute(
                "CREATE TABLE IF NOT EXISTS messages (hash BLOB PRIMARY KEY, data BLOB)",
                [],
            )?;
            conn.execute(
                "CREATE TABLE IF NOT EXISTS accounts (address BLOB PRIMARY KEY, data BLOB)",
                [],
            )?;
            conn.execute(
                "CREATE TABLE IF NOT EXISTS compiler_abis (code_hash BLOB PRIMARY KEY, data BLOB)",
                [],
            )?;
            conn.execute(
                "CREATE TABLE IF NOT EXISTS verified_sources (code_hash BLOB PRIMARY KEY, data BLOB)",
                [],
            )?;
            Some(conn)
        } else {
            None
        };

        let config_cell =
            Boc::decode(&config_boc).context("Failed to decode blockchain config BOC")?;
        let config_hash = Hash256::from(config_cell.repr_hash());

        let mut history = History::new();
        let mut latest = LatestState::new();
        let mut indexes = Indexes::new();
        let mut head_seqno = 0;

        if let Some(conn) = &conn_obj {
            // Load blocks
            let mut stmt = conn.prepare("SELECT data FROM blocks ORDER BY seqno ASC")?;
            let block_iter = stmt.query_map([], |row| {
                let data: Vec<u8> = row.get(0)?;
                serde_json::from_slice::<BlockMeta>(&data)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
            })?;
            for block in block_iter {
                let block = block?;
                head_seqno = block.seqno;
                history.blocks.push(block);
            }

            let mut stmt =
                conn.prepare("SELECT data FROM masterchain_blocks ORDER BY seqno ASC")?;
            let block_iter = stmt.query_map([], |row| {
                let data: Vec<u8> = row.get(0)?;
                serde_json::from_slice::<MasterchainBlockMeta>(&data)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
            })?;
            for block in block_iter {
                history.masterchain_blocks.push(block?);
            }

            // Load transactions into indexes
            let mut stmt =
                conn.prepare("SELECT hash, data, account, lt, seqno FROM transactions")?;
            let tx_iter = stmt.query_map([], |row| {
                let hash_bytes: Vec<u8> = row.get(0)?;
                let data: Vec<u8> = row.get(1)?;
                let account_bytes: Vec<u8> = row.get(2)?;
                let lt: u64 = row.get(3)?;
                let seqno: u32 = row.get(4)?;

                let mut hash = [0u8; 32];
                hash.copy_from_slice(&hash_bytes);
                let mut addr = [0u8; 32];
                addr.copy_from_slice(&account_bytes);

                let tx_meta = serde_json::from_slice::<TxMeta>(&data)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

                Ok((
                    Hash256(hash),
                    tx_meta,
                    Addr { workchain: 0, addr },
                    lt,
                    seqno,
                ))
            })?;
            for tx in tx_iter {
                let (hash, tx_meta, addr, lt, _seqno) = tx?;
                if let Some(in_msg_hash) = tx_meta.in_msg_hash {
                    history.msg_to_tx.insert(in_msg_hash, hash);
                }
                for out_msg_hash in &tx_meta.out_msg_hashes {
                    indexes.tx_by_out_msg.insert(*out_msg_hash, hash);
                }
                history.tx_by_hash.insert(hash, tx_meta);

                let key = ReverseLtKey(cmp::Reverse(lt), hash);
                indexes
                    .tx_by_account
                    .entry(addr)
                    .or_default()
                    .insert(key, hash);
            }

            for block in &history.blocks {
                indexes
                    .tx_by_block
                    .insert(block.seqno, block.tx_hashes.clone());
            }

            // Load accounts
            let mut stmt = conn.prepare("SELECT address, data FROM accounts")?;
            let acc_iter = stmt.query_map([], |row| {
                let addr_bytes: Vec<u8> = row.get(0)?;
                let data: Vec<u8> = row.get(1)?;
                let mut addr = [0u8; 32];
                addr.copy_from_slice(&addr_bytes);
                let meta = serde_json::from_slice::<AccountMeta>(&data)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                Ok((Addr { workchain: 0, addr }, meta))
            })?;
            for acc in acc_iter {
                let (addr, meta) = acc?;
                latest.accounts.insert(addr, meta);
            }

            // Load messages
            let mut stmt = conn.prepare("SELECT hash, data FROM messages")?;
            let msg_iter = stmt.query_map([], |row| {
                let hash_bytes: Vec<u8> = row.get(0)?;
                let data: Vec<u8> = row.get(1)?;
                let mut hash = [0u8; 32];
                hash.copy_from_slice(&hash_bytes);
                let meta = serde_json::from_slice::<MsgMeta>(&data)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                Ok((Hash256(hash), meta))
            })?;
            for msg in msg_iter {
                let (hash, meta) = msg?;
                history.msg_by_hash.insert(hash, meta);
            }

            // Load compiler ABI registry
            let mut stmt = conn.prepare("SELECT code_hash, data FROM compiler_abis")?;
            let abi_iter = stmt.query_map([], |row| {
                let hash_bytes: Vec<u8> = row.get(0)?;
                let data: Vec<u8> = row.get(1)?;
                let mut hash = [0u8; 32];
                hash.copy_from_slice(&hash_bytes);
                let compiler_abi = serde_json::from_slice::<Value>(&data)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                Ok((Hash256(hash), compiler_abi))
            })?;
            for abi in abi_iter {
                let (hash, compiler_abi) = abi?;
                history.compiler_abis.insert(hash, compiler_abi);
            }

            // Load verified source registry
            let mut stmt = conn.prepare("SELECT code_hash, data FROM verified_sources")?;
            let source_iter = stmt.query_map([], |row| {
                let hash_bytes: Vec<u8> = row.get(0)?;
                let data: Vec<u8> = row.get(1)?;
                let mut hash = [0u8; 32];
                hash.copy_from_slice(&hash_bytes);
                let source = serde_json::from_slice::<Value>(&data)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                Ok((Hash256(hash), source))
            })?;
            for source in source_iter {
                let (hash, value) = source?;
                history.verified_sources.insert(hash, value);
            }
        }

        let conn = conn_obj.map(|c| Arc::new(std::sync::Mutex::new(c)));

        let mut cas = if let Some(conn) = &conn {
            CellStore::with_conn(conn.clone())
        } else {
            CellStore::new()
        };
        cas.put(config_boc, config_hash);

        if let Some(conn) = &conn {
            history.conn = Some(conn.clone());
        }

        latest
            .accounts
            .entry(GIVER_ADDR)
            .or_insert_with(|| AccountMeta {
                account_hash: Hash256([0; 32]),
                status: AccountStatus::Active,
                balance: GIVER_BALANCE,
                last_trans_lt: None,
                last_trans_hash: None,
                code_hash: None,
                data_hash: None,
                frozen_hash: None,
            });

        let mut globals = Globals::new(config_hash);
        globals.head_seqno = head_seqno;
        // Approximation of global LT
        globals.global_lt = history.blocks.last().map_or(0, |b| b.end_lt);
        let time_offset_seconds = initial_time_offset_for_blocks(&history.blocks)?;

        let mut node = Self {
            cas,
            latest,
            history,
            indexes,
            globals,
            pool: MessagePool::new(),
            executor,
            state_source,
            conn,
            global_libraries: HashMap::new(),
            global_libs_boc: None,
            global_libs_dirty: true,
            streaming_events: None,
            time_offset_seconds,
            next_block_timestamp: None,
            config_cell,
            latest_masterchain_state: None,
            latest_shard_state: None,
            pending_freeze_current: VecDeque::new(),
        };
        node.rebuild_global_libraries_from_accounts()?;
        Ok(node)
    }

    pub fn send_boc(&mut self, boc: BocBytes) -> anyhow::Result<Hash256> {
        self.send_boc_to_queue(
            boc,
            MessageKind::ExternalIn,
            "sendBoc accepts only external-in messages",
        )
    }

    pub fn send_internal_boc(&mut self, boc: BocBytes) -> anyhow::Result<Hash256> {
        self.send_boc_to_queue(
            boc,
            MessageKind::Internal,
            "acton_sendInternalMessage accepts only internal messages",
        )
    }

    fn send_boc_to_queue(
        &mut self,
        boc: BocBytes,
        expected_kind: MessageKind,
        kind_error: &'static str,
    ) -> anyhow::Result<Hash256> {
        // 1. Validate
        let hash = boc.hash()?;
        tracing::info!(
            "send_boc: msg_hash={}, current_queue={}",
            hash.to_hex(),
            self.pool.external.len() + self.pool.internal.len()
        );
        let (msg_meta, kind) = parse_msg_meta_with_kind(&boc, hash)?;
        if kind != expected_kind {
            anyhow::bail!(kind_error);
        }

        // 2. Store
        if self.cas.get(&hash).is_none() {
            self.cas.put(boc, hash);
        }

        // 3. Register MsgMeta
        self.history.msg_by_hash.insert(hash, msg_meta);

        // 4. Enqueue
        match kind {
            MessageKind::ExternalIn => self.pool.push_external(hash),
            MessageKind::Internal => self.pool.push_internal(hash),
            MessageKind::ExternalOut => unreachable!("external-out messages are rejected above"),
        }

        Ok(hash)
    }

    pub fn mine_one(&mut self) -> anyhow::Result<(BlockMeta, TxMeta)> {
        let block_meta = self.mine_block()?;
        let tx_hash = block_meta
            .tx_hashes
            .first()
            .context("Block contains no transactions")?;
        let tx_meta = self
            .history
            .tx_by_hash
            .get(tx_hash)
            .cloned()
            .context("Transaction in mined block not found")?;
        Ok((block_meta, tx_meta))
    }

    pub fn mine_block_if_pending(&mut self) -> anyhow::Result<Option<BlockMeta>> {
        if !self.has_pending_messages() {
            return Ok(None);
        }

        self.mine_block().map(Some)
    }

    pub fn mine_block(&mut self) -> anyhow::Result<BlockMeta> {
        self.mine_block_with_limits(BASECHAIN_BLOCK_LIMITS)
    }

    fn mine_block_with_limits(&mut self, block_limits: BlockLimits) -> anyhow::Result<BlockMeta> {
        let seqno = self.globals.head_seqno + 1;
        let prev_lt = self.globals.global_lt;
        let gen_utime = self.next_block_gen_utime()?;
        let initial_pending = self.pool.external.len() + self.pool.internal.len();

        let mut tx_commits = Vec::new();
        let mut block_usage = BlockResourceUsage::default();
        let mut new_msgs = VecDeque::new();
        let mut deferred_msg_hashes = Vec::new();

        for _ in 0..initial_pending {
            if block_usage.hard_limit_reached(block_limits) {
                tracing::info!(
                    "Basechain block hard limit reached in block {seqno}; leaving remaining queued messages for the next block"
                );
                break;
            }

            let Some(msg_hash) = self
                .pool
                .pop_next(self.globals.queue_policy, &self.history.msg_by_hash)
            else {
                break;
            };
            match self.execute_message_in_block(msg_hash, seqno, gen_utime) {
                Ok(commit) => {
                    self.collect_local_internal_messages(&commit.out_msg_hashes, &mut new_msgs);
                    block_usage.add_transaction(commit.resource_usage);
                    tx_commits.push(commit);
                }
                Err(e) => {
                    tracing::error!(
                        "Block collation skipped message {}: {:?}",
                        msg_hash.to_hex(),
                        e
                    );
                }
            }
        }

        let mut cascade_txs = 0usize;
        while let Some(msg_hash) = new_msgs.pop_front() {
            if block_usage.hard_limit_reached(block_limits) {
                tracing::info!(
                    "Basechain block hard limit reached in block {seqno}; deferring remaining cascade messages"
                );
                deferred_msg_hashes.push(msg_hash);
                deferred_msg_hashes.extend(new_msgs);
                break;
            }

            if cascade_txs >= CASCADE_TX_HARD_LIMIT {
                tracing::error!(
                    "Cascade transaction hard limit reached in block {seqno}; deferring remaining messages"
                );
                deferred_msg_hashes.push(msg_hash);
                deferred_msg_hashes.extend(new_msgs);
                break;
            }
            cascade_txs += 1;

            match self.execute_message_in_block(msg_hash, seqno, gen_utime) {
                Ok(commit) => {
                    self.collect_local_internal_messages(&commit.out_msg_hashes, &mut new_msgs);
                    block_usage.add_transaction(commit.resource_usage);
                    tx_commits.push(commit);
                }
                Err(e) => {
                    tracing::error!(
                        "Block collation skipped cascade message {}: {:?}",
                        msg_hash.to_hex(),
                        e
                    );
                }
            }
        }

        while let Some(addr) = self.pending_freeze_current.pop_front() {
            if block_usage.hard_limit_reached(block_limits) {
                tracing::info!(
                    "Basechain block hard limit reached in block {seqno}; deferring remaining account freezes"
                );
                self.pending_freeze_current.push_front(addr);
                break;
            }

            match self.build_freeze_account_transaction(&addr, seqno, gen_utime) {
                Ok(commit) => {
                    block_usage.add_transaction(commit.resource_usage);
                    tx_commits.push(commit);
                }
                Err(e) => {
                    tracing::error!(
                        "Block collation skipped deferred account freeze {}: {:?}",
                        addr,
                        e
                    );
                }
            }
        }

        self.commit_transaction_block(seqno, prev_lt, gen_utime, tx_commits, deferred_msg_hashes)
    }

    fn commit_transaction_block(
        &mut self,
        seqno: Seqno,
        prev_lt: Lt,
        gen_utime: u32,
        tx_commits: Vec<TransactionCommit>,
        deferred_msg_hashes: Vec<Hash256>,
    ) -> anyhow::Result<BlockMeta> {
        let tx_hashes = tx_commits
            .iter()
            .map(|commit| commit.tx_meta.tx_hash)
            .collect::<Vec<_>>();
        let start_lt = tx_commits
            .first()
            .map_or(prev_lt, |commit| commit.tx_meta.lt);
        let end_lt = tx_commits
            .last()
            .map_or(prev_lt, |commit| commit.tx_meta.lt);
        let block_transactions = tx_commits
            .iter()
            .map(|commit| commit.block_tx.clone())
            .collect::<Vec<_>>();
        let prev_masterchain_block = self.history.masterchain_blocks.last().cloned();
        let prev_masterchain_blocks = self
            .history
            .masterchain_blocks
            .iter()
            .rev()
            .take(MASTERCHAIN_PREV_BLOCKS_LIMIT)
            .cloned()
            .collect::<Vec<_>>();
        let prev_masterchain_state = if let Some(block) = &prev_masterchain_block {
            if let Some(state) = &self.latest_masterchain_state
                && Hash256::from(state.repr_hash()) == block.state_root_hash
            {
                Some(state.clone())
            } else {
                Some(self.get_masterchain_state_cell(block.seqno)?)
            }
        } else {
            None
        };
        let prev_shard_state = self.latest_shard_state.take();
        let block = create_block_boc(BlockBuildContext {
            seqno,
            gen_utime,
            start_lt,
            end_lt,
            prev_block: self.history.blocks.last(),
            master_ref: prev_masterchain_block.as_ref(),
            prev_state: prev_shard_state.as_ref(),
            accounts_after: &self.latest.accounts,
            transactions: &block_transactions,
            cas: &self.cas,
        })?;
        let block_hash = block.block_hash;
        let file_hash = block_file_hash(&block.block_boc);
        let next_shard_state = block.state;
        self.cas.put(block.block_boc, block_hash);

        let block_meta = BlockMeta {
            seqno,
            prev_seqno: if seqno > 1 { Some(seqno - 1) } else { None },
            gen_utime,
            start_lt,
            end_lt,
            tx_hashes,
            block_hash,
            file_hash,
        };
        if Hash256::from(self.config_cell.repr_hash()) != self.globals.config_boc_hash {
            self.config_cell = self
                .cas
                .get_cell(&self.globals.config_boc_hash)
                .context("Config missing")?;
        }
        let masterchain_block = create_masterchain_block_boc(MasterchainBlockBuildContext {
            seqno,
            gen_utime,
            start_lt,
            end_lt,
            prev_block: prev_masterchain_block.as_ref(),
            prev_state: prev_masterchain_state,
            shard_block: &block_meta,
            config_cell: &self.config_cell,
            prev_blocks: &prev_masterchain_blocks,
        })?;
        let masterchain_block_hash = masterchain_block.block_hash;
        let masterchain_file_hash = block_file_hash(&masterchain_block.block_boc);
        self.cas
            .put(masterchain_block.block_boc, masterchain_block_hash);
        let next_masterchain_state = masterchain_block.state_cell;
        let masterchain_block_meta = MasterchainBlockMeta {
            seqno,
            prev_seqno: if seqno > 1 { Some(seqno - 1) } else { None },
            gen_utime,
            start_lt,
            end_lt,
            shard_block: block_meta.block_id(),
            state_root_hash: masterchain_block.state_root_hash,
            block_hash: masterchain_block_hash,
            file_hash: masterchain_file_hash,
        };

        let pending = PendingCommit {
            block_meta: block_meta.clone(),
            masterchain_block_meta: Some(masterchain_block_meta),
            tx_metas: tx_commits
                .iter()
                .map(|commit| commit.tx_meta.clone())
                .collect(),
            deltas: tx_commits
                .iter()
                .map(|commit| commit.delta.clone())
                .collect(),
            out_msg_hashes: tx_commits
                .iter()
                .flat_map(|commit| commit.out_msg_hashes.iter().copied())
                .collect(),
            msg_to_tx: tx_commits
                .iter()
                .flat_map(|commit| commit.msg_to_tx.iter().copied())
                .collect(),
            deferred_msg_hashes,
        };

        self.apply_commit(pending)?;
        self.latest_shard_state = Some(next_shard_state);
        self.latest_masterchain_state = Some(next_masterchain_state);
        Ok(block_meta)
    }

    #[must_use]
    pub fn prev_blocks_info_at(&self, seqno: Seqno) -> PrevBlocksInfo {
        let zero_block = PrevBlockId::from(LocalnetBlockId::first());
        let mut last_mc_blocks = self
            .history
            .blocks
            .iter()
            .rev()
            .filter(|block| block.seqno <= seqno)
            .take(MASTERCHAIN_PREV_BLOCKS_LIMIT)
            .map(|block| block.block_id().into())
            .collect::<Vec<_>>();

        if last_mc_blocks.len() < MASTERCHAIN_PREV_BLOCKS_LIMIT {
            last_mc_blocks.push(zero_block.clone());
        }

        // Localnet does not model key blocks, so use the latest known MC-like block.
        let prev_key_block = last_mc_blocks[0].clone();
        let mut last_mc_blocks_100 = self
            .history
            .blocks
            .iter()
            .rev()
            .filter(|block| block.seqno <= seqno && block.seqno % 100 == 0)
            .take(MASTERCHAIN_PREV_BLOCKS_LIMIT)
            .map(|block| block.block_id().into())
            .collect::<Vec<_>>();

        if last_mc_blocks_100.len() < MASTERCHAIN_PREV_BLOCKS_LIMIT {
            last_mc_blocks_100.push(zero_block);
        }

        PrevBlocksInfo::new(last_mc_blocks, prev_key_block, last_mc_blocks_100)
    }

    #[must_use]
    pub fn prev_blocks_info_before_block(&self, seqno: Seqno) -> PrevBlocksInfo {
        self.prev_blocks_info_at(seqno.saturating_sub(1))
    }

    fn execute_message_in_block(
        &mut self,
        msg_hash: Hash256,
        seqno: Seqno,
        gen_utime: u32,
    ) -> anyhow::Result<TransactionCommit> {
        let msg_meta = self
            .history
            .msg_by_hash
            .get(&msg_hash)
            .context("Msg meta missing")?;
        let msg_boc = self
            .cas
            .get(&msg_meta.msg_boc_hash)
            .context("Msg BOC missing")?;
        let dst = msg_meta
            .dst
            .ok_or_else(|| anyhow::anyhow!("Msg has no dst"))?;

        // 3. Load old account
        let shard_account_boc = self.get_shard_account(&dst)?;
        let _ = store_account_state_cell_from_shard_account_boc(&mut self.cas, &shard_account_boc);
        let old_account_cell =
            Boc::decode(&shard_account_boc).context("Failed to decode old ShardAccount BOC")?;
        let old_account_state_hash = Hash256::from(
            old_account_cell
                .parse::<ShardAccount>()
                .context("Failed to parse old ShardAccount")?
                .account
                .inner()
                .repr_hash(),
        );
        let old_meta = self.latest.accounts.get(&dst).cloned();

        // 4. Allocate LT & time
        let lt = self.globals.global_lt + self.globals.lt_step;
        self.globals.global_lt = lt;

        // 5. Execute
        let config_boc = self
            .cas
            .get(&self.globals.config_boc_hash)
            .context("Config missing")?;
        let ctx = ExecContext {
            lt,
            gen_utime,
            rand_seed: None,
            ignore_chksig: false,
            prev_blocks_info: self.prev_blocks_info_before_block(seqno),
        };
        let provider = match &self.state_source {
            StateSource::Remote(provider) => Some(provider.clone()),
            StateSource::Local => None,
        };
        self.register_message_state_init_libraries(&dst, provider.as_ref(), &msg_boc, lt)?;
        let global_libs = self.build_vm_global_libs_boc()?;

        let exec_result = self.executor.execute(
            &shard_account_boc,
            &msg_boc,
            &ctx,
            &config_boc,
            global_libs.as_ref(),
        )?;

        // 6. Store outputs & 7. Derive hashes
        let tx_info = exec_result.tx.info.load().ok();
        let resource_usage = TransactionResourceUsage {
            bytes: exec_result.tx_boc.len(),
            gas: gas_used_from_tx_info(tx_info.as_ref()),
        };
        let tx_hash = exec_result.tx_boc.hash()?;
        self.cas.put(exec_result.tx_boc.clone(), tx_hash);
        let tx_cell =
            Boc::decode(&exec_result.tx_boc).context("Failed to decode transaction BOC")?;

        let mut balance = 0;
        let mut status = AccountStatus::Nonexist;
        let mut code_hash = None;
        let mut data_hash = None;
        let mut frozen_hash = None;

        let new_account_boc = &exec_result.new_account_boc;
        let new_account_cell =
            Boc::decode(new_account_boc).context("Failed to decode new ShardAccount BOC")?;

        let new_account_hash = Hash256::from(new_account_cell.repr_hash());
        self.cas.put(new_account_boc.clone(), new_account_hash);

        let new_shard_account = new_account_cell
            .parse::<ShardAccount>()
            .context("Failed to parse new ShardAccount")?;
        let account_state_cell = new_shard_account.account.inner().clone();
        let account_state_hash = Hash256::from(account_state_cell.repr_hash());
        self.cas
            .put(Boc::encode(account_state_cell).into(), account_state_hash);

        if let Some(acc) = new_shard_account
            .account
            .load()
            .context("Failed to load new account state")?
            .0
        {
            balance = acc.balance.tokens.into();
            status = match acc.state {
                AccountState::Uninit => AccountStatus::Uninit,
                AccountState::Active(state) => {
                    if let Some(cell) = state.code {
                        let ch = Hash256::from(cell.repr_hash());
                        let boc = Boc::encode(cell);
                        self.cas.put(boc.into(), ch);
                        code_hash = Some(ch);
                    }
                    if let Some(cell) = state.data {
                        let dh = Hash256::from(cell.repr_hash());
                        let boc = Boc::encode(cell);
                        self.cas.put(boc.into(), dh);
                        data_hash = Some(dh);
                    }
                    AccountStatus::Active
                }
                AccountState::Frozen(state) => {
                    frozen_hash = Some(Hash256(state.0));
                    AccountStatus::Frozen
                }
            };
        }

        let mut out_msg_hashes = Vec::new();
        for out_cell in &exec_result.out_msg_cells {
            let h = Hash256::from(out_cell.repr_hash());
            let out_boc = BocBytes::from(Boc::encode(out_cell.clone()));
            self.cas.put(out_boc, h);
            out_msg_hashes.push(h);

            let out_meta = parse_msg_meta_from_cell(out_cell, h)?;
            self.history.msg_by_hash.insert(h, out_meta);
        }

        let compute_exit_code = compute_exit_code_from_tx_info(tx_info.as_ref());
        let action_result_code = action_result_code_from_tx_info(tx_info.as_ref());
        let (storage_fees, other_fees) =
            transaction_fee_breakdown(&exec_result.tx, tx_info.as_ref());
        let total_fees = exec_result.tx.total_fees.tokens.into();

        let tx_meta = TxMeta {
            tx_hash,
            account: dst,
            lt,
            now: gen_utime,
            success: compute_exit_code == Some(0) && action_result_code == Some(0),
            compute_exit_code,
            action_result_code,
            total_fees,
            storage_fees,
            other_fees,
            in_msg_hash: Some(msg_hash),
            out_msg_hashes: out_msg_hashes.clone(),
            block_seqno: seqno,
        };

        // 9. Prepare deltas
        let new_meta = Some(AccountMeta {
            account_hash: new_account_hash,
            status,
            balance,
            last_trans_lt: Some(lt),
            last_trans_hash: Some(tx_hash),
            code_hash,
            data_hash,
            frozen_hash,
        });

        let delta = AccountDelta {
            addr: dst,
            old_hash: old_meta.as_ref().map(|m| m.account_hash),
            new_hash: Some(new_account_hash),
            old_meta,
            new_meta,
        };

        if let Some(new_meta) = &delta.new_meta {
            self.latest.accounts.insert(delta.addr, new_meta.clone());
        } else {
            self.latest.accounts.remove(&delta.addr);
        }
        self.update_public_libraries_from_account_diff(
            &dst,
            Some(&shard_account_boc),
            Some(new_account_boc),
            lt,
        )?;
        self.register_account_code_libraries(&dst, None, new_account_boc, lt)?;

        self.detect_assets(&dst)?;

        Ok(TransactionCommit {
            block_tx: BlockTransaction {
                tx_meta: tx_meta.clone(),
                old_meta: delta.old_meta.clone(),
                tx_cell,
                old_account_state_hash,
                new_account_state_hash: account_state_hash,
            },
            tx_meta,
            delta,
            out_msg_hashes,
            msg_to_tx: vec![(msg_hash, tx_hash)],
            resource_usage,
        })
    }

    fn collect_local_internal_messages(
        &self,
        out_msg_hashes: &[Hash256],
        new_msgs: &mut VecDeque<Hash256>,
    ) {
        for hash in out_msg_hashes {
            if self
                .history
                .msg_by_hash
                .get(hash)
                .is_some_and(|meta| meta.dst.is_some())
            {
                new_msgs.push_back(*hash);
            }
        }
    }

    pub fn iter_jetton_masters(&self) -> impl Iterator<Item = &JettonMasterMeta> {
        self.history.jetton_masters.values()
    }

    pub fn iter_jetton_wallets(&self) -> impl Iterator<Item = &storage::JettonWalletMeta> {
        self.history.jetton_wallets.values()
    }

    pub fn iter_nft_items(&self) -> impl Iterator<Item = &NftItemMeta> {
        self.history.nft_items.values()
    }

    #[must_use]
    pub fn get_libraries(&self, hashes: &[Hash256]) -> Vec<Option<GlobalLibraryEntry>> {
        hashes
            .iter()
            .map(|hash| self.global_libraries.get(hash).cloned())
            .collect()
    }

    pub(crate) fn rebuild_global_libraries_from_accounts(&mut self) -> anyhow::Result<()> {
        self.global_libraries.clear();

        let mut accounts: Vec<_> = self
            .latest
            .accounts
            .iter()
            .map(|(address, meta)| (*address, meta.clone()))
            .collect();
        accounts.sort_by_key(|(address, _)| *address);
        for (address, meta) in accounts {
            if meta.status != AccountStatus::Active {
                continue;
            }

            let Some(shard_account_boc) = self.cas.get(&meta.account_hash) else {
                continue;
            };

            let libs = Self::extract_public_libraries_from_shard_account(&shard_account_boc)?;
            for (hash, lib_cell) in libs {
                let lib_boc: BocBytes = Boc::encode(lib_cell).into();
                let lt = meta.last_trans_lt.unwrap_or(0);
                let entry =
                    self.global_libraries
                        .entry(hash)
                        .or_insert_with(|| GlobalLibraryEntry {
                            hash,
                            lib_boc: lib_boc.clone(),
                            publishers: std::collections::BTreeSet::new(),
                            first_seen_lt: lt,
                            last_seen_lt: lt,
                        });
                entry.publishers.insert(address);
                entry.first_seen_lt = entry.first_seen_lt.min(lt);
                entry.last_seen_lt = entry.last_seen_lt.max(lt);
            }
            let lt = meta.last_trans_lt.unwrap_or(0);
            self.register_account_code_libraries(&address, None, &shard_account_boc, lt)?;
        }

        self.global_libs_dirty = true;
        self.global_libs_boc = None;
        Ok(())
    }

    pub(crate) fn build_vm_global_libs_boc(&mut self) -> anyhow::Result<Option<BocBytes>> {
        if !self.global_libs_dirty {
            return Ok(self.global_libs_boc.clone());
        }

        let mut libs = tycho_types::dict::Dict::<HashBytes, LibDescr>::new();
        for (hash, entry) in &self.global_libraries {
            if entry.publishers.is_empty() {
                continue;
            }

            let lib_cell = Boc::decode(&entry.lib_boc).with_context(|| {
                format!("Failed to decode stored library BOC {}", hash.to_hex())
            })?;
            let actual_hash = Hash256::from(lib_cell.repr_hash());
            if actual_hash != *hash {
                anyhow::bail!(
                    "Stored global library hash mismatch for {}: got {}",
                    hash.to_hex(),
                    actual_hash.to_hex()
                );
            }

            let mut publishers = tycho_types::dict::Dict::<HashBytes, ()>::new();
            for publisher in &entry.publishers {
                publishers
                    .add(HashBytes(publisher.addr), ())
                    .context("Failed to add publisher to global library record")?;
            }

            libs.add(
                HashBytes(hash.0),
                LibDescr {
                    lib: lib_cell,
                    publishers,
                },
            )
            .context("Failed to add global library to VM dictionary")?;
        }

        self.global_libs_boc = libs.into_root().map(|cell| Boc::encode(cell).into());
        self.global_libs_dirty = false;
        Ok(self.global_libs_boc.clone())
    }

    fn update_public_libraries_from_account_diff(
        &mut self,
        account: &Addr,
        old_shard_account: Option<&BocBytes>,
        new_shard_account: Option<&BocBytes>,
        lt: Lt,
    ) -> anyhow::Result<()> {
        let old_public = if let Some(old) = old_shard_account {
            Self::extract_public_libraries_from_shard_account(old)?
        } else {
            HashMap::new()
        };
        let new_public = if let Some(new_) = new_shard_account {
            Self::extract_public_libraries_from_shard_account(new_)?
        } else {
            HashMap::new()
        };

        let mut all_hashes = old_public.keys().copied().collect::<HashSet<_>>();
        all_hashes.extend(new_public.keys().copied());

        for hash in all_hashes {
            let old_present = old_public.get(&hash);
            let new_present = new_public.get(&hash);

            match (old_present, new_present) {
                (Some(_), None) => {
                    if let Some(entry) = self.global_libraries.get_mut(&hash)
                        && entry.publishers.remove(account)
                    {
                        entry.last_seen_lt = lt;
                        self.global_libs_dirty = true;
                    }
                    if self
                        .global_libraries
                        .get(&hash)
                        .is_some_and(|entry| entry.publishers.is_empty())
                    {
                        self.global_libraries.remove(&hash);
                        self.global_libs_dirty = true;
                    }
                }
                (_, Some(new_lib)) => {
                    let new_hash = Hash256::from(new_lib.repr_hash());
                    if new_hash != hash {
                        anyhow::bail!(
                            "Public library hash mismatch in account {}: dict key {} != library hash {}",
                            account,
                            hash.to_hex(),
                            new_hash.to_hex()
                        );
                    }

                    let lib_boc: BocBytes = Boc::encode(new_lib.clone()).into();
                    let entry =
                        self.global_libraries
                            .entry(hash)
                            .or_insert_with(|| GlobalLibraryEntry {
                                hash,
                                lib_boc: lib_boc.clone(),
                                publishers: std::collections::BTreeSet::new(),
                                first_seen_lt: lt,
                                last_seen_lt: lt,
                            });
                    let stored_cell = Boc::decode(&entry.lib_boc)?;
                    let stored_hash = Hash256::from(stored_cell.repr_hash());
                    if stored_hash != hash {
                        anyhow::bail!(
                            "Global library store is corrupted for {} (stored hash {})",
                            hash.to_hex(),
                            stored_hash.to_hex()
                        );
                    }

                    if entry.publishers.insert(*account) {
                        entry.last_seen_lt = lt;
                        self.global_libs_dirty = true;
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn register_account_code_libraries(
        &mut self,
        account: &Addr,
        provider: Option<&RemoteProvider>,
        shard_account_boc: &BocBytes,
        lt: Lt,
    ) -> anyhow::Result<()> {
        let pending = Self::collect_code_library_refs_from_shard_account(shard_account_boc)?;
        self.register_code_library_refs(account, provider, pending, lt)
    }

    fn register_message_state_init_libraries(
        &mut self,
        account: &Addr,
        provider: Option<&RemoteProvider>,
        msg_boc: &BocBytes,
        lt: Lt,
    ) -> anyhow::Result<()> {
        let cell = Boc::decode(msg_boc).context("Failed to decode inbound message BOC")?;
        let msg = cell
            .parse::<Message<'_>>()
            .context("Failed to parse inbound message")?;
        let Some(init) = msg.init else {
            return Ok(());
        };
        let Some(code) = init.code else {
            return Ok(());
        };

        let pending = Self::collect_library_refs(&code)?;
        self.register_code_library_refs(account, provider, pending, lt)
    }

    fn register_code_library_refs(
        &mut self,
        account: &Addr,
        provider: Option<&RemoteProvider>,
        mut pending: Vec<Hash256>,
        lt: Lt,
    ) -> anyhow::Result<()> {
        let mut processed = HashSet::new();

        while let Some(hash) = pending.pop() {
            if !processed.insert(hash) {
                continue;
            }

            let lib = match self.register_existing_global_library_publisher(account, hash, lt)? {
                Some(lib) => lib,
                None => match self.cas.get(&hash) {
                    Some(lib_boc) => {
                        let lib = Boc::decode(&lib_boc).with_context(|| {
                            format!("Failed to decode cached remote library {}", hash.to_hex())
                        })?;
                        self.register_account_code_library(account, hash, &lib, lt)?;
                        lib
                    }
                    None if let Some(provider) = provider => {
                        match fetch_remote_library(&hash, provider) {
                            Ok(lib) => {
                                self.register_account_code_library(account, hash, &lib, lt)?;
                                lib
                            }
                            Err(err) => {
                                tracing::warn!(
                                    "Failed to load remote library {} for account {}: {err:#}",
                                    hash.to_hex(),
                                    account
                                );
                                continue;
                            }
                        }
                    }
                    None => continue,
                },
            };
            pending.extend(Self::collect_library_refs(&lib)?);
        }

        Ok(())
    }

    fn register_existing_global_library_publisher(
        &mut self,
        account: &Addr,
        hash: Hash256,
        lt: Lt,
    ) -> anyhow::Result<Option<Cell>> {
        let Some(entry) = self.global_libraries.get_mut(&hash) else {
            return Ok(None);
        };

        let lib = Boc::decode(&entry.lib_boc)
            .with_context(|| format!("Failed to decode global library {}", hash.to_hex()))?;

        if entry.publishers.insert(*account) {
            entry.last_seen_lt = lt;
            self.global_libs_dirty = true;
        }

        Ok(Some(lib))
    }

    fn register_account_code_library(
        &mut self,
        account: &Addr,
        hash: Hash256,
        lib: &Cell,
        lt: Lt,
    ) -> anyhow::Result<()> {
        let lib_boc: BocBytes = Boc::encode(lib.clone()).into();
        self.cas.put(lib_boc.clone(), hash);
        let entry = self
            .global_libraries
            .entry(hash)
            .or_insert_with(|| GlobalLibraryEntry {
                hash,
                lib_boc,
                publishers: std::iter::once(*account).collect(),
                first_seen_lt: lt,
                last_seen_lt: lt,
            });
        if entry.publishers.insert(*account) {
            entry.last_seen_lt = lt;
        }
        entry.first_seen_lt = entry.first_seen_lt.min(lt);
        entry.last_seen_lt = entry.last_seen_lt.max(lt);
        self.global_libs_dirty = true;
        self.global_libs_boc = None;
        Ok(())
    }

    fn collect_code_library_refs_from_shard_account(
        shard_account_boc: &BocBytes,
    ) -> anyhow::Result<Vec<Hash256>> {
        let cell = Boc::decode(shard_account_boc).context("Failed to decode shard account BOC")?;
        let shard_account = cell
            .parse::<ShardAccount>()
            .context("Failed to parse shard account")?;
        let opt_account = shard_account
            .account
            .load()
            .context("Failed to load optional account from shard account")?;

        let Some(account) = opt_account.0 else {
            return Ok(Vec::new());
        };
        let AccountState::Active(state_init) = account.state else {
            return Ok(Vec::new());
        };
        let Some(code) = state_init.code else {
            return Ok(Vec::new());
        };
        Self::collect_library_refs(&code)
    }

    fn collect_library_refs(root: &Cell) -> anyhow::Result<Vec<Hash256>> {
        let mut hashes = HashSet::new();
        let mut visited = HashSet::new();
        Self::collect_library_refs_inner(root, &mut hashes, &mut visited)?;
        Ok(hashes.into_iter().collect())
    }

    fn collect_library_refs_inner(
        cell: &Cell,
        hashes: &mut HashSet<Hash256>,
        visited: &mut HashSet<Hash256>,
    ) -> anyhow::Result<()> {
        if !visited.insert(Hash256::from(cell.repr_hash())) {
            return Ok(());
        }

        if let Some(hash) = library_ref_hash(cell)? {
            hashes.insert(hash);
        }

        for index in 0..cell.reference_count() {
            if let Some(child) = cell.reference_cloned(index) {
                Self::collect_library_refs_inner(&child, hashes, visited)?;
            }
        }
        Ok(())
    }

    fn extract_public_libraries_from_shard_account(
        shard_account_boc: &BocBytes,
    ) -> anyhow::Result<HashMap<Hash256, Cell>> {
        let cell = Boc::decode(shard_account_boc).context("Failed to decode shard account BOC")?;
        let shard_account = cell
            .parse::<ShardAccount>()
            .context("Failed to parse shard account")?;
        let opt_account = shard_account
            .account
            .load()
            .context("Failed to load optional account from shard account")?;

        let Some(account) = opt_account.0 else {
            return Ok(HashMap::new());
        };
        let AccountState::Active(state_init) = account.state else {
            return Ok(HashMap::new());
        };

        let mut result = HashMap::new();
        for item in state_init.libraries.iter() {
            let (key_hash, simple_lib) =
                item.context("Failed to read account library dictionary")?;
            let key_hash = Hash256::from(key_hash);
            let root_hash = Hash256::from(simple_lib.root.repr_hash());
            if root_hash != key_hash {
                anyhow::bail!(
                    "Malformed account library entry: key {} != root hash {}",
                    key_hash.to_hex(),
                    root_hash.to_hex()
                );
            }

            if simple_lib.public {
                result.insert(key_hash, simple_lib.root);
            }
        }

        Ok(result)
    }

    fn apply_commit(&mut self, pending: PendingCommit) -> anyhow::Result<()> {
        tracing::info!(
            "Applying block commit: seqno={}, tx_count={}",
            pending.block_meta.seqno,
            pending.tx_metas.len()
        );

        // Persistent storage
        if let Some(conn) = &self.conn {
            let conn = conn.lock().expect("Failed to lock DB connection");

            // Save block
            let block_data = serde_json::to_vec(&pending.block_meta)?;
            conn.execute(
                "INSERT OR REPLACE INTO blocks (seqno, data) VALUES (?1, ?2)",
                params![pending.block_meta.seqno, block_data],
            )?;
            if let Some(masterchain_block_meta) = &pending.masterchain_block_meta {
                let block_data = serde_json::to_vec(masterchain_block_meta)?;
                conn.execute(
                    "INSERT OR REPLACE INTO masterchain_blocks (seqno, data) VALUES (?1, ?2)",
                    params![masterchain_block_meta.seqno, block_data],
                )?;
            }

            // Save transactions
            for tx_meta in &pending.tx_metas {
                let tx_data = serde_json::to_vec(tx_meta)?;
                conn.execute(
                    "INSERT OR REPLACE INTO transactions (hash, data, account, lt, seqno) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        tx_meta.tx_hash.0.to_vec(),
                        tx_data,
                        tx_meta.account.addr.to_vec(),
                        tx_meta.lt,
                        pending.block_meta.seqno
                    ],
                )?;
            }

            // Save account state
            for delta in &pending.deltas {
                if let Some(new_meta) = &delta.new_meta {
                    let account_data = serde_json::to_vec(new_meta)?;
                    conn.execute(
                        "INSERT OR REPLACE INTO accounts (address, data) VALUES (?1, ?2)",
                        params![delta.addr.addr.to_vec(), account_data],
                    )?;
                }
            }

            // Save every message referenced by this block.
            for h in pending
                .out_msg_hashes
                .iter()
                .chain(pending.msg_to_tx.iter().map(|(msg, _)| msg))
            {
                if let Some(msg_meta) = self.history.msg_by_hash.get(h) {
                    let msg_data = serde_json::to_vec(msg_meta)?;
                    conn.execute(
                        "INSERT OR REPLACE INTO messages (hash, data) VALUES (?1, ?2)",
                        params![h.0.to_vec(), msg_data],
                    )?;
                }
            }
        }

        // Apply delta
        for delta in &pending.deltas {
            if let Some(new_meta) = &delta.new_meta {
                self.latest.accounts.insert(delta.addr, new_meta.clone());
            } else {
                self.latest.accounts.remove(&delta.addr);
            }
        }

        // History
        self.history.blocks.push(pending.block_meta.clone());
        if let Some(masterchain_block_meta) = pending.masterchain_block_meta.clone() {
            self.history.masterchain_blocks.push(masterchain_block_meta);
        }

        let seqno = pending.block_meta.seqno;
        if self.history.deltas_by_seqno.len() < seqno as usize {
            self.history
                .deltas_by_seqno
                .resize(seqno as usize, Vec::new());
        }
        // seqno is 1-based, index is seqno-1
        if seqno > 0 {
            for delta in &pending.deltas {
                self.indexes
                    .account_deltas_by_addr
                    .entry(delta.addr)
                    .or_default()
                    .insert(seqno, delta.clone());
            }
            self.history.deltas_by_seqno[seqno as usize - 1].extend(pending.deltas);
        }

        for tx_meta in &pending.tx_metas {
            self.history
                .tx_by_hash
                .insert(tx_meta.tx_hash, tx_meta.clone());
        }

        for (msg, tx) in pending.msg_to_tx {
            self.history.msg_to_tx.insert(msg, tx);
        }

        // Indexes
        for tx_meta in &pending.tx_metas {
            let key = ReverseLtKey(cmp::Reverse(tx_meta.lt), tx_meta.tx_hash);
            self.indexes
                .tx_by_account
                .entry(tx_meta.account)
                .or_default()
                .insert(key, tx_meta.tx_hash);
            self.indexes
                .tx_by_block
                .entry(seqno)
                .or_default()
                .push(tx_meta.tx_hash);
            for out_msg_hash in &tx_meta.out_msg_hashes {
                self.indexes
                    .tx_by_out_msg
                    .insert(*out_msg_hash, tx_meta.tx_hash);
            }

            if let Some(events) = &self.streaming_events {
                let _ = events.send(StreamingCommitEvent {
                    tx_hash: tx_meta.tx_hash,
                });
            }
        }

        // Enqueue out msgs deferred to future blocks.
        for h in pending.deferred_msg_hashes {
            self.pool.push_internal(h);
        }

        let remaining = self.pool.external.len() + self.pool.internal.len();
        if remaining > 0 {
            tracing::info!("Queue size after commit: {} messages remaining", remaining);
        }

        self.globals.head_seqno = seqno;

        Ok(())
    }

    pub fn get_address_information(&mut self, addr: &Addr) -> Option<AccountMeta> {
        if let Some(meta) = self.latest.accounts.get(addr) {
            return Some(meta.clone());
        }

        if let StateSource::Remote(provider) = &self.state_source {
            let provider = provider.clone();
            if let Ok(Some(_)) = self.fetch_remote_shard_account(addr, &provider) {
                return self.latest.accounts.get(addr).cloned();
            }
        }

        None
    }

    pub fn get_address_information_at_block(
        &mut self,
        addr: &Addr,
        seqno: Seqno,
    ) -> Option<AccountMeta> {
        if seqno >= self.globals.head_seqno {
            return self.get_address_information(addr);
        }

        if let Some(deltas) = self.indexes.account_deltas_by_addr.get(addr) {
            if let Some((_, delta)) = deltas.range(..=seqno).next_back() {
                return delta.new_meta.clone();
            }
            return deltas
                .values()
                .next()
                .and_then(|delta| delta.old_meta.clone());
        }

        self.get_address_information(addr)
    }

    #[must_use]
    pub fn get_cell(&self, hash: &Hash256) -> Option<BocBytes> {
        self.cas.get(hash)
    }

    #[must_use]
    pub fn get_cell_or_empty(&self, hash: Option<Hash256>) -> BocBytes {
        hash.and_then(|hash| self.get_cell(&hash))
            .unwrap_or_else(|| Boc::encode(Cell::default()).into())
    }

    #[must_use]
    pub fn get_transactions(
        &self,
        addr: &Addr,
        limit: usize,
        lt: Option<Lt>,
        hash: Option<Hash256>,
    ) -> Vec<TransactionInfo> {
        let Some(index) = self.indexes.tx_by_account.get(addr) else {
            return Vec::new();
        };

        let start_key = if let (Some(l), Some(h)) = (lt, hash) {
            ReverseLtKey(cmp::Reverse(l), h)
        } else if let Some(l) = lt {
            ReverseLtKey(cmp::Reverse(l), Hash256([0; 32]))
        } else {
            ReverseLtKey(cmp::Reverse(u64::MAX), Hash256([0; 32]))
        };

        index
            .range(start_key..)
            .take(limit)
            .filter_map(|(_, tx_hash)| self.history.tx_by_hash.get(tx_hash).cloned())
            .map(|tx| self.transaction_info_from_meta(tx))
            .collect()
    }

    #[must_use]
    pub fn get_block_header(&self, seqno: Seqno) -> Option<BlockMeta> {
        if seqno == 0 || seqno as usize > self.history.blocks.len() {
            None
        } else {
            Some(self.history.blocks[seqno as usize - 1].clone())
        }
    }

    #[must_use]
    pub fn get_masterchain_block_header(&self, seqno: Seqno) -> Option<MasterchainBlockMeta> {
        if seqno == 0 || seqno as usize > self.history.masterchain_blocks.len() {
            None
        } else {
            Some(self.history.masterchain_blocks[seqno as usize - 1].clone())
        }
    }

    /// Returns the serialized TON block `BoC` for a mined localnet block.
    ///
    /// Blocks are assembled during mining and stored in the content-addressed
    /// store under their representation hash. LiteServer-compatible tooling
    /// needs the original `BoC` bytes, not just the JSON block header metadata, so
    /// this method exposes that stored artifact without rebuilding or mutating
    /// block history.
    pub fn get_block_data(&self, seqno: Seqno) -> anyhow::Result<BocBytes> {
        let block = self
            .get_block_header(seqno)
            .ok_or(LocalnetError::BlockNotFound { seqno })?;
        self.cas
            .get(&block.block_hash)
            .ok_or_else(|| LocalnetError::BlockDataNotFound { seqno }.into())
    }

    /// Rebuilds the full post-block shard state for a mined localnet block.
    ///
    /// Stored shard block BOCs may keep their `state_update` pruned so empty
    /// blocks do not retain a full account dictionary forever. Proof builders
    /// that need to read accounts should use this reconstructed state instead of
    /// extracting `state_update.new` from the block body.
    pub fn get_shard_state_cell(&self, seqno: Seqno) -> anyhow::Result<Cell> {
        let block = self
            .get_block_header(seqno)
            .ok_or(LocalnetError::BlockNotFound { seqno })?;
        let accounts = self.accounts_after_block(seqno);

        create_shard_state_cell(
            &self.cas,
            &accounts,
            block.seqno,
            block.gen_utime,
            block.end_lt,
        )
    }

    fn accounts_after_block(&self, seqno: Seqno) -> HashMap<Addr, AccountMeta> {
        let mut accounts = self.latest.accounts.clone();
        if seqno >= self.globals.head_seqno {
            return accounts;
        }

        for deltas in self
            .history
            .deltas_by_seqno
            .iter()
            .skip(seqno as usize)
            .rev()
        {
            for delta in deltas.iter().rev() {
                if let Some(old_meta) = &delta.old_meta {
                    accounts.insert(delta.addr, old_meta.clone());
                } else {
                    accounts.remove(&delta.addr);
                }
            }
        }

        accounts
    }

    /// Returns the serialized TON masterchain block `BoC` for a mined localnet block.
    ///
    /// Masterchain blocks are mined together with basechain blocks and stored in
    /// the same content-addressed store. They contain no localnet transactions;
    /// their state anchors config and the basechain shard descriptor for the
    /// matching sequence number.
    pub fn get_masterchain_block_data(&self, seqno: Seqno) -> anyhow::Result<BocBytes> {
        let block = self
            .get_masterchain_block_header(seqno)
            .ok_or(LocalnetError::BlockNotFound { seqno })?;
        self.cas
            .get(&block.block_hash)
            .ok_or_else(|| LocalnetError::BlockDataNotFound { seqno }.into())
    }

    /// Rebuilds the full post-block masterchain state for a mined localnet block.
    ///
    /// Stored masterchain block BOCs may keep their `state_update` pruned to avoid
    /// serializing the large config subtree on every mined block. Proof builders
    /// that need to read config or shard hashes should use this full in-memory
    /// state instead of extracting `state_update.new` from the block body.
    pub fn get_masterchain_state_cell(&self, seqno: Seqno) -> anyhow::Result<Cell> {
        let block = self
            .get_masterchain_block_header(seqno)
            .ok_or(LocalnetError::BlockNotFound { seqno })?;

        if seqno == self.globals.head_seqno
            && let Some(state) = &self.latest_masterchain_state
            && Hash256::from(state.repr_hash()) == block.state_root_hash
        {
            return Ok(state.clone());
        }

        let shard_block = self
            .get_block_header(seqno)
            .ok_or(LocalnetError::BlockNotFound { seqno })?;
        let prev_block = block
            .prev_seqno
            .and_then(|prev_seqno| self.get_masterchain_block_header(prev_seqno));
        let prev_blocks = self
            .history
            .masterchain_blocks
            .iter()
            .rev()
            .filter(|prev| prev.seqno < seqno)
            .take(MASTERCHAIN_PREV_BLOCKS_LIMIT)
            .cloned()
            .collect::<Vec<_>>();

        let state = create_masterchain_state_cell(&MasterchainBlockBuildContext {
            seqno,
            gen_utime: block.gen_utime,
            start_lt: block.start_lt,
            end_lt: block.end_lt,
            prev_block: prev_block.as_ref(),
            prev_state: None,
            shard_block: &shard_block,
            config_cell: &self.config_cell,
            prev_blocks: &prev_blocks,
        })?;

        anyhow::ensure!(
            Hash256::from(state.repr_hash()) == block.state_root_hash,
            "Rebuilt masterchain state root does not match block metadata for seqno {seqno}"
        );
        Ok(state)
    }

    #[must_use]
    pub fn find_block_by_lt(&self, lt: Lt) -> Option<BlockMeta> {
        self.history
            .blocks
            .iter()
            .find(|b| lt >= b.start_lt && lt <= b.end_lt)
            .cloned()
    }

    #[must_use]
    pub fn find_masterchain_block_by_lt(&self, lt: Lt) -> Option<MasterchainBlockMeta> {
        self.history
            .masterchain_blocks
            .iter()
            .find(|b| lt >= b.start_lt && lt <= b.end_lt)
            .cloned()
    }

    #[must_use]
    pub fn find_block_by_unixtime(&self, utime: u32) -> Option<BlockMeta> {
        // Find block with gen_utime closest but not greater than utime
        self.history
            .blocks
            .iter()
            .rfind(|b| b.gen_utime <= utime)
            .cloned()
    }

    #[must_use]
    pub fn find_masterchain_block_by_unixtime(&self, utime: u32) -> Option<MasterchainBlockMeta> {
        self.history
            .masterchain_blocks
            .iter()
            .rfind(|b| b.gen_utime <= utime)
            .cloned()
    }

    #[must_use]
    pub fn get_block_transactions(&self, block_meta: &BlockMeta) -> Option<Vec<TxMeta>> {
        let tx_hashes = self
            .indexes
            .tx_by_block
            .get(&block_meta.seqno)
            .map_or(block_meta.tx_hashes.as_slice(), Vec::as_slice);
        tx_hashes
            .iter()
            .map(|tx_hash| self.history.tx_by_hash.get(tx_hash).cloned())
            .collect()
    }

    #[must_use]
    pub fn get_message_info(&self, hash: &Hash256) -> Option<MessageInfo> {
        let meta = self.history.msg_by_hash.get(hash).cloned()?;
        let boc = self.cas.get(&meta.msg_boc_hash)?;
        Some(MessageInfo { meta, boc })
    }

    #[must_use]
    pub fn get_transaction_by_hash(&self, hash: &Hash256) -> Option<TransactionInfo> {
        let tx = self.history.tx_by_hash.get(hash).cloned()?;
        Some(self.transaction_info_from_meta(tx))
    }

    fn get_rich_transaction_by_hash(
        &self,
        hash: &Hash256,
        account_state_cache: &mut HashMap<Hash256, Option<AccountStateSnapshot>>,
    ) -> Option<TransactionInfo> {
        let tx = self.history.tx_by_hash.get(hash).cloned()?;
        Some(self.rich_transaction_info_from_meta(tx, account_state_cache))
    }

    /// Fast transaction view used by list-style endpoints. It intentionally skips
    /// account state snapshots because finding them may scan the whole CAS.
    fn transaction_info_from_meta(&self, tx: TxMeta) -> TransactionInfo {
        let in_msg = tx.in_msg_hash.and_then(|h| self.get_message_info(&h));
        let out_msgs = tx
            .out_msg_hashes
            .iter()
            .filter_map(|h| self.get_message_info(h))
            .collect();
        let tx_boc = self.get_cell(&tx.tx_hash).unwrap_or_default();
        TransactionInfo {
            meta: tx,
            in_msg,
            out_msgs,
            tx_boc,
            account_state_before: None,
            account_state_after: None,
        }
    }

    /// Rich transaction view used by traces. Traces need full before/after account
    /// states, so this path pays the extra parsing and CAS lookup cost explicitly.
    fn rich_transaction_info_from_meta(
        &self,
        tx: TxMeta,
        account_state_cache: &mut HashMap<Hash256, Option<AccountStateSnapshot>>,
    ) -> TransactionInfo {
        let mut info = self.transaction_info_from_meta(tx);
        (info.account_state_before, info.account_state_after) = self
            .transaction_account_state_snapshots(
                &info.meta.tx_hash,
                &info.tx_boc,
                account_state_cache,
            );
        info
    }

    fn transaction_account_state_snapshots(
        &self,
        tx_hash: &Hash256,
        tx_boc: &BocBytes,
        account_state_cache: &mut HashMap<Hash256, Option<AccountStateSnapshot>>,
    ) -> (Option<AccountStateSnapshot>, Option<AccountStateSnapshot>) {
        let Some(state_update) = self
            .cas
            .get_cell(tx_hash)
            .or_else(|| Boc::decode(tx_boc).ok())
            .and_then(|cell| cell.parse::<Transaction>().ok())
            .and_then(|tx| tx.state_update.load().ok())
        else {
            return (None, None);
        };

        (
            self.find_account_state_snapshot_cached(
                &Hash256::from(&state_update.old),
                account_state_cache,
            ),
            self.find_account_state_snapshot_cached(
                &Hash256::from(&state_update.new),
                account_state_cache,
            ),
        )
    }

    fn find_account_state_snapshot_cached(
        &self,
        state_hash: &Hash256,
        account_state_cache: &mut HashMap<Hash256, Option<AccountStateSnapshot>>,
    ) -> Option<AccountStateSnapshot> {
        if let Some(snapshot) = account_state_cache.get(state_hash) {
            return snapshot.clone();
        }

        let snapshot = self.find_account_state_snapshot(state_hash);
        account_state_cache.insert(*state_hash, snapshot.clone());
        snapshot
    }

    fn find_account_state_snapshot(&self, state_hash: &Hash256) -> Option<AccountStateSnapshot> {
        if let Some(cell) = self.cas.get_cell(state_hash) {
            if let Some(snapshot) = account_state_snapshot_from_account_state_cell(&cell) {
                return Some(snapshot);
            }
            if let Some(snapshot) = account_state_snapshot_from_cell(&cell)
                && snapshot.hash == *state_hash
            {
                return Some(snapshot);
            }
        }

        self.cas.find_map_cell(|cell| {
            let snapshot = account_state_snapshot_from_cell(cell)?;
            (snapshot.hash == *state_hash).then_some(snapshot)
        })
    }

    pub fn get_traces(&self, tx_hash: &Hash256) -> anyhow::Result<TraceNode> {
        // 1. Find the root transaction (traverse UP)
        let mut root_hash = *tx_hash;
        let mut curr_tx_hash = *tx_hash;

        // Use a set to detect cycles and prevent infinite loops
        let mut visited_up = HashSet::new();
        visited_up.insert(curr_tx_hash);

        while let Some(tx) = self.history.tx_by_hash.get(&curr_tx_hash) {
            if let Some(in_msg_hash) = &tx.in_msg_hash {
                // Find message to see if it has a source
                if let Some(msg) = self.history.msg_by_hash.get(in_msg_hash) {
                    if msg.src.is_none() {
                        // External message, this is the root
                        root_hash = curr_tx_hash;
                        break;
                    }

                    if let Some(parent_hash) = self.indexes.tx_by_out_msg.get(in_msg_hash) {
                        if visited_up.contains(parent_hash) {
                            break;
                        }
                        root_hash = *parent_hash;
                        curr_tx_hash = *parent_hash;
                        visited_up.insert(*parent_hash);
                    } else {
                        // Source is not in our history (maybe external or pruned)
                        root_hash = curr_tx_hash;
                        break;
                    }
                } else {
                    root_hash = curr_tx_hash;
                    break;
                }
            } else {
                root_hash = curr_tx_hash;
                break;
            }
        }

        // 2. Build trace tree starting from root_hash (traverse DOWN)
        let external_hash = self.history.tx_by_hash.get(&root_hash).and_then(|tx| {
            tx.in_msg_hash.and_then(|h| {
                self.history
                    .msg_by_hash
                    .get(&h)
                    .and_then(|msg| if msg.src.is_none() { Some(h) } else { None })
            })
        });

        let mut visited_down = HashSet::new();
        let mut account_state_cache = HashMap::new();
        let mut trace = self
            .build_trace_node(&root_hash, &mut visited_down, &mut account_state_cache)
            .ok_or_else(|| anyhow::anyhow!("Root transaction not found"))?;
        trace.external_hash = external_hash;
        Ok(trace)
    }

    pub fn get_traces_by_message_hash(&self, msg_hash: &Hash256) -> anyhow::Result<TraceNode> {
        let tx_hash = self
            .history
            .msg_to_tx
            .get(msg_hash)
            .or_else(|| self.indexes.tx_by_out_msg.get(msg_hash))
            .copied()
            .or_else(|| self.find_trace_tx_hash_by_normalized_message_hash(msg_hash))
            .ok_or_else(|| anyhow::anyhow!("Trace not found for message {}", msg_hash.to_hex()))?;
        self.get_traces(&tx_hash)
    }

    fn find_trace_tx_hash_by_normalized_message_hash(&self, msg_hash: &Hash256) -> Option<Hash256> {
        self.history.tx_by_hash.values().find_map(|tx| {
            let in_msg_hash = tx.in_msg_hash?;
            let msg_meta = self.history.msg_by_hash.get(&in_msg_hash)?;
            if msg_meta.src.is_some() {
                return None;
            }

            let msg_boc = self.cas.get(&msg_meta.msg_boc_hash)?;
            let cell = Boc::decode(&msg_boc).ok()?;
            let parsed = cell.parse::<Message<'_>>().ok()?;
            let normalized = compute_normalized_ext_in_hash(&parsed).ok()?;
            (normalized == *msg_hash).then_some(tx.tx_hash)
        })
    }

    fn build_trace_node(
        &self,
        tx_hash: &Hash256,
        visited: &mut HashSet<Hash256>,
        account_state_cache: &mut HashMap<Hash256, Option<AccountStateSnapshot>>,
    ) -> Option<TraceNode> {
        if !visited.insert(*tx_hash) {
            return None;
        }

        let tx_info = self.get_rich_transaction_by_hash(tx_hash, account_state_cache)?;
        let mut children = Vec::new();

        for out_msg in &tx_info.meta.out_msg_hashes {
            if let Some(child_tx_hash) = self.history.msg_to_tx.get(out_msg)
                && let Some(child_node) =
                    self.build_trace_node(child_tx_hash, visited, account_state_cache)
            {
                children.push(child_node);
            }
        }

        Some(TraceNode {
            transaction: tx_info,
            children,
            external_hash: None,
        })
    }

    pub fn get_shard_account(&mut self, addr: &Addr) -> anyhow::Result<BocBytes> {
        if let Some(meta) = self.latest.accounts.get(addr).cloned()
            && let Some(boc) = self.cas.get(&meta.account_hash)
        {
            let provider = match &self.state_source {
                StateSource::Remote(provider) => Some(provider.clone()),
                StateSource::Local => None,
            };
            let lt = meta.last_trans_lt.unwrap_or(self.globals.global_lt);
            self.register_account_code_libraries(addr, provider.as_ref(), &boc, lt)?;
            return Ok(boc);
        }

        if let StateSource::Remote(provider) = &self.state_source {
            let provider = provider.clone();
            if let Ok(Some(boc)) = self.fetch_remote_shard_account(addr, &provider) {
                return Ok(boc);
            }
        }

        // Create empty shard account
        Self::empty_shard_account_boc()
    }

    pub fn set_shard_account(
        &mut self,
        addr: &Addr,
        shard_account_boc: BocBytes,
    ) -> anyhow::Result<()> {
        let old_boc = self.get_shard_account(addr).ok();
        self.clear_detected_assets(addr);

        let cell = Boc::decode(&shard_account_boc).context("Failed to decode ShardAccount BOC")?;
        let shard_account = cell
            .parse::<ShardAccount>()
            .context("Failed to parse ShardAccount BOC")?;
        let meta =
            account_meta_from_shard_account(&shard_account, &shard_account_boc, &mut self.cas)?;
        let _ = store_account_state_cell_from_shard_account_boc(&mut self.cas, &shard_account_boc);
        let lt = meta.last_trans_lt.unwrap_or(self.globals.global_lt);

        self.persist_account_meta(addr, &meta)?;
        self.latest.accounts.insert(*addr, meta);
        self.latest_shard_state = None;
        self.update_public_libraries_from_account_diff(
            addr,
            old_boc.as_ref(),
            Some(&shard_account_boc),
            lt,
        )?;
        let provider = match &self.state_source {
            StateSource::Remote(provider) => Some(provider.clone()),
            StateSource::Local => None,
        };
        self.register_account_code_libraries(addr, provider.as_ref(), &shard_account_boc, lt)?;
        self.detect_assets(addr)?;

        Ok(())
    }

    pub fn change_account_state(
        &mut self,
        addr: &Addr,
        change: LocalnetAccountStateChange,
        mine: bool,
    ) -> anyhow::Result<()> {
        if !mine {
            return match change {
                LocalnetAccountStateChange::FrozenFromCurrent => {
                    self.pending_freeze_current.push_back(*addr);
                    Ok(())
                }
                _ => anyhow::bail!("`mine: false` is only supported with frozen `source: current`"),
            };
        }

        let shard_account_boc = match change {
            LocalnetAccountStateChange::Nonexist => Self::empty_shard_account_boc()?,
            LocalnetAccountStateChange::Uninit { balance } => {
                Self::account_shard_account_boc(addr, AccountState::Uninit, balance)?
            }
            LocalnetAccountStateChange::FrozenFromCurrent => {
                return self.freeze_account_from_current(addr);
            }
            LocalnetAccountStateChange::Frozen {
                frozen_hash,
                balance,
            } => Self::account_shard_account_boc(
                addr,
                AccountState::Frozen(HashBytes(frozen_hash.0)),
                balance,
            )?,
        };

        self.set_shard_account(addr, shard_account_boc)
    }

    fn freeze_account_from_current(&mut self, addr: &Addr) -> anyhow::Result<()> {
        let seqno = self.globals.head_seqno + 1;
        let prev_lt = self.globals.global_lt;
        let gen_utime = self.next_block_gen_utime()?;
        let commit = self.build_freeze_account_transaction(addr, seqno, gen_utime)?;
        self.commit_transaction_block(seqno, prev_lt, gen_utime, vec![commit], Vec::new())?;
        Ok(())
    }

    fn build_freeze_account_transaction(
        &mut self,
        addr: &Addr,
        seqno: Seqno,
        gen_utime: u32,
    ) -> anyhow::Result<TransactionCommit> {
        let old_shard_account_boc = self.get_shard_account(addr)?;
        let old_shard_account_cell = Boc::decode(&old_shard_account_boc)
            .context("Failed to decode current ShardAccount BOC")?;
        let old_shard_account = old_shard_account_cell
            .parse::<ShardAccount>()
            .context("Failed to parse current ShardAccount BOC")?;
        let old_account_state_hash = Hash256::from(old_shard_account.account.inner().repr_hash());
        let old_meta = self.latest.accounts.get(addr).cloned();

        let frozen_account = Self::frozen_account_from_shard_account(addr, &old_shard_account)?;
        let new_account_state_cell =
            CellBuilder::build_from(OptionalAccount(Some(frozen_account.clone())))
                .context("Failed to serialize frozen account state")?;
        let new_account_state_hash = Hash256::from(new_account_state_cell.repr_hash());
        self.cas.put(
            Boc::encode(new_account_state_cell).into(),
            new_account_state_hash,
        );

        let lt = self.globals.global_lt + self.globals.lt_step;
        self.globals.global_lt = lt;

        let in_msg_cell = self.build_admin_state_change_message(addr, lt, gen_utime)?;
        let in_msg_boc = BocBytes::from(Boc::encode(in_msg_cell.clone()));
        let in_msg_hash = in_msg_boc.hash()?;
        self.cas.put(in_msg_boc, in_msg_hash);
        let in_msg_meta = parse_msg_meta_from_cell(&in_msg_cell, in_msg_hash)
            .context("Failed to parse synthetic freeze message")?;
        self.history.msg_by_hash.insert(in_msg_hash, in_msg_meta);

        let previous_tx = old_meta
            .as_ref()
            .and_then(|meta| Some((HashBytes(meta.last_trans_hash?.0), meta.last_trans_lt?)));
        let tx = Transaction {
            account: HashBytes(addr.addr),
            lt,
            prev_trans_hash: previous_tx.map_or(old_shard_account.last_trans_hash, |tx| tx.0),
            prev_trans_lt: previous_tx
                .as_ref()
                .map_or(old_shard_account.last_trans_lt, |tx| tx.1),
            now: gen_utime,
            out_msg_count: tycho_types::num::Uint15::ZERO,
            orig_status: old_meta
                .as_ref()
                .map_or(tycho_types::models::AccountStatus::NotExists, |meta| {
                    tycho_account_status(meta.status.clone())
                }),
            end_status: tycho_types::models::AccountStatus::Frozen,
            in_msg: Some(in_msg_cell),
            out_msgs: Default::default(),
            total_fees: CurrencyCollection::ZERO,
            state_update: Lazy::new(&HashUpdate {
                old: HashBytes(old_account_state_hash.0),
                new: HashBytes(new_account_state_hash.0),
            })
            .context("Failed to build synthetic freeze transaction state update")?,
            info: Lazy::new(&TxInfo::Ordinary(OrdinaryTxInfo {
                credit_first: false,
                storage_phase: Some(StoragePhase {
                    storage_fees_collected: Default::default(),
                    storage_fees_due: None,
                    status_change: AccountStatusChange::Frozen,
                }),
                credit_phase: None,
                compute_phase: ComputePhase::Skipped(SkippedComputePhase {
                    reason: ComputePhaseSkipReason::NoGas,
                }),
                action_phase: None,
                aborted: true,
                bounce_phase: None,
                destroyed: false,
            }))
            .context("Failed to build synthetic freeze transaction info")?,
        };
        let tx_boc = BocBytes::from(BocRepr::encode(tx)?);
        let resource_usage = TransactionResourceUsage {
            bytes: tx_boc.len(),
            gas: 0,
        };
        let tx_hash = tx_boc.hash()?;
        self.cas.put(tx_boc.clone(), tx_hash);
        let tx_cell = Boc::decode(&tx_boc).context("Failed to decode synthetic freeze tx BOC")?;

        let new_shard_account_boc = Self::shard_account_boc(
            OptionalAccount(Some(frozen_account)),
            HashBytes(tx_hash.0),
            lt,
        )?;
        let new_shard_account_cell = Boc::decode(&new_shard_account_boc)
            .context("Failed to decode frozen ShardAccount BOC")?;
        let new_shard_account = new_shard_account_cell
            .parse::<ShardAccount>()
            .context("Failed to parse frozen ShardAccount BOC")?;
        let new_meta = account_meta_from_shard_account(
            &new_shard_account,
            &new_shard_account_boc,
            &mut self.cas,
        )?;
        let _ =
            store_account_state_cell_from_shard_account_boc(&mut self.cas, &new_shard_account_boc);

        let tx_meta = TxMeta {
            tx_hash,
            account: *addr,
            lt,
            now: gen_utime,
            success: true,
            compute_exit_code: Some(0),
            action_result_code: Some(0),
            total_fees: 0,
            storage_fees: 0,
            other_fees: 0,
            in_msg_hash: Some(in_msg_hash),
            out_msg_hashes: Vec::new(),
            block_seqno: seqno,
        };

        let delta = AccountDelta {
            addr: *addr,
            old_hash: old_meta.as_ref().map(|meta| meta.account_hash),
            new_hash: Some(new_meta.account_hash),
            old_meta,
            new_meta: Some(new_meta.clone()),
        };

        self.clear_detected_assets(addr);
        self.latest.accounts.insert(*addr, new_meta);
        self.update_public_libraries_from_account_diff(
            addr,
            Some(&old_shard_account_boc),
            Some(&new_shard_account_boc),
            lt,
        )?;
        self.detect_assets(addr)?;

        Ok(TransactionCommit {
            block_tx: BlockTransaction {
                tx_meta: tx_meta.clone(),
                old_meta: delta.old_meta.clone(),
                tx_cell,
                old_account_state_hash,
                new_account_state_hash,
            },
            tx_meta,
            delta,
            out_msg_hashes: Vec::new(),
            msg_to_tx: vec![(in_msg_hash, tx_hash)],
            resource_usage,
        })
    }

    fn build_admin_state_change_message(
        &self,
        addr: &Addr,
        created_lt: Lt,
        created_at: u32,
    ) -> anyhow::Result<Cell> {
        let zero_addr = Addr {
            workchain: 0,
            addr: [0; 32],
        };
        let message_info = IntMsgInfo {
            ihr_disabled: true,
            bounce: false,
            bounced: false,
            src: zero_addr.into(),
            dst: addr.into(),
            ihr_fee: Default::default(),
            value: CurrencyCollection::ZERO,
            fwd_fee: Default::default(),
            created_lt,
            created_at,
        };
        let message = OwnedMessage {
            info: MsgInfo::Int(message_info),
            init: None,
            body: Default::default(),
            layout: None,
        };
        CellBuilder::build_from(&message).context("Failed to build synthetic freeze message")
    }

    fn persist_account_meta(&self, addr: &Addr, meta: &AccountMeta) -> anyhow::Result<()> {
        let Some(conn) = &self.conn else {
            return Ok(());
        };

        let account_data = serde_json::to_vec(meta)?;
        conn.lock().expect("Failed to lock DB connection").execute(
            "INSERT OR REPLACE INTO accounts (address, data) VALUES (?1, ?2)",
            params![addr.addr.to_vec(), account_data],
        )?;

        Ok(())
    }

    pub fn get_shard_account_at_block(
        &mut self,
        addr: &Addr,
        seqno: Option<Seqno>,
    ) -> anyhow::Result<BocBytes> {
        let Some(seqno) = seqno else {
            return self.get_shard_account(addr);
        };

        if seqno == 0 || seqno >= self.globals.head_seqno {
            return self.get_shard_account(addr);
        }

        if let Some(meta) = self.get_address_information_at_block(addr, seqno)
            && let Some(boc) = self.cas.get(&meta.account_hash)
        {
            return Ok(boc);
        }

        Self::empty_shard_account_boc()
    }

    pub fn emulate_trace_by_external_message(
        &mut self,
        boc: BocBytes,
        ignore_chksig: bool,
        mc_block_seqno: Option<Seqno>,
    ) -> anyhow::Result<storage::EmulateTraceResult> {
        let msg_hash = boc.hash()?;
        let msg_meta = parse_msg_meta(&boc, msg_hash)?;
        let dst = msg_meta
            .dst
            .ok_or_else(|| anyhow::anyhow!("Msg has no dst"))?;

        let shard_account_boc = self.get_shard_account_for_emulation(&dst, mc_block_seqno)?;
        let (lt, gen_utime, block_seqno) = self.emulation_context(mc_block_seqno)?;
        let mut code_cells = HashMap::new();
        let mut data_cells = HashMap::new();
        collect_code_data_cells(Some(&shard_account_boc), &mut code_cells, &mut data_cells);

        let config_boc = self
            .cas
            .get(&self.globals.config_boc_hash)
            .context("Config missing")?;
        let vm_global_libs = self.build_vm_global_libs_boc()?;
        let ctx = ExecContext {
            lt,
            gen_utime,
            rand_seed: None,
            ignore_chksig,
            prev_blocks_info: self.prev_blocks_info_at(block_seqno),
        };

        let exec_result = self.executor.execute(
            &shard_account_boc,
            &boc,
            &ctx,
            &config_boc,
            vm_global_libs.as_ref(),
        )?;

        let tx_hash = exec_result.tx_boc.hash()?;
        let mut out_msg_hashes = Vec::new();
        let mut out_msgs = Vec::new();
        for out_cell in &exec_result.out_msg_cells {
            let out_hash = Hash256::from(out_cell.repr_hash());
            out_msg_hashes.push(out_hash);
            let out_meta = parse_msg_meta_from_cell(out_cell, out_hash)?;
            let out_boc = BocBytes::from(Boc::encode(out_cell.clone()));
            out_msgs.push(MessageInfo {
                meta: out_meta,
                boc: out_boc,
            });
        }

        let tx_info = exec_result.tx.info.load().ok();
        let compute_exit_code = compute_exit_code_from_tx_info(tx_info.as_ref());
        let action_result_code = action_result_code_from_tx_info(tx_info.as_ref());
        let (storage_fees, other_fees) =
            transaction_fee_breakdown(&exec_result.tx, tx_info.as_ref());
        let total_fees = exec_result.tx.total_fees.tokens.into();

        let tx_meta = TxMeta {
            tx_hash,
            account: dst,
            lt,
            now: gen_utime,
            success: compute_exit_code == Some(0) && action_result_code == Some(0),
            compute_exit_code,
            action_result_code,
            total_fees,
            storage_fees,
            other_fees,
            in_msg_hash: Some(msg_hash),
            out_msg_hashes,
            block_seqno,
        };

        collect_code_data_cells(
            Some(&exec_result.new_account_boc),
            &mut code_cells,
            &mut data_cells,
        );

        Ok(storage::EmulateTraceResult {
            trace: TraceNode {
                transaction: TransactionInfo {
                    meta: tx_meta,
                    in_msg: Some(MessageInfo {
                        meta: msg_meta,
                        boc,
                    }),
                    out_msgs,
                    tx_boc: exec_result.tx_boc,
                    account_state_before: account_state_snapshot_from_boc(&shard_account_boc),
                    account_state_after: account_state_snapshot_from_boc(
                        &exec_result.new_account_boc,
                    ),
                },
                children: Vec::new(),
                external_hash: Some(msg_hash),
            },
            code_cells,
            data_cells,
        })
    }

    fn get_shard_account_for_emulation(
        &mut self,
        addr: &Addr,
        mc_block_seqno: Option<Seqno>,
    ) -> anyhow::Result<BocBytes> {
        self.get_shard_account_at_block(addr, mc_block_seqno)
    }

    fn emulation_context(&self, mc_block_seqno: Option<Seqno>) -> anyhow::Result<(Lt, u32, Seqno)> {
        if let Some(seqno) = mc_block_seqno {
            if seqno == 0 {
                return Ok((
                    self.globals.global_lt.saturating_add(self.globals.lt_step),
                    self.now_unix()?,
                    self.globals.head_seqno,
                ));
            }

            let block = self
                .get_block_header(seqno)
                .ok_or(LocalnetError::BlockNotFound { seqno })?;
            return Ok((
                block.end_lt.saturating_add(self.globals.lt_step),
                block.gen_utime,
                seqno,
            ));
        }

        Ok((
            self.globals.global_lt.saturating_add(self.globals.lt_step),
            self.now_unix()?,
            self.globals.head_seqno,
        ))
    }

    pub fn now_unix(&self) -> anyhow::Result<u32> {
        unix_now_with_offset(self.time_offset_seconds)
    }

    pub fn clock_info(&self) -> anyhow::Result<NodeClockInfo> {
        Ok(NodeClockInfo {
            current_unix_time: self.now_unix()?,
            time_offset_seconds: self.time_offset_seconds,
            next_block_timestamp: self.next_block_timestamp,
        })
    }

    pub fn increase_time(&mut self, seconds: u64) -> anyhow::Result<NodeClockInfo> {
        anyhow::ensure!(seconds > 0, "seconds must be greater than 0");
        let current = u64::from(self.now_unix()?);
        let next = current
            .checked_add(seconds)
            .context("localnet time overflow")?;
        anyhow::ensure!(
            next <= u64::from(u32::MAX),
            "localnet time cannot exceed {}",
            u32::MAX
        );
        let seconds = i64::try_from(seconds).context("localnet time delta is too large")?;
        self.time_offset_seconds = self
            .time_offset_seconds
            .checked_add(seconds)
            .context("localnet time offset overflow")?;
        self.clock_info()
    }

    pub fn set_time(&mut self, timestamp: u32) -> anyhow::Result<NodeClockInfo> {
        self.ensure_timestamp_not_before_latest_block(timestamp)?;
        self.time_offset_seconds = i64::from(timestamp) - system_unix_now_i64()?;
        self.clock_info()
    }

    pub fn set_next_block_timestamp(&mut self, timestamp: u32) -> anyhow::Result<NodeClockInfo> {
        self.ensure_timestamp_not_before_latest_block(timestamp)?;
        self.next_block_timestamp = Some(timestamp);
        self.clock_info()
    }

    fn next_block_gen_utime(&mut self) -> anyhow::Result<u32> {
        if let Some(timestamp) = self.next_block_timestamp {
            self.ensure_timestamp_not_before_latest_block(timestamp)?;
            self.next_block_timestamp = None;
            self.bump_offset_to_at_least(timestamp)?;
            return Ok(timestamp);
        }

        self.now_unix()
    }

    fn ensure_timestamp_not_before_latest_block(&self, timestamp: u32) -> anyhow::Result<()> {
        let latest = self.latest_block_timestamp();
        anyhow::ensure!(
            timestamp >= latest,
            "timestamp {timestamp} is before latest block timestamp {latest}"
        );
        Ok(())
    }

    fn latest_block_timestamp(&self) -> u32 {
        self.history
            .blocks
            .last()
            .map_or(0, |block| block.gen_utime)
    }

    pub(crate) fn bump_offset_to_at_least(&mut self, timestamp: u32) -> anyhow::Result<()> {
        let required = i64::from(timestamp) - system_unix_now_i64()?;
        if self.time_offset_seconds < required {
            self.time_offset_seconds = required;
        }
        Ok(())
    }

    fn empty_shard_account_boc() -> anyhow::Result<BocBytes> {
        Self::shard_account_boc(OptionalAccount(None), HashBytes::ZERO, 0)
    }

    fn account_shard_account_boc(
        addr: &Addr,
        state: AccountState,
        balance: u128,
    ) -> anyhow::Result<BocBytes> {
        let account = Account {
            address: IntAddr::Std(StdAddr::new(addr.workchain as i8, HashBytes(addr.addr))),
            storage_stat: Default::default(),
            last_trans_lt: 0,
            balance: CurrencyCollection::new(balance),
            state,
        };
        Self::shard_account_boc(OptionalAccount(Some(account)), HashBytes::ZERO, 0)
    }

    fn frozen_account_from_shard_account(
        addr: &Addr,
        shard_account: &ShardAccount,
    ) -> anyhow::Result<Account> {
        let optional_account = shard_account
            .account
            .load()
            .context("Failed to load current account state")?;
        let mut account = optional_account
            .0
            .context("Cannot freeze non-existing account from current state")?;

        let state_hash = match account.state.clone() {
            AccountState::Active(state_init) => {
                let state_cell = CellBuilder::build_from(state_init)
                    .context("Failed to serialize current StateInit")?;
                HashBytes(*state_cell.repr_hash().as_array())
            }
            AccountState::Uninit => {
                anyhow::bail!("Cannot freeze uninitialized account from current state")
            }
            AccountState::Frozen(_) => anyhow::bail!("Account is already frozen"),
        };

        account.address = IntAddr::Std(StdAddr::new(addr.workchain as i8, HashBytes(addr.addr)));
        account.state = AccountState::Frozen(state_hash);
        Ok(account)
    }

    fn shard_account_boc(
        optional_account: OptionalAccount,
        last_trans_hash: HashBytes,
        last_trans_lt: u64,
    ) -> anyhow::Result<BocBytes> {
        let sa = ShardAccount {
            account: Lazy::new(&optional_account)?,
            last_trans_hash,
            last_trans_lt,
        };
        let mut builder = CellBuilder::new();
        sa.store_into(&mut builder, Cell::empty_context())?;
        let cell = builder.build()?;
        Ok(Boc::encode(cell).into())
    }

    fn fetch_remote_shard_account(
        &mut self,
        addr: &Addr,
        provider: &RemoteProvider,
    ) -> anyhow::Result<Option<BocBytes>> {
        let (boc, meta) = fetch_remote_shard_account(addr, provider, &mut self.cas)?;
        if meta.status == AccountStatus::Nonexist {
            return Ok(None);
        }
        let lt = meta.last_trans_lt.unwrap_or(0);
        self.latest.accounts.insert(*addr, meta);
        self.latest_shard_state = None;
        self.update_public_libraries_from_account_diff(addr, None, Some(&boc), lt)?;
        self.register_account_code_libraries(addr, Some(provider), &boc, lt)?;
        Ok(Some(boc))
    }

    #[must_use]
    pub fn has_pending_messages(&self) -> bool {
        !self.pool.external.is_empty()
            || !self.pool.internal.is_empty()
            || !self.pending_freeze_current.is_empty()
    }

    pub fn faucet(&mut self, addr: &Addr, amount: u128) -> anyhow::Result<Hash256> {
        let mut giver_meta = self
            .latest
            .accounts
            .get(&GIVER_ADDR)
            .cloned()
            .context("Giver account not found")?;
        let giver_balance = giver_meta.balance;
        if giver_balance < amount {
            anyhow::bail!("Giver has insufficient balance");
        }

        let message_info = IntMsgInfo {
            ihr_disabled: true,
            bounce: false,
            bounced: false,
            src: GIVER_ADDR.into(),
            dst: addr.into(),
            ihr_fee: Default::default(),
            value: CurrencyCollection::new(amount),
            fwd_fee: Default::default(),
            created_at: 0,
            created_lt: 0,
        };

        let message = OwnedMessage {
            info: MsgInfo::Int(message_info),
            init: None,
            body: Default::default(),
            layout: None,
        };

        // Decrease giver balance before injecting the internal message. The local faucet
        // models a single destination transaction, so the source account is adjusted here.
        giver_meta.balance = giver_balance - amount;
        self.latest.accounts.insert(GIVER_ADDR, giver_meta);
        self.latest_shard_state = None;

        self.send_internal_boc(BocRepr::encode(message)?.into())
    }
}

fn store_account_state_cell_from_shard_account_boc(
    cas: &mut CellStore,
    shard_account_boc: &BocBytes,
) -> Option<Hash256> {
    let cell = Boc::decode(shard_account_boc).ok()?;
    let shard_account = cell.parse::<ShardAccount>().ok()?;
    let account_cell = shard_account.account.inner().clone();
    let hash = Hash256::from(account_cell.repr_hash());
    cas.put(Boc::encode(account_cell).into(), hash);
    Some(hash)
}

fn account_state_snapshot_from_boc(boc: &BocBytes) -> Option<AccountStateSnapshot> {
    let cell = Boc::decode(boc).ok()?;
    account_state_snapshot_from_cell(&cell)
}

fn account_state_snapshot_from_cell(cell: &Cell) -> Option<AccountStateSnapshot> {
    if let Ok(shard_account) = cell.parse::<ShardAccount>() {
        let hash = Hash256::from(shard_account.account.inner().repr_hash());
        let optional_account = shard_account.account.load().ok()?;
        return Some(account_state_snapshot_from_optional_account(
            hash,
            optional_account,
        ));
    }

    let hash = Hash256::from(cell.repr_hash());
    let optional_account = cell.parse::<OptionalAccount>().ok()?;
    Some(account_state_snapshot_from_optional_account(
        hash,
        optional_account,
    ))
}

fn account_state_snapshot_from_account_state_cell(cell: &Cell) -> Option<AccountStateSnapshot> {
    let hash = Hash256::from(cell.repr_hash());
    let optional_account = cell.parse::<OptionalAccount>().ok()?;
    Some(account_state_snapshot_from_optional_account(
        hash,
        optional_account,
    ))
}

fn account_state_snapshot_from_optional_account(
    hash: Hash256,
    optional_account: OptionalAccount,
) -> AccountStateSnapshot {
    let Some(account) = optional_account.0 else {
        return AccountStateSnapshot {
            hash,
            balance: 0,
            status: AccountStatus::Nonexist,
            code: None,
            data: None,
            frozen_hash: None,
        };
    };

    let mut code = None;
    let mut data = None;
    let mut frozen_hash = None;
    let status = match account.state {
        AccountState::Uninit => AccountStatus::Uninit,
        AccountState::Active(state) => {
            code = state.code;
            data = state.data;
            AccountStatus::Active
        }
        AccountState::Frozen(state) => {
            frozen_hash = Some(Hash256(state.0));
            AccountStatus::Frozen
        }
    };

    AccountStateSnapshot {
        hash,
        balance: account.balance.tokens.into(),
        status,
        code,
        data,
        frozen_hash,
    }
}

const fn tycho_account_status(status: AccountStatus) -> tycho_types::models::AccountStatus {
    match status {
        AccountStatus::Active => tycho_types::models::AccountStatus::Active,
        AccountStatus::Uninit => tycho_types::models::AccountStatus::Uninit,
        AccountStatus::Frozen => tycho_types::models::AccountStatus::Frozen,
        AccountStatus::Nonexist => tycho_types::models::AccountStatus::NotExists,
    }
}

fn collect_code_data_cells(
    shard_account_boc: Option<&BocBytes>,
    code_cells: &mut HashMap<Hash256, BocBytes>,
    data_cells: &mut HashMap<Hash256, BocBytes>,
) {
    let Some(shard_account_boc) = shard_account_boc else {
        return;
    };

    let Ok(cell) = Boc::decode(shard_account_boc) else {
        return;
    };
    let Ok(shard_account) = cell.parse::<ShardAccount>() else {
        return;
    };
    let Ok(optional_account) = shard_account.account.load() else {
        return;
    };
    let Some(account) = optional_account.0 else {
        return;
    };
    let AccountState::Active(state) = account.state else {
        return;
    };

    if let Some(code) = state.code {
        let hash = Hash256::from(code.repr_hash());
        code_cells
            .entry(hash)
            .or_insert_with(|| Boc::encode(code).into());
    }

    if let Some(data) = state.data {
        let hash = Hash256::from(data.repr_hash());
        data_cells
            .entry(hash)
            .or_insert_with(|| Boc::encode(data).into());
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MessageKind {
    Internal,
    ExternalIn,
    ExternalOut,
}

fn parse_msg_meta(boc: &[u8], hash: Hash256) -> anyhow::Result<MsgMeta> {
    Ok(parse_msg_meta_with_kind(boc, hash)?.0)
}

fn parse_msg_meta_with_kind(boc: &[u8], hash: Hash256) -> anyhow::Result<(MsgMeta, MessageKind)> {
    let cell = Boc::decode(boc)?;
    parse_msg_meta_with_kind_from_cell(&cell, hash)
}

fn parse_msg_meta_from_cell(cell: &Cell, hash: Hash256) -> anyhow::Result<MsgMeta> {
    Ok(parse_msg_meta_with_kind_from_cell(cell, hash)?.0)
}

fn initial_time_offset_for_blocks(blocks: &[BlockMeta]) -> anyhow::Result<i64> {
    let Some(latest_block) = blocks.last() else {
        return Ok(0);
    };
    let required = i64::from(latest_block.gen_utime) - system_unix_now_i64()?;
    Ok(required.max(0))
}

fn unix_now_with_offset(offset_seconds: i64) -> anyhow::Result<u32> {
    let now = system_unix_now_i64()?
        .checked_add(offset_seconds)
        .context("localnet time offset overflow")?;
    anyhow::ensure!(now >= 0, "localnet time cannot be before unix epoch");
    u32::try_from(now).context("localnet time cannot exceed u32::MAX")
}

fn system_unix_now_i64() -> anyhow::Result<i64> {
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    i64::try_from(seconds).context("system unix time is too large")
}

fn parse_msg_meta_with_kind_from_cell(
    cell: &Cell,
    hash: Hash256,
) -> anyhow::Result<(MsgMeta, MessageKind)> {
    let msg = cell.parse::<Message<'_>>()?;

    let (kind, src, dst, value, bounce, created_lt, created_at) = match msg.info {
        MsgInfo::Int(info) => (
            MessageKind::Internal,
            Some((&info.src).into()),
            Some((&info.dst).into()),
            Some(info.value.tokens.into()),
            Some(info.bounce),
            Some(info.created_lt),
            Some(info.created_at),
        ),
        MsgInfo::ExtIn(info) => (
            MessageKind::ExternalIn,
            None,
            Some((&info.dst).into()),
            None,
            None,
            None,
            None,
        ),
        MsgInfo::ExtOut(info) => (
            MessageKind::ExternalOut,
            Some((&info.src).into()),
            None,
            None,
            None,
            Some(info.created_lt),
            Some(info.created_at),
        ),
    };

    Ok((
        MsgMeta {
            msg_hash: hash,
            msg_boc_hash: hash,
            src,
            dst,
            value,
            bounce,
            created_lt,
            created_at,
        },
        kind,
    ))
}

fn library_ref_hash(cell: &Cell) -> anyhow::Result<Option<Hash256>> {
    const EXOTIC_LIBRARY_TAG: u8 = 2;
    if !cell.is_exotic() {
        return Ok(None);
    }

    let slice = cell.as_slice_allow_exotic();
    if slice.size_bits() != 8 + 256 {
        return Ok(None);
    }

    let mut slice = cell.as_slice_allow_exotic();
    if slice.load_u8()? != EXOTIC_LIBRARY_TAG {
        return Ok(None);
    }
    Ok(Some(Hash256(slice.load_u256()?.0)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::{ExecContext, ExecResult, TvmExecutor};
    use crate::node::StateSource;
    use serde_json::json;
    use std::sync::{Arc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};
    use ton_executor::DEFAULT_CONFIG;
    use tycho_types::cell::{Cell, CellBuilder, Lazy, Store};
    use tycho_types::dict::Dict;
    use tycho_types::models::block::Block;
    use tycho_types::models::transaction::{
        ComputePhase, ComputePhaseSkipReason, HashUpdate, OrdinaryTxInfo, SkippedComputePhase,
        Transaction, TxInfo,
    };
    use tycho_types::models::{
        Account, CurrencyCollection, IntAddr, OptionalAccount, SimpleLib, StateInit, StdAddr,
        StdAddrFormat,
    };

    struct NoopExecutor;

    impl TvmExecutor for NoopExecutor {
        fn execute(
            &self,
            _shard_account: &BocBytes,
            _in_msg: &BocBytes,
            _ctx: &ExecContext,
            _config: &BocBytes,
            _libs: Option<&BocBytes>,
        ) -> anyhow::Result<ExecResult> {
            anyhow::bail!("NoopExecutor should not be used in this test")
        }
    }

    struct SingleTxExecutor;

    impl TvmExecutor for SingleTxExecutor {
        fn execute(
            &self,
            shard_account: &BocBytes,
            in_msg: &BocBytes,
            ctx: &ExecContext,
            _config: &BocBytes,
            _libs: Option<&BocBytes>,
        ) -> anyhow::Result<ExecResult> {
            let in_msg_cell = Boc::decode(in_msg)?;
            let in_msg_owned = in_msg_cell.parse::<OwnedMessage>()?;
            let dst = match &in_msg_owned.info {
                MsgInfo::Int(info) => Addr::from(&info.dst),
                MsgInfo::ExtIn(info) => Addr::from(&info.dst),
                MsgInfo::ExtOut(_) => anyhow::bail!("test executor does not accept ext-out"),
            };

            let old_shard_account = Boc::decode(shard_account)?.parse::<ShardAccount>()?;
            let old_account_hash = *old_shard_account.account.inner().repr_hash();
            let new_account_boc =
                make_active_shard_account_boc_with_state(dst, None, None, Dict::new(), 42_000);
            let new_account_cell = Boc::decode(&new_account_boc)?;
            let new_shard_account = new_account_cell.parse::<ShardAccount>()?;
            let new_account_hash = *new_shard_account.account.inner().repr_hash();

            let tx = Transaction {
                account: HashBytes(dst.addr),
                lt: ctx.lt,
                prev_trans_hash: HashBytes::ZERO,
                prev_trans_lt: 0,
                now: ctx.gen_utime,
                out_msg_count: tycho_types::num::Uint15::ZERO,
                orig_status: tycho_types::models::AccountStatus::NotExists,
                end_status: tycho_types::models::AccountStatus::Active,
                in_msg: Some(in_msg_cell),
                out_msgs: Dict::new(),
                total_fees: CurrencyCollection::ZERO,
                state_update: Lazy::new(&HashUpdate {
                    old: old_account_hash,
                    new: new_account_hash,
                })?,
                info: Lazy::new(&TxInfo::Ordinary(OrdinaryTxInfo {
                    credit_first: false,
                    storage_phase: None,
                    credit_phase: None,
                    compute_phase: ComputePhase::Skipped(SkippedComputePhase {
                        reason: ComputePhaseSkipReason::NoState,
                    }),
                    action_phase: None,
                    aborted: true,
                    bounce_phase: None,
                    destroyed: false,
                }))?,
            };

            Ok(ExecResult {
                tx: tx.clone(),
                tx_boc: BocRepr::encode(tx)?.into(),
                new_account_boc,
                out_msg_cells: Vec::new(),
            })
        }
    }

    #[derive(Clone)]
    struct RecordingExecutor {
        recorded_libs: Arc<Mutex<Vec<Option<BocBytes>>>>,
        recorded_prev_blocks_info: Arc<Mutex<Vec<PrevBlocksInfo>>>,
    }

    impl TvmExecutor for RecordingExecutor {
        fn execute(
            &self,
            _shard_account: &BocBytes,
            _in_msg: &BocBytes,
            ctx: &ExecContext,
            _config: &BocBytes,
            libs: Option<&BocBytes>,
        ) -> anyhow::Result<ExecResult> {
            self.recorded_libs
                .lock()
                .expect("recorded libs mutex poisoned")
                .push(libs.cloned());
            self.recorded_prev_blocks_info
                .lock()
                .expect("recorded prev blocks info mutex poisoned")
                .push(ctx.prev_blocks_info.clone());
            anyhow::bail!("forced executor failure")
        }
    }

    fn make_test_node(executor: Box<dyn TvmExecutor>) -> Node {
        let config_boc = BocBytes::from_base64(DEFAULT_CONFIG).expect("must decode default config");
        Node::new(executor, config_boc, StateSource::Local).expect("must create test node")
    }

    fn block_meta(seqno: Seqno) -> BlockMeta {
        BlockMeta {
            seqno,
            prev_seqno: (seqno > 1).then_some(seqno - 1),
            gen_utime: seqno,
            start_lt: u64::from(seqno),
            end_lt: u64::from(seqno),
            tx_hashes: Vec::new(),
            block_hash: Hash256([seqno as u8; 32]),
            file_hash: Hash256([seqno as u8; 32]),
        }
    }

    #[test]
    fn with_db_path_creates_missing_parent_directories() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be after unix epoch")
            .as_nanos();
        let temp_root = std::env::temp_dir().join(format!(
            "ton-localnet-db-parent-test-{}-{unique}",
            std::process::id()
        ));
        let db_path = temp_root.join("build/data/localnet.db");

        let config_boc = BocBytes::from_base64(DEFAULT_CONFIG).expect("must decode default config");
        let node = Node::with_db_path(
            Box::new(NoopExecutor),
            config_boc,
            StateSource::Local,
            Some(&db_path),
        )
        .expect("must create test node with nested db path");

        assert!(db_path.exists(), "db file must be created");
        assert!(
            db_path.parent().is_some_and(std::path::Path::exists),
            "db parent directories must be created"
        );
        assert!(node.conn.is_some(), "sqlite connection must be initialized");

        drop(node);
        let _ = std::fs::remove_dir_all(temp_root);
    }

    #[test]
    fn compiler_abi_registry_persists_across_db_reopen() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be after unix epoch")
            .as_nanos();
        let temp_root = std::env::temp_dir().join(format!(
            "ton-localnet-compiler-abi-test-{}-{unique}",
            std::process::id()
        ));
        let db_path = temp_root.join("localnet.db");

        let config_boc = BocBytes::from_base64(DEFAULT_CONFIG).expect("must decode default config");
        let mut node = Node::with_db_path(
            Box::new(NoopExecutor),
            config_boc.clone(),
            StateSource::Local,
            Some(&db_path),
        )
        .expect("must create sqlite-backed test node");

        let code_hash = Hash256([0x42; 32]);
        let compiler_abi = json!({
            "compiler_name": "tolk",
            "contract_name": "Counter",
            "get_methods": [
                { "name": "currentCounter" }
            ]
        });

        node.history
            .set_compiler_abi(code_hash, compiler_abi.clone())
            .expect("must persist compiler ABI");
        drop(node);

        let reopened = Node::with_db_path(
            Box::new(NoopExecutor),
            config_boc,
            StateSource::Local,
            Some(&db_path),
        )
        .expect("must reopen sqlite-backed test node");

        assert_eq!(
            reopened.history.get_compiler_abi(&code_hash),
            Some(compiler_abi),
            "compiler ABI registry must survive node restart"
        );

        drop(reopened);
        let _ = std::fs::remove_dir_all(temp_root);
    }

    #[test]
    fn verified_source_registry_persists_across_db_reopen() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be after unix epoch")
            .as_nanos();
        let temp_root = std::path::PathBuf::from("/tmp").join(format!(
            "ton-localnet-verified-source-test-{}-{unique}",
            std::process::id()
        ));
        let db_path = temp_root.join("localnet.db");

        let config_boc = BocBytes::from_base64(DEFAULT_CONFIG).expect("must decode default config");
        let mut node = Node::with_db_path(
            Box::new(NoopExecutor),
            config_boc.clone(),
            StateSource::Local,
            Some(&db_path),
        )
        .expect("must create sqlite-backed test node");

        let code_hash = Hash256([0x24; 32]);
        let source = json!({
            "code_hash": code_hash.to_hex(),
            "verified": true,
            "bundles": [
                {
                    "source_bundle_hash": "source-bundle",
                    "verified_at": 0,
                    "storage_revision": "local",
                    "entrypoint": "contracts/main.tolk",
                    "compiler": {
                        "language": "tolk",
                        "version": "1.4.0",
                        "params": {}
                    },
                    "files": []
                }
            ]
        });

        node.history
            .set_verified_source(code_hash, source.clone())
            .expect("must persist verified source");
        drop(node);

        let reopened = Node::with_db_path(
            Box::new(NoopExecutor),
            config_boc,
            StateSource::Local,
            Some(&db_path),
        )
        .expect("must reopen sqlite-backed test node");

        assert_eq!(
            reopened.history.get_verified_source(&code_hash),
            Some(source),
            "verified source registry must survive node restart"
        );

        drop(reopened);
        let _ = std::fs::remove_dir_all(temp_root);
    }

    #[test]
    fn db_reopen_restores_block_messages_and_msg_to_tx_index() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be after unix epoch")
            .as_nanos();
        let temp_root = std::path::PathBuf::from("/tmp").join(format!(
            "ton-localnet-history-reopen-test-{}-{unique}",
            std::process::id()
        ));
        let db_path = temp_root.join("localnet.db");

        let config_boc = BocBytes::from_base64(DEFAULT_CONFIG).expect("must decode default config");
        let mut node = Node::with_db_path(
            Box::new(NoopExecutor),
            config_boc.clone(),
            StateSource::Local,
            Some(&db_path),
        )
        .expect("must create sqlite-backed test node");

        let account = test_addr(0x42);
        let tx_hash = Hash256([0x10; 32]);
        let in_msg_hash = Hash256([0x11; 32]);
        let out_msg_hash = Hash256([0x12; 32]);
        let block_hash = Hash256([0x13; 32]);
        let dummy_boc = BocBytes::from(Boc::encode(Cell::default()));
        node.cas.put(dummy_boc.clone(), in_msg_hash);
        node.cas.put(dummy_boc, out_msg_hash);
        node.history.msg_by_hash.insert(
            in_msg_hash,
            MsgMeta {
                msg_hash: in_msg_hash,
                msg_boc_hash: in_msg_hash,
                src: None,
                dst: Some(account),
                value: None,
                bounce: None,
                created_lt: None,
                created_at: None,
            },
        );
        node.history.msg_by_hash.insert(
            out_msg_hash,
            MsgMeta {
                msg_hash: out_msg_hash,
                msg_boc_hash: out_msg_hash,
                src: Some(account),
                dst: Some(test_addr(0x43)),
                value: Some(1),
                bounce: Some(false),
                created_lt: Some(2),
                created_at: Some(3),
            },
        );

        let tx_meta = TxMeta {
            tx_hash,
            account,
            lt: 1,
            now: 3,
            success: true,
            compute_exit_code: Some(0),
            action_result_code: Some(0),
            total_fees: 0,
            storage_fees: 0,
            other_fees: 0,
            in_msg_hash: Some(in_msg_hash),
            out_msg_hashes: vec![out_msg_hash],
            block_seqno: 1,
        };
        let block_meta = BlockMeta {
            seqno: 1,
            prev_seqno: None,
            gen_utime: 3,
            start_lt: 1,
            end_lt: 1,
            tx_hashes: vec![tx_hash],
            block_hash,
            file_hash: block_hash,
        };
        node.apply_commit(PendingCommit {
            block_meta,
            masterchain_block_meta: None,
            tx_metas: vec![tx_meta.clone()],
            deltas: Vec::new(),
            out_msg_hashes: vec![out_msg_hash],
            msg_to_tx: vec![(in_msg_hash, tx_hash)],
            deferred_msg_hashes: Vec::new(),
        })
        .expect("commit must persist");
        drop(node);

        let reopened = Node::with_db_path(
            Box::new(NoopExecutor),
            config_boc,
            StateSource::Local,
            Some(&db_path),
        )
        .expect("must reopen sqlite-backed test node");
        let reopened_block = reopened
            .get_block_header(1)
            .expect("persisted block must be loaded");

        assert_eq!(reopened_block.tx_hashes, vec![tx_hash]);
        let txs = reopened
            .get_block_transactions(&reopened_block)
            .expect("persisted block transactions must resolve");
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0].tx_hash, tx_meta.tx_hash);
        assert_eq!(txs[0].in_msg_hash, tx_meta.in_msg_hash);
        assert_eq!(txs[0].out_msg_hashes, tx_meta.out_msg_hashes);
        assert!(reopened.get_message_info(&in_msg_hash).is_some());
        assert!(reopened.get_message_info(&out_msg_hash).is_some());
        assert_eq!(reopened.history.msg_to_tx.get(&in_msg_hash), Some(&tx_hash));
        assert_eq!(
            reopened.indexes.tx_by_out_msg.get(&out_msg_hash),
            Some(&tx_hash)
        );

        drop(reopened);
        let _ = std::fs::remove_dir_all(temp_root);
    }

    #[test]
    fn snapshot_load_rebuilds_historical_account_and_out_message_indexes() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be after unix epoch")
            .as_nanos();
        let temp_root = std::path::PathBuf::from("/tmp").join(format!(
            "ton-localnet-snapshot-indexes-test-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp_root).expect("must create temp dir");
        let snapshot_path = temp_root.join("state.json");

        let account = test_addr(0x44);
        let account_meta = AccountMeta {
            account_hash: Hash256([0x41; 32]),
            status: AccountStatus::Active,
            balance: 7,
            last_trans_lt: Some(10),
            last_trans_hash: Some(Hash256([0x42; 32])),
            code_hash: None,
            data_hash: None,
            frozen_hash: None,
        };
        let tx_hash = Hash256([0x10; 32]);
        let out_msg_hash = Hash256([0x11; 32]);
        let mut node = make_test_node(Box::new(NoopExecutor));
        node.globals.head_seqno = 2;
        node.history.blocks.push(BlockMeta {
            seqno: 1,
            prev_seqno: None,
            gen_utime: 3,
            start_lt: 10,
            end_lt: 10,
            tx_hashes: vec![tx_hash],
            block_hash: Hash256([0x12; 32]),
            file_hash: Hash256([0x12; 32]),
        });
        node.history.deltas_by_seqno = vec![vec![AccountDelta {
            addr: account,
            old_hash: None,
            new_hash: Some(account_meta.account_hash),
            old_meta: None,
            new_meta: Some(account_meta.clone()),
        }]];
        node.history.tx_by_hash.insert(
            tx_hash,
            TxMeta {
                tx_hash,
                account,
                lt: 10,
                now: 3,
                success: true,
                compute_exit_code: Some(0),
                action_result_code: Some(0),
                total_fees: 0,
                storage_fees: 0,
                other_fees: 0,
                in_msg_hash: None,
                out_msg_hashes: vec![out_msg_hash],
                block_seqno: 1,
            },
        );
        node.dump_state_to_path(&snapshot_path)
            .expect("snapshot must dump");

        let mut loaded = make_test_node(Box::new(NoopExecutor));
        loaded
            .load_state_from_path(&snapshot_path)
            .expect("snapshot must load");

        assert_eq!(
            loaded
                .get_address_information_at_block(&account, 1)
                .map(|meta| meta.balance),
            Some(account_meta.balance)
        );
        assert_eq!(
            loaded.indexes.tx_by_out_msg.get(&out_msg_hash),
            Some(&tx_hash)
        );
        assert_eq!(loaded.indexes.tx_by_block.get(&1), Some(&vec![tx_hash]));

        let _ = std::fs::remove_dir_all(temp_root);
    }

    fn test_addr(byte: u8) -> Addr {
        Addr {
            workchain: 0,
            addr: [byte; 32],
        }
    }

    fn parse_test_addr(address: &str) -> Addr {
        let (std_addr, _) =
            StdAddr::from_str_ext(address, StdAddrFormat::any()).expect("test address must parse");
        Addr {
            workchain: i32::from(std_addr.workchain),
            addr: std_addr.address.0,
        }
    }

    #[test]
    fn jetton_content_merge_keeps_onchain_values_and_adds_offchain_metadata() {
        let mut content = json!({
            "uri": "https://example.test/jetton.json",
            "decimals": "6",
            "symbol": "LOCAL"
        });
        let remote_content = json!({
            "name": "Tether USD",
            "description": "Tether Token for Tether USD",
            "image": "https://tether.to/images/logoCircle.png",
            "symbol": "USDt",
            "decimals": 9
        });

        ton_indexer::jettons::merge_jetton_content(&mut content, &remote_content);

        assert_eq!(content["uri"], "https://example.test/jetton.json");
        assert_eq!(content["name"], "Tether USD");
        assert_eq!(content["description"], "Tether Token for Tether USD");
        assert_eq!(content["image"], "https://tether.to/images/logoCircle.png");
        assert_eq!(content["symbol"], "LOCAL");
        assert_eq!(content["decimals"], "6");
    }

    fn store_test_account_meta(
        node: &mut Node,
        boc: &BocBytes,
        status: AccountStatus,
    ) -> AccountMeta {
        let account_hash = boc.hash().expect("must hash shard account");
        node.cas.put(boc.clone(), account_hash);
        let cached_balance = if status == AccountStatus::Nonexist {
            0
        } else {
            1_000_000_000
        };
        AccountMeta {
            account_hash,
            status,
            balance: cached_balance,
            last_trans_lt: Some(0),
            last_trans_hash: None,
            code_hash: None,
            data_hash: None,
            frozen_hash: None,
        }
    }

    fn shard_account_state_name(boc: &BocBytes) -> &'static str {
        let cell = Boc::decode(boc).expect("shard account boc must decode");
        let shard_account = cell
            .parse::<ShardAccount>()
            .expect("shard account boc must parse");
        let optional_account = shard_account
            .account
            .load()
            .expect("shard account lazy account must load");
        let Some(account) = optional_account.0 else {
            return "nonexist";
        };

        match account.state {
            AccountState::Active(_) => "active",
            AccountState::Uninit => "uninit",
            AccountState::Frozen(_) => "frozen",
        }
    }

    fn make_lib_root(seed: u32) -> Cell {
        let mut builder = CellBuilder::new();
        builder.store_u32(seed).expect("must store seed");
        builder.build().expect("must build test lib root")
    }

    fn valid_simple_lib_entry(public: bool, seed: u32) -> (HashBytes, SimpleLib) {
        let root = make_lib_root(seed);
        let hash = HashBytes(*root.repr_hash().as_array());
        (hash, SimpleLib { public, root })
    }

    fn make_active_shard_account_boc(
        addr: Addr,
        libraries: Dict<HashBytes, SimpleLib>,
    ) -> BocBytes {
        make_active_shard_account_boc_with_state(addr, None, None, libraries, 1_000_000_000)
    }

    fn make_active_shard_account_boc_with_state(
        addr: Addr,
        code: Option<Cell>,
        data: Option<Cell>,
        libraries: Dict<HashBytes, SimpleLib>,
        balance: u128,
    ) -> BocBytes {
        let state_init = StateInit {
            split_depth: None,
            special: None,
            code,
            data,
            libraries,
        };
        let account = Account {
            address: IntAddr::Std(StdAddr::new(addr.workchain as i8, HashBytes(addr.addr))),
            storage_stat: Default::default(),
            last_trans_lt: 0,
            balance: CurrencyCollection::new(balance),
            state: AccountState::Active(state_init),
        };
        shard_account_boc(OptionalAccount(Some(account)))
    }

    fn make_uninit_shard_account_boc(addr: Addr) -> BocBytes {
        let account = Account {
            address: IntAddr::Std(StdAddr::new(addr.workchain as i8, HashBytes(addr.addr))),
            storage_stat: Default::default(),
            last_trans_lt: 0,
            balance: CurrencyCollection::new(1_000_000_000),
            state: AccountState::Uninit,
        };
        shard_account_boc(OptionalAccount(Some(account)))
    }

    fn make_frozen_shard_account_boc(addr: Addr) -> BocBytes {
        let account = Account {
            address: IntAddr::Std(StdAddr::new(addr.workchain as i8, HashBytes(addr.addr))),
            storage_stat: Default::default(),
            last_trans_lt: 0,
            balance: CurrencyCollection::new(1_000_000_000),
            state: AccountState::Frozen(HashBytes([0xAA; 32])),
        };
        shard_account_boc(OptionalAccount(Some(account)))
    }

    fn make_nonexist_shard_account_boc() -> BocBytes {
        shard_account_boc(OptionalAccount(None))
    }

    fn shard_account_boc(optional_account: OptionalAccount) -> BocBytes {
        let shard_account = ShardAccount {
            account: Lazy::new(&optional_account).expect("must build lazy account"),
            last_trans_hash: HashBytes::ZERO,
            last_trans_lt: 0,
        };
        let mut builder = CellBuilder::new();
        shard_account
            .store_into(&mut builder, Cell::empty_context())
            .expect("must serialize shard account");
        let cell = builder.build().expect("must build shard account cell");
        Boc::encode(cell).into()
    }

    fn account_state_hash_from_shard_account_boc(boc: &BocBytes) -> Hash256 {
        let cell = Boc::decode(boc).expect("shard account BOC must decode");
        let shard_account = cell
            .parse::<ShardAccount>()
            .expect("shard account BOC must parse");
        Hash256::from(shard_account.account.inner().repr_hash())
    }

    #[test]
    fn account_state_snapshot_uses_account_state_hash_and_balance() {
        let account = test_addr(0x21);
        let code = make_lib_root(0xc0de);
        let data = make_lib_root(0xda7a);
        let boc = make_active_shard_account_boc_with_state(
            account,
            Some(code.clone()),
            Some(data.clone()),
            Dict::new(),
            12_345_678,
        );

        let snapshot = account_state_snapshot_from_boc(&boc).expect("snapshot must parse");

        assert_eq!(
            snapshot.hash,
            account_state_hash_from_shard_account_boc(&boc)
        );
        assert_eq!(snapshot.balance, 12_345_678);
        assert_eq!(snapshot.status, AccountStatus::Active);
        assert_eq!(snapshot.code_hash(), Some(Hash256::from(code.repr_hash())));
        assert_eq!(snapshot.data_hash(), Some(Hash256::from(data.repr_hash())));
        assert_eq!(snapshot.code.as_ref(), Some(&code));
        assert_eq!(snapshot.data.as_ref(), Some(&data));
    }

    #[test]
    fn account_state_snapshot_lookup_scans_shard_account_bocs_by_account_state_hash() {
        let mut node = make_test_node(Box::new(NoopExecutor));
        let account = test_addr(0x22);
        let boc =
            make_active_shard_account_boc_with_state(account, None, None, Dict::new(), 777_000);
        let shard_hash = boc.hash().expect("shard account BOC must hash");
        node.cas.put(boc.clone(), shard_hash);

        let snapshot = node
            .find_account_state_snapshot(&account_state_hash_from_shard_account_boc(&boc))
            .expect("snapshot must be found by account state hash");

        assert_eq!(snapshot.balance, 777_000);
        assert_eq!(snapshot.status, AccountStatus::Active);
    }

    #[test]
    fn account_state_snapshot_lookup_reads_stored_account_state_cell_directly() {
        let mut node = make_test_node(Box::new(NoopExecutor));
        let account = test_addr(0x23);
        let boc =
            make_active_shard_account_boc_with_state(account, None, None, Dict::new(), 888_000);
        let state_hash = store_account_state_cell_from_shard_account_boc(&mut node.cas, &boc)
            .expect("account state cell must be stored");

        let snapshot = node
            .find_account_state_snapshot(&state_hash)
            .expect("snapshot must be found by stored account state hash");

        assert_eq!(snapshot.hash, state_hash);
        assert_eq!(snapshot.balance, 888_000);
        assert_eq!(snapshot.status, AccountStatus::Active);
    }

    #[test]
    fn mine_block_creates_empty_block_without_pending_messages() {
        let mut node = make_test_node(Box::new(NoopExecutor));
        let assert_pruned_masterchain_block = |node: &Node, seqno| {
            let masterchain_block = node
                .get_masterchain_block_header(seqno)
                .expect("masterchain block must be stored");
            let masterchain_block_boc = node
                .get_masterchain_block_data(seqno)
                .expect("masterchain block BOC must be stored");
            let config_boc = node
                .get_cell(&node.globals.config_boc_hash)
                .expect("config BOC must be stored");
            assert!(
                masterchain_block_boc.len() < config_boc.len() / 4,
                "masterchain block BOC should not serialize the full config subtree"
            );
            let masterchain_state = node
                .get_masterchain_state_cell(seqno)
                .expect("masterchain state must be rebuildable");
            assert_eq!(
                Hash256::from(masterchain_state.repr_hash()),
                masterchain_block.state_root_hash
            );
            masterchain_block_boc.len()
        };
        let assert_pruned_shard_block = |node: &Node, seqno| {
            let block_boc = node
                .get_block_data(seqno)
                .expect("shard block BOC must be stored");
            let block_cell = Boc::decode(&block_boc).expect("shard block BOC must decode");
            let block = block_cell.parse::<Block>().expect("shard block must parse");
            let state_update = block
                .load_state_update()
                .expect("shard block state update must load");
            let shard_state = node
                .get_shard_state_cell(seqno)
                .expect("shard state must be rebuildable");

            assert_eq!(
                Hash256::from(shard_state.repr_hash()),
                Hash256::from(&state_update.new_hash)
            );
            block_boc.len()
        };

        let block = node.mine_block().expect("empty block must be mined");

        assert_eq!(block.seqno, 1);
        assert_eq!(block.prev_seqno, None);
        assert!(block.tx_hashes.is_empty());
        assert_eq!(block.start_lt, 0);
        assert_eq!(block.end_lt, 0);
        assert_eq!(node.globals.head_seqno, 1);
        assert_pruned_masterchain_block(&node, 1);
        assert_pruned_shard_block(&node, 1);
        assert_eq!(
            node.get_block_transactions(&block)
                .expect("empty block transactions must resolve")
                .len(),
            0
        );

        let second_block = node.mine_block().expect("second empty block must be mined");

        assert_eq!(second_block.seqno, 2);
        assert_eq!(second_block.prev_seqno, Some(1));
        assert!(second_block.tx_hashes.is_empty());
        assert_eq!(node.globals.head_seqno, 2);
        assert_pruned_masterchain_block(&node, 2);
        assert_pruned_shard_block(&node, 2);

        for expected_seqno in 3..=40 {
            let block = node.mine_block().expect("empty block must be mined");
            assert_eq!(block.seqno, expected_seqno);
            assert!(block.tx_hashes.is_empty());
        }

        let size_after_limit = assert_pruned_masterchain_block(&node, 20);
        let later_size = assert_pruned_masterchain_block(&node, 40);
        assert!(
            later_size <= size_after_limit + 2048,
            "masterchain block BOC should stay bounded after old_mc_blocks reaches its limit"
        );
        let shard_size_after_limit = assert_pruned_shard_block(&node, 20);
        let later_shard_size = assert_pruned_shard_block(&node, 40);
        assert!(
            later_shard_size <= shard_size_after_limit + 1024,
            "shard block BOC should stay bounded across empty blocks"
        );
    }

    #[test]
    fn mine_block_stores_parseable_block_with_account_transactions() {
        let account = test_addr(0x44);
        let mut node = make_test_node(Box::new(SingleTxExecutor));
        let message = OwnedMessage {
            info: MsgInfo::Int(IntMsgInfo {
                ihr_disabled: true,
                bounce: false,
                bounced: false,
                src: GIVER_ADDR.into(),
                dst: account.into(),
                ihr_fee: Default::default(),
                value: CurrencyCollection::new(1_000),
                fwd_fee: Default::default(),
                created_at: 0,
                created_lt: 0,
            }),
            init: None,
            body: Default::default(),
            layout: None,
        };
        node.send_internal_boc(
            BocRepr::encode(message)
                .expect("message must serialize")
                .into(),
        )
        .expect("message must be queued");

        let block = node.mine_block().expect("block must be mined");
        let block_boc = node
            .cas
            .get(&block.block_hash)
            .expect("block BoC must be stored in CAS");
        let block_cell = Boc::decode(&block_boc).expect("block BoC must decode");
        let parsed = block_cell
            .parse::<Block>()
            .expect("block must parse as TON block");

        assert_eq!(block.file_hash, crate::block::file_hash(&block_boc));
        assert_ne!(block.file_hash, block.block_hash);
        assert_eq!(parsed.load_info().expect("block info must load").seqno, 1);
        assert!(
            parsed
                .load_value_flow()
                .expect("value flow must load")
                .validate()
                .expect("value flow must validate")
        );

        let extra = parsed.load_extra().expect("block extra must load");
        let account_blocks = extra
            .account_blocks
            .load()
            .expect("account blocks must load");
        let (_, account_block) = account_blocks
            .get(HashBytes(account.addr))
            .expect("account block lookup must not fail")
            .expect("account block must exist");
        let (_, tx_ref) = account_block
            .transactions
            .get(block.start_lt)
            .expect("transaction lookup must not fail")
            .expect("transaction must exist in account block");
        let tx = tx_ref.load().expect("transaction ref must load");

        assert_eq!(tx.account, HashBytes(account.addr));
        assert_eq!(tx.lt, block.start_lt);
        assert_eq!(block.tx_hashes, vec![Hash256::from(tx_ref.repr_hash())]);
    }

    #[test]
    fn mine_block_defers_messages_after_lt_delta_hard_limit() {
        let account = test_addr(0x45);
        let mut node = make_test_node(Box::new(SingleTxExecutor));
        let limits = BlockLimits {
            bytes_hard_limit: usize::MAX,
            gas_hard_limit: u64::MAX,
            lt_delta_hard_limit: 2,
        };

        for value in 1..=3 {
            let message = OwnedMessage {
                info: MsgInfo::Int(IntMsgInfo {
                    ihr_disabled: true,
                    bounce: false,
                    bounced: false,
                    src: GIVER_ADDR.into(),
                    dst: account.into(),
                    ihr_fee: Default::default(),
                    value: CurrencyCollection::new(value),
                    fwd_fee: Default::default(),
                    created_at: 0,
                    created_lt: value as u64,
                }),
                init: None,
                body: Default::default(),
                layout: None,
            };
            node.send_internal_boc(
                BocRepr::encode(message)
                    .expect("message must serialize")
                    .into(),
            )
            .expect("message must be queued");
        }

        let first_block = node
            .mine_block_with_limits(limits)
            .expect("first block must be mined");

        assert_eq!(first_block.tx_hashes.len(), 2);
        assert_eq!(node.pool.internal.len(), 1);
        assert_eq!(node.globals.head_seqno, 1);

        let second_block = node
            .mine_block_with_limits(limits)
            .expect("second block must be mined");

        assert_eq!(second_block.tx_hashes.len(), 1);
        assert!(node.pool.internal.is_empty());
        assert_eq!(node.globals.head_seqno, 2);
    }

    #[test]
    fn prev_blocks_info_before_block_uses_existing_blocks_and_zero_anchor() {
        let mut node = make_test_node(Box::new(NoopExecutor));
        node.history.blocks.push(block_meta(1));
        node.history.blocks.push(block_meta(2));

        let info = node.prev_blocks_info_before_block(3);
        let seqnos = info
            .last_mc_blocks
            .iter()
            .map(|block| block.seqno)
            .collect::<Vec<_>>();
        let sparse_seqnos = info
            .last_mc_blocks_100
            .iter()
            .map(|block| block.seqno)
            .collect::<Vec<_>>();

        assert_eq!(seqnos, vec![2, 1, 0]);
        assert_eq!(info.prev_key_block.seqno, 2);
        assert_eq!(sparse_seqnos, vec![0]);
    }

    #[allow(clippy::significant_drop_tightening)]
    #[test]
    fn mined_transaction_receives_prev_blocks_info_for_previous_block() {
        let recorded_libs = Arc::new(Mutex::new(Vec::<Option<BocBytes>>::new()));
        let recorded_prev_blocks_info = Arc::new(Mutex::new(Vec::<PrevBlocksInfo>::new()));
        let executor = RecordingExecutor {
            recorded_libs: Arc::clone(&recorded_libs),
            recorded_prev_blocks_info: Arc::clone(&recorded_prev_blocks_info),
        };
        let mut node = make_test_node(Box::new(executor));
        node.mine_block().expect("block 1 must be mined");
        node.mine_block().expect("block 2 must be mined");

        node.faucet(&test_addr(0x68), 1)
            .expect("faucet message must be queued");
        node.mine_block()
            .expect("block with forced executor failure must still be mined");

        let calls = recorded_prev_blocks_info
            .lock()
            .expect("recorded prev blocks info mutex poisoned");
        assert!(!calls.is_empty(), "executor must be invoked");
        let seqnos = calls[0]
            .last_mc_blocks
            .iter()
            .map(|block| block.seqno)
            .collect::<Vec<_>>();

        assert_eq!(seqnos, vec![2, 1, 0]);
        assert_eq!(calls[0].prev_key_block.seqno, 2);
    }

    #[test]
    fn get_traces_uses_out_msg_index_to_find_parent_transaction() {
        let mut node = make_test_node(Box::new(NoopExecutor));
        let parent_account = test_addr(0x61);
        let child_account = test_addr(0x62);
        let external_msg_hash = Hash256([0x63; 32]);
        let internal_msg_hash = Hash256([0x64; 32]);
        let parent_tx_hash = Hash256([0x65; 32]);
        let child_tx_hash = Hash256([0x66; 32]);
        let block_hash = Hash256([0x67; 32]);
        let dummy_boc = BocBytes::from(Boc::encode(Cell::default()));

        for hash in [
            external_msg_hash,
            internal_msg_hash,
            parent_tx_hash,
            child_tx_hash,
        ] {
            node.cas.put(dummy_boc.clone(), hash);
        }
        node.history.msg_by_hash.insert(
            external_msg_hash,
            MsgMeta {
                msg_hash: external_msg_hash,
                msg_boc_hash: external_msg_hash,
                src: None,
                dst: Some(parent_account),
                value: None,
                bounce: None,
                created_lt: None,
                created_at: None,
            },
        );
        node.history.msg_by_hash.insert(
            internal_msg_hash,
            MsgMeta {
                msg_hash: internal_msg_hash,
                msg_boc_hash: internal_msg_hash,
                src: Some(parent_account),
                dst: Some(child_account),
                value: Some(1),
                bounce: Some(false),
                created_lt: Some(1),
                created_at: Some(2),
            },
        );

        let parent_tx = TxMeta {
            tx_hash: parent_tx_hash,
            account: parent_account,
            lt: 1,
            now: 2,
            success: true,
            compute_exit_code: Some(0),
            action_result_code: Some(0),
            total_fees: 0,
            storage_fees: 0,
            other_fees: 0,
            in_msg_hash: Some(external_msg_hash),
            out_msg_hashes: vec![internal_msg_hash],
            block_seqno: 1,
        };
        let child_tx = TxMeta {
            tx_hash: child_tx_hash,
            account: child_account,
            lt: 2,
            now: 2,
            success: true,
            compute_exit_code: Some(0),
            action_result_code: Some(0),
            total_fees: 0,
            storage_fees: 0,
            other_fees: 0,
            in_msg_hash: Some(internal_msg_hash),
            out_msg_hashes: Vec::new(),
            block_seqno: 1,
        };
        node.apply_commit(PendingCommit {
            block_meta: BlockMeta {
                seqno: 1,
                prev_seqno: None,
                gen_utime: 2,
                start_lt: 1,
                end_lt: 2,
                tx_hashes: vec![parent_tx_hash, child_tx_hash],
                block_hash,
                file_hash: block_hash,
            },
            masterchain_block_meta: None,
            tx_metas: vec![parent_tx, child_tx],
            deltas: Vec::new(),
            out_msg_hashes: vec![internal_msg_hash],
            msg_to_tx: vec![
                (external_msg_hash, parent_tx_hash),
                (internal_msg_hash, child_tx_hash),
            ],
            deferred_msg_hashes: Vec::new(),
        })
        .expect("commit must index trace");

        assert_eq!(
            node.indexes.tx_by_out_msg.get(&internal_msg_hash),
            Some(&parent_tx_hash)
        );
        let trace = node
            .get_traces(&child_tx_hash)
            .expect("child transaction trace must resolve to root");
        assert_eq!(trace.transaction.meta.tx_hash, parent_tx_hash);
        assert_eq!(trace.external_hash, Some(external_msg_hash));
        assert_eq!(trace.children.len(), 1);
        assert_eq!(trace.children[0].transaction.meta.tx_hash, child_tx_hash);
    }

    fn found_library_entry(node: &Node, hash: Hash256) -> Option<GlobalLibraryEntry> {
        let mut entries = node.get_libraries(&[hash]);
        assert_eq!(entries.len(), 1, "expected one lookup result");
        entries.remove(0)
    }

    #[test]
    fn get_shard_account_at_block_uses_latest_for_current_queries() {
        let mut node = make_test_node(Box::new(NoopExecutor));
        let account = test_addr(0x10);
        let boc = make_active_shard_account_boc(account, Dict::new());
        let meta = store_test_account_meta(&mut node, &boc, AccountStatus::Active);
        node.latest.accounts.insert(account, meta);
        node.globals.head_seqno = 3;

        for seqno in [None, Some(0), Some(3), Some(4)] {
            assert_eq!(
                node.get_shard_account_at_block(&account, seqno)
                    .expect("must get current shard account"),
                boc,
                "seqno {seqno:?} must resolve to latest account state"
            );
        }
    }

    #[test]
    fn get_shard_account_at_block_uses_historical_deltas() {
        let mut node = make_test_node(Box::new(NoopExecutor));
        let account = test_addr(0x20);
        let uninit_boc = make_uninit_shard_account_boc(account);
        let active_boc = make_active_shard_account_boc(account, Dict::new());
        let uninit_meta = store_test_account_meta(&mut node, &uninit_boc, AccountStatus::Uninit);
        let active_meta = store_test_account_meta(&mut node, &active_boc, AccountStatus::Active);

        node.latest.accounts.insert(account, active_meta.clone());
        node.history.deltas_by_seqno = vec![
            Vec::new(),
            vec![AccountDelta {
                addr: account,
                old_hash: None,
                new_hash: Some(uninit_meta.account_hash),
                old_meta: None,
                new_meta: Some(uninit_meta.clone()),
            }],
            vec![AccountDelta {
                addr: account,
                old_hash: Some(uninit_meta.account_hash),
                new_hash: Some(active_meta.account_hash),
                old_meta: Some(uninit_meta),
                new_meta: Some(active_meta),
            }],
        ];
        for (index, deltas) in node.history.deltas_by_seqno.iter().enumerate() {
            for delta in deltas {
                node.indexes
                    .account_deltas_by_addr
                    .entry(delta.addr)
                    .or_default()
                    .insert(index as Seqno + 1, delta.clone());
            }
        }
        node.globals.head_seqno = 3;

        let before_first_delta = node
            .get_shard_account_at_block(&account, Some(1))
            .expect("must return empty state before first account delta");
        assert_eq!(shard_account_state_name(&before_first_delta), "nonexist");
        assert_eq!(
            node.get_shard_account_at_block(&account, Some(2))
                .expect("must return historical uninit state"),
            uninit_boc
        );
        assert_eq!(
            node.get_shard_account_at_block(&account, Some(3))
                .expect("must return latest active state"),
            active_boc
        );
    }

    #[test]
    fn get_shard_account_at_block_uses_old_meta_before_first_delta() {
        let mut node = make_test_node(Box::new(NoopExecutor));
        let account = test_addr(0x24);
        let remote_boc =
            make_active_shard_account_boc_with_state(account, None, None, Dict::new(), 100_000_000);
        let changed_boc =
            make_active_shard_account_boc_with_state(account, None, None, Dict::new(), 90_000_000);
        let remote_meta = store_test_account_meta(&mut node, &remote_boc, AccountStatus::Active);
        let changed_meta = store_test_account_meta(&mut node, &changed_boc, AccountStatus::Active);

        node.latest.accounts.insert(account, changed_meta.clone());
        node.indexes
            .account_deltas_by_addr
            .entry(account)
            .or_default()
            .insert(
                3,
                AccountDelta {
                    addr: account,
                    old_hash: Some(remote_meta.account_hash),
                    new_hash: Some(changed_meta.account_hash),
                    old_meta: Some(remote_meta),
                    new_meta: Some(changed_meta),
                },
            );
        node.globals.head_seqno = 4;

        assert_eq!(
            node.get_shard_account_at_block(&account, Some(2))
                .expect("must return fork baseline state before first local delta"),
            remote_boc
        );
    }

    #[test]
    fn get_shard_account_at_block_uses_last_delta_in_same_block() {
        let mut node = make_test_node(Box::new(NoopExecutor));
        let account = test_addr(0x25);
        let uninit_boc = make_uninit_shard_account_boc(account);
        let active_boc = make_active_shard_account_boc(account, Dict::new());
        let uninit_meta = store_test_account_meta(&mut node, &uninit_boc, AccountStatus::Uninit);
        let active_meta = store_test_account_meta(&mut node, &active_boc, AccountStatus::Active);

        node.history.deltas_by_seqno = vec![vec![
            AccountDelta {
                addr: account,
                old_hash: None,
                new_hash: Some(uninit_meta.account_hash),
                old_meta: None,
                new_meta: Some(uninit_meta.clone()),
            },
            AccountDelta {
                addr: account,
                old_hash: Some(uninit_meta.account_hash),
                new_hash: Some(active_meta.account_hash),
                old_meta: Some(uninit_meta),
                new_meta: Some(active_meta),
            },
        ]];
        for (index, deltas) in node.history.deltas_by_seqno.iter().enumerate() {
            for delta in deltas {
                node.indexes
                    .account_deltas_by_addr
                    .entry(delta.addr)
                    .or_default()
                    .insert(index as Seqno + 1, delta.clone());
            }
        }
        node.globals.head_seqno = 2;

        assert_eq!(
            node.get_shard_account_at_block(&account, Some(1))
                .expect("must return final state from historical block"),
            active_boc
        );
    }

    #[test]
    fn get_shard_account_at_block_preserves_account_state_variants() {
        let cases = [
            (
                "active",
                test_addr(0x31),
                make_active_shard_account_boc(test_addr(0x31), Dict::new()),
                AccountStatus::Active,
            ),
            (
                "uninit",
                test_addr(0x32),
                make_uninit_shard_account_boc(test_addr(0x32)),
                AccountStatus::Uninit,
            ),
            (
                "frozen",
                test_addr(0x33),
                make_frozen_shard_account_boc(test_addr(0x33)),
                AccountStatus::Frozen,
            ),
            (
                "nonexist",
                test_addr(0x34),
                make_nonexist_shard_account_boc(),
                AccountStatus::Nonexist,
            ),
        ];

        for (expected_state, account, boc, status) in cases {
            let mut node = make_test_node(Box::new(NoopExecutor));
            let meta = store_test_account_meta(&mut node, &boc, status);
            node.latest.accounts.insert(account, meta);

            let actual = node
                .get_shard_account_at_block(&account, None)
                .expect("must return stored shard account");
            assert_eq!(actual, boc, "{expected_state} BOC must be preserved");
            assert_eq!(
                shard_account_state_name(&actual),
                expected_state,
                "{expected_state} shard account state must round-trip"
            );
        }
    }

    #[test]
    fn private_library_is_not_visible_in_global_index() {
        let mut node = make_test_node(Box::new(NoopExecutor));
        let account = test_addr(0x11);
        let old_boc = make_nonexist_shard_account_boc();

        let mut libs = Dict::<HashBytes, SimpleLib>::new();
        let (key, lib) = valid_simple_lib_entry(false, 1);
        libs.set(key, lib).expect("must insert private lib");
        let new_boc = make_active_shard_account_boc(account, libs);

        node.update_public_libraries_from_account_diff(&account, Some(&old_boc), Some(&new_boc), 1)
            .expect("must update library diff");

        assert!(
            node.global_libraries.is_empty(),
            "private libraries must not be added to global index"
        );
    }

    #[test]
    fn add_public_library_by_account_a_is_visible_globally() {
        let mut node = make_test_node(Box::new(NoopExecutor));
        let account_a = test_addr(0x22);
        let old_boc = make_nonexist_shard_account_boc();

        let mut libs = Dict::<HashBytes, SimpleLib>::new();
        let (hash, lib) = valid_simple_lib_entry(true, 2);
        libs.set(hash, lib).expect("must insert public lib");
        let new_boc = make_active_shard_account_boc(account_a, libs);

        node.update_public_libraries_from_account_diff(
            &account_a,
            Some(&old_boc),
            Some(&new_boc),
            2,
        )
        .expect("must update library diff");

        let entry = found_library_entry(&node, Hash256::from(&hash))
            .expect("public library must appear globally");
        assert!(
            entry.publishers.contains(&account_a),
            "publisher A must be tracked"
        );
    }

    #[test]
    fn account_code_library_reference_from_cache_is_visible_globally() {
        let mut node = make_test_node(Box::new(NoopExecutor));
        let account = test_addr(0x21);
        let library = make_lib_root(21);
        let hash = Hash256::from(library.repr_hash());
        node.cas.put(Boc::encode(library.clone()).into(), hash);

        let code_ref = CellBuilder::build_library(&HashBytes(hash.0));
        let account_boc =
            make_active_shard_account_boc_with_state(account, Some(code_ref), None, Dict::new(), 1);

        node.register_account_code_libraries(&account, None, &account_boc, 21)
            .expect("cached code library must be registered");

        let entry = found_library_entry(&node, hash).expect("code library must appear globally");
        assert_eq!(entry.lib_boc, Boc::encode(library).into());
        assert!(entry.publishers.contains(&account));
    }

    #[test]
    fn set_shard_account_registers_cached_code_reference_libraries() {
        let mut node = make_test_node(Box::new(NoopExecutor));
        let account = test_addr(0x25);
        let library = make_lib_root(25);
        let hash = Hash256::from(library.repr_hash());
        node.cas.put(Boc::encode(library.clone()).into(), hash);

        let code_ref = CellBuilder::build_library(&HashBytes(hash.0));
        let account_boc =
            make_active_shard_account_boc_with_state(account, Some(code_ref), None, Dict::new(), 1);

        node.set_shard_account(&account, account_boc)
            .expect("imported account code library must be registered");

        let entry = found_library_entry(&node, hash).expect("code library must appear globally");
        assert_eq!(entry.lib_boc, Boc::encode(library).into());
        assert!(entry.publishers.contains(&account));
    }

    #[test]
    fn get_shard_account_registers_cached_code_reference_libraries_for_cached_account() {
        let mut node = make_test_node(Box::new(NoopExecutor));
        let account = test_addr(0x26);
        let library = make_lib_root(26);
        let hash = Hash256::from(library.repr_hash());
        node.cas.put(Boc::encode(library.clone()).into(), hash);

        let code_ref = CellBuilder::build_library(&HashBytes(hash.0));
        let account_boc =
            make_active_shard_account_boc_with_state(account, Some(code_ref), None, Dict::new(), 1);
        let meta = store_test_account_meta(&mut node, &account_boc, AccountStatus::Active);
        node.latest.accounts.insert(account, meta);

        assert!(
            found_library_entry(&node, hash).is_none(),
            "test must start without a registered code library"
        );
        assert_eq!(
            node.get_shard_account(&account)
                .expect("cached account must load"),
            account_boc
        );

        let entry = found_library_entry(&node, hash).expect("code library must appear globally");
        assert_eq!(entry.lib_boc, Boc::encode(library).into());
        assert!(entry.publishers.contains(&account));
    }

    #[test]
    fn account_code_library_reference_from_cache_registers_nested_libraries() {
        let mut node = make_test_node(Box::new(NoopExecutor));
        let account = test_addr(0x24);

        let nested = make_lib_root(24);
        let nested_hash = Hash256::from(nested.repr_hash());
        node.cas.put(Boc::encode(nested).into(), nested_hash);

        let nested_ref = CellBuilder::build_library(&HashBytes(nested_hash.0));
        let mut parent_builder = CellBuilder::new();
        parent_builder.store_u32(25).expect("must store seed");
        parent_builder
            .store_reference(nested_ref)
            .expect("must store nested library ref");
        let parent = parent_builder.build().expect("must build parent library");
        let parent_hash = Hash256::from(parent.repr_hash());
        node.cas.put(Boc::encode(parent).into(), parent_hash);

        let code_ref = CellBuilder::build_library(&HashBytes(parent_hash.0));
        let account_boc =
            make_active_shard_account_boc_with_state(account, Some(code_ref), None, Dict::new(), 1);

        node.register_account_code_libraries(&account, None, &account_boc, 24)
            .expect("cached code libraries must be registered recursively");

        assert!(
            found_library_entry(&node, parent_hash).is_some(),
            "parent code library must be registered"
        );
        assert!(
            found_library_entry(&node, nested_hash).is_some(),
            "nested code library must be registered"
        );
    }

    #[test]
    fn rebuild_global_libraries_registers_cached_code_reference_libraries() {
        let mut node = make_test_node(Box::new(NoopExecutor));
        let account = test_addr(0x23);
        let library = make_lib_root(23);
        let hash = Hash256::from(library.repr_hash());
        node.cas.put(Boc::encode(library.clone()).into(), hash);

        let code_ref = CellBuilder::build_library(&HashBytes(hash.0));
        let account_boc =
            make_active_shard_account_boc_with_state(account, Some(code_ref), None, Dict::new(), 1);
        let account_hash = account_boc.hash().expect("account BOC must hash");
        node.cas.put(account_boc, account_hash);
        node.latest.accounts.insert(
            account,
            AccountMeta {
                account_hash,
                status: AccountStatus::Active,
                balance: 1,
                last_trans_lt: Some(23),
                last_trans_hash: None,
                code_hash: None,
                data_hash: None,
                frozen_hash: None,
            },
        );

        node.rebuild_global_libraries_from_accounts()
            .expect("rebuild must register cached code refs");

        let entry = found_library_entry(&node, hash).expect("code library must appear globally");
        assert_eq!(entry.lib_boc, Boc::encode(library).into());
        assert!(entry.publishers.contains(&account));
    }

    #[test]
    fn same_public_library_by_two_accounts_has_one_entry_and_two_publishers() {
        let mut node = make_test_node(Box::new(NoopExecutor));
        let account_a = test_addr(0x33);
        let account_b = test_addr(0x44);

        let mut libs = Dict::<HashBytes, SimpleLib>::new();
        let (hash, lib) = valid_simple_lib_entry(true, 3);
        libs.set(hash, lib).expect("must insert public lib");

        let active_a = make_active_shard_account_boc(account_a, libs.clone());
        let active_b = make_active_shard_account_boc(account_b, libs);
        let empty = make_nonexist_shard_account_boc();

        node.update_public_libraries_from_account_diff(
            &account_a,
            Some(&empty),
            Some(&active_a),
            3,
        )
        .expect("must add publisher A");
        node.update_public_libraries_from_account_diff(
            &account_b,
            Some(&empty),
            Some(&active_b),
            4,
        )
        .expect("must add publisher B");

        let entry = found_library_entry(&node, Hash256::from(&hash))
            .expect("library hash must have one global entry");
        assert_eq!(entry.publishers.len(), 2, "must have 2 publishers");
        assert!(entry.publishers.contains(&account_a));
        assert!(entry.publishers.contains(&account_b));
    }

    #[test]
    fn remove_by_account_a_keeps_entry_with_account_b() {
        let mut node = make_test_node(Box::new(NoopExecutor));
        let account_a = test_addr(0x55);
        let account_b = test_addr(0x66);
        let empty = make_nonexist_shard_account_boc();

        let mut with_lib = Dict::<HashBytes, SimpleLib>::new();
        let (hash, lib) = valid_simple_lib_entry(true, 4);
        with_lib.set(hash, lib).expect("must insert public lib");
        let active_a_with_lib = make_active_shard_account_boc(account_a, with_lib.clone());
        let active_b_with_lib = make_active_shard_account_boc(account_b, with_lib);
        let active_without_lib = make_active_shard_account_boc(account_a, Dict::new());

        node.update_public_libraries_from_account_diff(
            &account_a,
            Some(&empty),
            Some(&active_a_with_lib),
            4,
        )
        .expect("must add publisher A");
        node.update_public_libraries_from_account_diff(
            &account_b,
            Some(&empty),
            Some(&active_b_with_lib),
            5,
        )
        .expect("must add publisher B");
        node.update_public_libraries_from_account_diff(
            &account_a,
            Some(&active_a_with_lib),
            Some(&active_without_lib),
            6,
        )
        .expect("must remove publisher A");

        let entry = found_library_entry(&node, Hash256::from(&hash))
            .expect("entry must remain while publisher B exists");
        assert_eq!(entry.publishers.len(), 1);
        assert!(entry.publishers.contains(&account_b));
    }

    #[test]
    fn remove_by_last_publisher_deletes_entry() {
        let mut node = make_test_node(Box::new(NoopExecutor));
        let account_a = test_addr(0x77);
        let account_b = test_addr(0x88);
        let empty = make_nonexist_shard_account_boc();

        let mut with_lib = Dict::<HashBytes, SimpleLib>::new();
        let (hash, lib) = valid_simple_lib_entry(true, 5);
        with_lib.set(hash, lib).expect("must insert public lib");
        let active_with_lib_a = make_active_shard_account_boc(account_a, with_lib.clone());
        let active_with_lib_b = make_active_shard_account_boc(account_b, with_lib);
        let active_without_lib_a = make_active_shard_account_boc(account_a, Dict::new());
        let active_without_lib_b = make_active_shard_account_boc(account_b, Dict::new());

        node.update_public_libraries_from_account_diff(
            &account_a,
            Some(&empty),
            Some(&active_with_lib_a),
            5,
        )
        .expect("must add publisher A");
        node.update_public_libraries_from_account_diff(
            &account_b,
            Some(&empty),
            Some(&active_with_lib_b),
            6,
        )
        .expect("must add publisher B");
        node.update_public_libraries_from_account_diff(
            &account_a,
            Some(&active_with_lib_a),
            Some(&active_without_lib_a),
            7,
        )
        .expect("must remove publisher A");
        node.update_public_libraries_from_account_diff(
            &account_b,
            Some(&active_with_lib_b),
            Some(&active_without_lib_b),
            8,
        )
        .expect("must remove publisher B");

        assert!(
            found_library_entry(&node, Hash256::from(&hash)).is_none(),
            "entry must be deleted when last publisher is removed"
        );
    }

    #[test]
    fn account_state_transitions_clear_published_libraries() {
        let scenarios = [
            (
                "uninit",
                make_uninit_shard_account_boc as fn(Addr) -> BocBytes,
            ),
            (
                "frozen",
                make_frozen_shard_account_boc as fn(Addr) -> BocBytes,
            ),
        ];

        for (name, next_state_builder) in scenarios {
            let mut node = make_test_node(Box::new(NoopExecutor));
            let account = test_addr(0x99);
            let empty = make_nonexist_shard_account_boc();

            let mut with_lib = Dict::<HashBytes, SimpleLib>::new();
            let (hash, lib) = valid_simple_lib_entry(true, 6);
            with_lib.set(hash, lib).expect("must insert public lib");
            let active_with_lib = make_active_shard_account_boc(account, with_lib);
            let next_state = next_state_builder(account);

            node.update_public_libraries_from_account_diff(
                &account,
                Some(&empty),
                Some(&active_with_lib),
                9,
            )
            .expect("must add library");
            node.update_public_libraries_from_account_diff(
                &account,
                Some(&active_with_lib),
                Some(&next_state),
                10,
            )
            .unwrap_or_else(|e| panic!("state transition {name} failed: {e}"));

            assert!(
                found_library_entry(&node, Hash256::from(&hash)).is_none(),
                "state transition {name} must clear published library"
            );
        }

        let mut node = make_test_node(Box::new(NoopExecutor));
        let account = test_addr(0x9A);
        let empty = make_nonexist_shard_account_boc();
        let mut with_lib = Dict::<HashBytes, SimpleLib>::new();
        let (hash, lib) = valid_simple_lib_entry(true, 7);
        with_lib.set(hash, lib).expect("must insert public lib");
        let active_with_lib = make_active_shard_account_boc(account, with_lib);
        let nonexist = make_nonexist_shard_account_boc();

        node.update_public_libraries_from_account_diff(
            &account,
            Some(&empty),
            Some(&active_with_lib),
            11,
        )
        .expect("must add library");
        node.update_public_libraries_from_account_diff(
            &account,
            Some(&active_with_lib),
            Some(&nonexist),
            12,
        )
        .expect("nonexist transition must be processed");

        assert!(
            found_library_entry(&node, Hash256::from(&hash)).is_none(),
            "nonexist transition must clear published library"
        );
    }

    #[test]
    fn mixed_actions_non_fatal_failure_uses_final_state_diff_as_source_of_truth() {
        let mut node = make_test_node(Box::new(NoopExecutor));
        let account = test_addr(0xAB);
        let old_boc = make_nonexist_shard_account_boc();

        // Final state contains public library, so it must be indexed regardless of
        // intermediate action-phase details.
        let mut new_libs = Dict::<HashBytes, SimpleLib>::new();
        let (hash, lib) = valid_simple_lib_entry(true, 8);
        new_libs
            .set(hash, lib)
            .expect("must insert final public library");
        let new_boc = make_active_shard_account_boc(account, new_libs);

        node.update_public_libraries_from_account_diff(
            &account,
            Some(&old_boc),
            Some(&new_boc),
            13,
        )
        .expect("must index from final state");

        assert!(
            found_library_entry(&node, Hash256::from(&hash)).is_some(),
            "final state with public library must be indexed"
        );
    }

    #[test]
    fn unchanged_public_library_is_indexed_when_global_cache_is_empty() {
        let mut node = make_test_node(Box::new(NoopExecutor));
        let account = test_addr(0xAC);

        let mut libs = Dict::<HashBytes, SimpleLib>::new();
        let (hash, lib) = valid_simple_lib_entry(true, 16);
        libs.set(hash, lib).expect("must insert public lib");
        let active_with_lib = make_active_shard_account_boc(account, libs);

        node.update_public_libraries_from_account_diff(
            &account,
            Some(&active_with_lib),
            Some(&active_with_lib),
            14,
        )
        .expect("unchanged public library must still be indexed");

        let entry = found_library_entry(&node, Hash256::from(&hash))
            .expect("unchanged public library must be visible");
        assert!(
            entry.publishers.contains(&account),
            "publisher must be recorded for unchanged public library"
        );
    }

    #[test]
    fn jetton_wallet_detection_uses_global_libraries_for_library_reference_code() {
        const USDT_WALLET_CODE_REF_B64: &str =
            "te6ccgEBAQEAIwAIQgKPRS16Tf10BmtoI2UXclntBXNENb52tf1L1divK3w9aA==";
        const USDT_WALLET_DATA_B64: &str = "te6ccgEBAQEASQAAjQMfVDaAEMMwjOj71Lak4LOU/ILddUHU9Jsd9EI9CQzS2z/InxmQAsROplLUCShZxn2kTkyjrdZWWw4ol9ZAosUb+zcNiHf6";
        const USDT_WALLET_LIBRARY_B64: &str = "te6ccgECDwEAA9EAART/APSkE/S88sgLAQIBYgUCAgEgBAMAIbxQj2omhpgf0AfSB9IGivgcACe/2BdqJoaYH9AH0gfSBomfwVIJhAL40AHQ0wMBcbCOSBNfA4Ag1yHtRNDTA/oA+kD6QNEE0x8BhA8hghAXjUUZugKCEHvdl966ErHy9IBA1yH6ADASoEATA8jLA1j6AgHPFgHPFsntVOD6QPpAMfoAMfQB+gAx+gABMXD4OgLTHwEgghAPin6luo6FMDRZ2zzgMwwGAtAighAXjUUZuo6EMlrbPOA0IYIQWV8HvLqOhDEB2zzgMiCCEO7SNtO6ji8wAYBA1yHTA9HtRNDTA/oA+kD6QNEzUULHBfLgSkAzA8jLA1j6AgHPFgHPFsntVOBsIYIQ03IVjLrchA/y8AgHAfLtRNDTA/oA+kD6QNEG0z8BAfoA+kD0AdFRQaFSiMcF8uBJJsL/8q/IghB73ZfeAcsfWAHLPwH6AiHPFljPFsnIgBgBywUmzxZw+gIBcVjLaszJA/g5IG6UMIEWn95xgQLycPg4AXD4NqCBGndw+DagvPKwAoBQ+wADCQP07UTQ0wP6APpA+kDRI3KwwALybQfTPwEB+gBRQaAE+kD6QFO6xwX4KlRk4HBUYAQTFQPIywNY+gIBzxYBzxbJIcjLARP0ABL0AMsAyfkAcHTIywLKB8v/ydBQDMcFG7Hy4EoJ+gAhkl8E4w0m1wsBwACzkzBsM+MNVQILCgkAIAPIywNY+gIBzxYBzxbJ7VQAelBUofgvoHOBBAmCEAlmAYBw+De2CXL7AsiAEAHLBVAFzxZw+gJwActqghDVMnbbAcsfWAHLP8mBAIL7AFkAYMiCEHNi0JwByx8lAcs/UAT6AljPFljPFsnIgBABywUkzxZY+gIBcVjLaszJgBH7AAHyA9M/AQH6APpAIfpEMMAA8uFN7UTQ0wP6APpA+kDRUwnHBSRxsMAAIbHyrVIrxwVQCrHy4ElRFaEgwv/yr/gqVCWQcFRgBBMVA8jLA1j6AgHPFgHPFskhyMsBE/QAEvQAywDJIPkAcHTIywLKB8v/ydAE+kD0AfoAIA0BmCDXCwCa10vAAQHAAbDysZEw4siCEBeNRRkByx9QCgHLP1AI+gIjzxYBzxYm+gJQB88WyciAGAHLBVAEzxZw+gJAY3dQA8trzMzJRTcOALQhkXKRceL4OSBuk4EkJ5Eg4iFulDGBKHORAeJQI6gToHOBA6Nw+DygAnD4NhKgAXD4NqBzgQQJghAJZgGAcPg3oLzysASAUPsAWAPIywNY+gIBzxYBzxbJ7VQ=";

        let mut node = make_test_node(Box::new(NoopExecutor));
        let wallet_address = parse_test_addr("EQAeizBKpsdLcS-ZOQW3_YEUYQHlCzWr2A5QMvyG1Ba7U_xb");
        let owner_address = parse_test_addr("EQCGGYRnR96ltScFnKfkFuuqDqek2O-iEehIZpbZ_kT4zJiF");
        let jetton_address = parse_test_addr("EQCxE6mUtQJKFnGfaROTKOt1lZbDiiX1kCixRv7Nw2Id_sDs");

        let code = Boc::decode_base64(USDT_WALLET_CODE_REF_B64)
            .expect("USDT wallet code reference must decode");
        let data = Boc::decode_base64(USDT_WALLET_DATA_B64).expect("USDT wallet data must decode");
        let library =
            Boc::decode_base64(USDT_WALLET_LIBRARY_B64).expect("USDT wallet library must decode");

        let library_hash = Hash256::from(library.repr_hash());
        assert_eq!(
            library_hash.to_hex().to_uppercase(),
            "8F452D7A4DFD74066B682365177259ED05734435BE76B5FD4BD5D8AF2B7C3D68"
        );
        node.global_libraries.insert(
            library_hash,
            GlobalLibraryEntry {
                hash: library_hash,
                lib_boc: Boc::encode(library).into(),
                publishers: std::iter::once(test_addr(0x77)).collect(),
                first_seen_lt: 1,
                last_seen_lt: 1,
            },
        );

        let code_hash = Hash256::from(code.repr_hash());
        node.cas.put(Boc::encode(code.clone()).into(), code_hash);
        let data_hash = Hash256::from(data.repr_hash());
        node.cas.put(Boc::encode(data.clone()).into(), data_hash);
        let account_boc = make_active_shard_account_boc_with_state(
            wallet_address,
            Some(code),
            Some(data),
            Dict::new(),
            974_433,
        );
        let account_hash = account_boc.hash().expect("account BOC must hash");
        node.cas.put(account_boc, account_hash);
        node.latest.accounts.insert(
            wallet_address,
            AccountMeta {
                account_hash,
                status: AccountStatus::Active,
                balance: 974_433,
                last_trans_lt: Some(42),
                last_trans_hash: None,
                code_hash: Some(code_hash),
                data_hash: Some(data_hash),
                frozen_hash: None,
            },
        );

        node.detect_jetton_wallets(&wallet_address)
            .expect("library-backed jetton wallet must be detected");

        let wallet = node
            .history
            .jetton_wallets
            .get(&wallet_address)
            .expect("USDT jetton wallet must be indexed");
        assert_eq!(wallet.balance, 2_053_174);
        assert_eq!(wallet.owner_address, owner_address);
        assert_eq!(wallet.jetton_address, jetton_address);
        assert_eq!(wallet.last_transaction_lt, 42);
    }

    #[test]
    fn state_limit_rollback_like_case_does_not_persist_library() {
        let mut node = make_test_node(Box::new(NoopExecutor));
        let account = test_addr(0xBC);
        let old_boc = make_nonexist_shard_account_boc();
        let new_boc = make_nonexist_shard_account_boc();

        // Final state unchanged => no library should be indexed.
        node.update_public_libraries_from_account_diff(
            &account,
            Some(&old_boc),
            Some(&new_boc),
            14,
        )
        .expect("must process rollback-like transition");

        assert!(
            node.global_libraries.is_empty(),
            "rolled back library changes must not persist"
        );
    }

    #[test]
    fn hash_mismatch_in_public_library_entry_is_rejected() {
        let mut node = make_test_node(Box::new(NoopExecutor));
        let account = test_addr(0xCD);
        let old_boc = make_nonexist_shard_account_boc();

        let mut libs = Dict::<HashBytes, SimpleLib>::new();
        let root = make_lib_root(9);
        let wrong_key = HashBytes([0xEE; 32]);
        libs.set(wrong_key, SimpleLib { public: true, root })
            .expect("must insert malformed library");

        let malformed = make_active_shard_account_boc(account, libs);
        let err = node
            .update_public_libraries_from_account_diff(
                &account,
                Some(&old_boc),
                Some(&malformed),
                15,
            )
            .expect_err("hash mismatch must be rejected");
        let err_text = err.to_string();
        assert!(
            err_text.contains("Malformed account library entry"),
            "unexpected error: {err_text}"
        );
    }

    #[allow(clippy::significant_drop_tightening)]
    #[test]
    fn next_transaction_receives_global_libs_via_set_libs_argument() {
        let recorded_libs = Arc::new(Mutex::new(Vec::<Option<BocBytes>>::new()));
        let recorded_prev_blocks_info = Arc::new(Mutex::new(Vec::<PrevBlocksInfo>::new()));
        let executor = RecordingExecutor {
            recorded_libs: Arc::clone(&recorded_libs),
            recorded_prev_blocks_info,
        };
        let mut node = make_test_node(Box::new(executor));

        let publisher = test_addr(0xDE);
        let old_boc = make_nonexist_shard_account_boc();
        let mut libs = Dict::<HashBytes, SimpleLib>::new();
        let (hash, lib) = valid_simple_lib_entry(true, 10);
        libs.set(hash, lib).expect("must insert public lib");
        let new_boc = make_active_shard_account_boc(publisher, libs);
        node.update_public_libraries_from_account_diff(
            &publisher,
            Some(&old_boc),
            Some(&new_boc),
            16,
        )
        .expect("must register global public library");

        let destination = test_addr(0xEF);
        let _ = node.faucet(&destination, 1);
        let _ = node.mine_one();

        let calls = recorded_libs.lock().expect("recorded libs mutex poisoned");
        assert!(!calls.is_empty(), "executor must be invoked");
        let libs_boc = calls[0]
            .as_ref()
            .expect("global libs must be passed to executor");
        let libs_cell = Boc::decode(libs_boc).expect("libs boc must decode");
        let mut slice = libs_cell.as_slice_allow_exotic();
        let dict =
            Dict::<HashBytes, LibDescr>::load_from_root_ext(&mut slice, Cell::empty_context())
                .expect("libs dict must decode");
        assert!(
            dict.get(HashBytes(hash.0))
                .expect("must query lib hash")
                .is_some(),
            "executor libs dict must include published library"
        );
    }

    #[allow(clippy::significant_drop_tightening)]
    #[test]
    fn state_init_code_library_reference_is_registered_before_execute() {
        let recorded_libs = Arc::new(Mutex::new(Vec::<Option<BocBytes>>::new()));
        let recorded_prev_blocks_info = Arc::new(Mutex::new(Vec::<PrevBlocksInfo>::new()));
        let executor = RecordingExecutor {
            recorded_libs: Arc::clone(&recorded_libs),
            recorded_prev_blocks_info,
        };
        let mut node = make_test_node(Box::new(executor));

        let destination = test_addr(0xF1);
        let library = make_lib_root(18);
        let hash = Hash256::from(library.repr_hash());
        node.cas.put(Boc::encode(library).into(), hash);

        let code_ref = CellBuilder::build_library(&HashBytes(hash.0));
        let state_init = StateInit {
            split_depth: None,
            special: None,
            code: Some(code_ref),
            data: None,
            libraries: Dict::new(),
        };
        let message_info = IntMsgInfo {
            ihr_disabled: true,
            bounce: true,
            bounced: false,
            src: GIVER_ADDR.into(),
            dst: destination.into(),
            ihr_fee: Default::default(),
            value: CurrencyCollection::new(1),
            fwd_fee: Default::default(),
            created_at: 0,
            created_lt: 0,
        };
        let message = OwnedMessage {
            info: MsgInfo::Int(message_info),
            init: Some(state_init),
            body: Default::default(),
            layout: None,
        };

        node.send_internal_boc(
            BocRepr::encode(message)
                .expect("message must serialize")
                .into(),
        )
        .expect("message must be queued");
        let _ = node.mine_one();

        let calls = recorded_libs.lock().expect("recorded libs mutex poisoned");
        assert!(!calls.is_empty(), "executor must be invoked");
        let libs_boc = calls[0]
            .as_ref()
            .expect("state init code library must be passed to executor");
        let libs_cell = Boc::decode(libs_boc).expect("libs boc must decode");
        let mut slice = libs_cell.as_slice_allow_exotic();
        let dict =
            Dict::<HashBytes, LibDescr>::load_from_root_ext(&mut slice, Cell::empty_context())
                .expect("libs dict must decode");
        assert!(
            dict.get(HashBytes(hash.0))
                .expect("must query lib hash")
                .is_some(),
            "executor libs dict must include state init code library"
        );
    }

    #[test]
    fn public_library_added_after_private_transition_becomes_visible() {
        let mut node = make_test_node(Box::new(NoopExecutor));
        let account = test_addr(0xF0);
        let empty = make_nonexist_shard_account_boc();

        let mut private_libs = Dict::<HashBytes, SimpleLib>::new();
        let (hash, private_lib) = valid_simple_lib_entry(false, 11);
        private_libs
            .set(hash, private_lib)
            .expect("must insert private lib");
        let private_state = make_active_shard_account_boc(account, private_libs);

        let mut public_libs = Dict::<HashBytes, SimpleLib>::new();
        let (same_hash, public_lib) = valid_simple_lib_entry(true, 11);
        assert_eq!(hash, same_hash, "same seed should produce same hash");
        public_libs
            .set(same_hash, public_lib)
            .expect("must insert public lib");
        let public_state = make_active_shard_account_boc(account, public_libs);

        node.update_public_libraries_from_account_diff(
            &account,
            Some(&empty),
            Some(&private_state),
            17,
        )
        .expect("private transition must succeed");
        assert!(
            found_library_entry(&node, Hash256::from(&hash)).is_none(),
            "private library must stay hidden"
        );

        node.update_public_libraries_from_account_diff(
            &account,
            Some(&private_state),
            Some(&public_state),
            18,
        )
        .expect("public transition must succeed");
        assert!(
            found_library_entry(&node, Hash256::from(&hash)).is_some(),
            "public transition must expose library"
        );
    }

    #[test]
    fn rollback_like_noop_update_keeps_existing_unrelated_public_library() {
        let mut node = make_test_node(Box::new(NoopExecutor));
        let account_a = test_addr(0xA1);
        let account_b = test_addr(0xB1);
        let empty = make_nonexist_shard_account_boc();

        let mut libs_a = Dict::<HashBytes, SimpleLib>::new();
        let (hash_a, lib_a) = valid_simple_lib_entry(true, 12);
        libs_a.set(hash_a, lib_a).expect("must insert A lib");
        let active_a = make_active_shard_account_boc(account_a, libs_a);
        node.update_public_libraries_from_account_diff(
            &account_a,
            Some(&empty),
            Some(&active_a),
            19,
        )
        .expect("must index A library");

        let old_b = make_nonexist_shard_account_boc();
        let new_b = make_nonexist_shard_account_boc();
        node.update_public_libraries_from_account_diff(&account_b, Some(&old_b), Some(&new_b), 20)
            .expect("rollback-like noop for B must succeed");

        assert!(
            found_library_entry(&node, Hash256::from(&hash_a)).is_some(),
            "unrelated noop update must not affect existing global library"
        );
    }

    #[test]
    fn malformed_global_library_storage_is_rejected_when_building_vm_dict() {
        let mut node = make_test_node(Box::new(NoopExecutor));
        let hash = Hash256([0x42; 32]);
        let wrong_root = make_lib_root(99);
        let wrong_boc: BocBytes = Boc::encode(wrong_root).into();
        node.global_libraries.insert(
            hash,
            GlobalLibraryEntry {
                hash,
                lib_boc: wrong_boc,
                publishers: std::iter::once(test_addr(0x01)).collect(),
                first_seen_lt: 1,
                last_seen_lt: 1,
            },
        );
        node.global_libs_dirty = true;
        node.global_libs_boc = None;

        let err = node
            .build_vm_global_libs_boc()
            .expect_err("corrupted storage must fail");
        let err_text = err.to_string();
        assert!(
            err_text.contains("hash mismatch"),
            "unexpected error: {err_text}"
        );
    }

    #[test]
    fn get_libraries_preserves_request_order_and_includes_not_found_entries() {
        let mut node = make_test_node(Box::new(NoopExecutor));
        let account = test_addr(0x01);
        let empty = make_nonexist_shard_account_boc();

        let mut libs = Dict::<HashBytes, SimpleLib>::new();
        let (hash_a, lib_a) = valid_simple_lib_entry(true, 13);
        let (hash_b, lib_b) = valid_simple_lib_entry(true, 14);
        libs.set(hash_a, lib_a).expect("must insert lib A");
        libs.set(hash_b, lib_b).expect("must insert lib B");
        let active = make_active_shard_account_boc(account, libs);

        node.update_public_libraries_from_account_diff(&account, Some(&empty), Some(&active), 21)
            .expect("must index libraries");

        let missing = Hash256([0xEE; 32]);
        let entries =
            node.get_libraries(&[missing, Hash256::from(&hash_b), Hash256::from(&hash_a)]);
        assert_eq!(entries.len(), 3);
        assert!(
            entries[0].is_none(),
            "missing hash must be returned as not found"
        );
        assert_eq!(
            entries[1].as_ref().map(|entry| entry.hash),
            Some(Hash256::from(&hash_b))
        );
        assert_eq!(
            entries[2].as_ref().map(|entry| entry.hash),
            Some(Hash256::from(&hash_a))
        );
    }

    #[test]
    fn rebuild_global_libraries_uses_min_first_seen_lt_for_same_hash() {
        let mut node = make_test_node(Box::new(NoopExecutor));
        let account_high_lt = test_addr(0x01);
        let account_low_lt = test_addr(0xFE);

        let (lib_hash, lib) = valid_simple_lib_entry(true, 15);
        let mut high_libs = Dict::<HashBytes, SimpleLib>::new();
        high_libs
            .set(lib_hash, lib.clone())
            .expect("must insert high-lt library");
        let high_boc = make_active_shard_account_boc(account_high_lt, high_libs);
        let high_account_hash = high_boc.hash().expect("must hash high-lt shard account");
        node.cas.put(high_boc, high_account_hash);

        let mut low_libs = Dict::<HashBytes, SimpleLib>::new();
        low_libs
            .set(lib_hash, lib)
            .expect("must insert low-lt library");
        let low_boc = make_active_shard_account_boc(account_low_lt, low_libs);
        let low_account_hash = low_boc.hash().expect("must hash low-lt shard account");
        node.cas.put(low_boc, low_account_hash);

        node.latest.accounts.insert(
            account_high_lt,
            AccountMeta {
                account_hash: high_account_hash,
                status: AccountStatus::Active,
                balance: 0,
                last_trans_lt: Some(100),
                last_trans_hash: None,
                code_hash: None,
                data_hash: None,
                frozen_hash: None,
            },
        );
        node.latest.accounts.insert(
            account_low_lt,
            AccountMeta {
                account_hash: low_account_hash,
                status: AccountStatus::Active,
                balance: 0,
                last_trans_lt: Some(5),
                last_trans_hash: None,
                code_hash: None,
                data_hash: None,
                frozen_hash: None,
            },
        );

        node.rebuild_global_libraries_from_accounts()
            .expect("must rebuild global libraries from accounts");
        let entry = found_library_entry(&node, Hash256::from(&lib_hash))
            .expect("shared public library must be present");
        assert_eq!(
            entry.first_seen_lt, 5,
            "first_seen_lt must be min publisher lt"
        );
        assert_eq!(
            entry.last_seen_lt, 100,
            "last_seen_lt must be max publisher lt"
        );
        assert_eq!(entry.publishers.len(), 2, "both publishers must be present");
    }

    #[test]
    fn set_libs_dict_is_empty_when_no_global_libraries() {
        let mut node = make_test_node(Box::new(NoopExecutor));
        let libs = node
            .build_vm_global_libs_boc()
            .expect("must build libs dict for empty state");
        assert!(
            libs.is_none(),
            "VM libs should be absent when there are no global libraries"
        );
    }
}
