use crate::executor::{ExecContext, TvmExecutor};
use crate::remote::{RemoteProvider, fetch_remote_shard_account};
use crate::storage::{
    AccountDelta, AccountMeta, AccountStatus, BlockMeta, CellStore, Globals, History, Indexes,
    LatestState, MessageInfo, MessagePool, MsgMeta, PendingCommit, ReverseLtKey, TraceNode,
    TransactionInfo, TxMeta,
};
use crate::types::{Addr, BocBytes, Hash256, Lt, Seqno};
use anyhow::Context;
use core::cmp;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tonlib_core::TonHash;
use tonlib_core::tlb_types::block::coins::{CurrencyCollection, Grams};
use tonlib_core::tlb_types::primitives::either::EitherRef;
use tycho_types::boc::Boc;
use tycho_types::cell::{CellBuilder, CellFamily, Store};
use tycho_types::models::{AccountState, Message, MsgInfo, ShardAccount};

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
}

pub const GIVER_ADDR: Addr = Addr {
    workchain: 0,
    addr: [0x55; 32],
};

pub const GIVER_BALANCE: u128 = 1_000_000_000_000_000_000; // 1B TON

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
            Some(conn)
        } else {
            None
        };

        let config_hash = compute_boc_hash(&config_boc)?;

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
                let (hash, tx_meta, addr, lt, seqno) = tx?;
                history.tx_by_hash.insert(hash, tx_meta);

                let key = ReverseLtKey(cmp::Reverse(lt), hash);
                indexes
                    .tx_by_account
                    .entry(addr)
                    .or_default()
                    .insert(key, hash);
                indexes.tx_by_block.insert(seqno, hash);
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
                cached_balance: Some(GIVER_BALANCE),
                last_trans_lt: None,
                last_trans_hash: None,
                code_hash: None,
                data_hash: None,
                frozen_hash: None,
            });

        let mut globals = Globals::new(config_hash);
        globals.head_seqno = head_seqno;
        // Approximation of global LT
        globals.global_lt = history.blocks.last().map(|b| b.end_lt).unwrap_or(0);

        Ok(Self {
            cas,
            latest,
            history,
            indexes,
            globals,
            pool: MessagePool::new(),
            executor,
            state_source,
            conn,
        })
    }

    pub fn send_boc(&mut self, boc: BocBytes) -> anyhow::Result<(Hash256, Seqno, Vec<Hash256>)> {
        // 1. Validate & Store
        let hash = compute_boc_hash(&boc)?;
        tracing::info!(
            "send_boc: msg_hash={}, current_queue={}",
            hash.to_hex(),
            self.pool.external.len() + self.pool.internal.len()
        );
        if self.cas.get(&hash).is_none() {
            self.cas.put(boc.clone(), hash);
        }

        // 2. Parse minimal meta
        let msg_meta = parse_msg_meta(&boc, hash)?;

        // 3. Register MsgMeta
        self.history.msg_by_hash.insert(hash, msg_meta);

        // 4. Enqueue
        self.pool.push_external(hash);

        // 5. Mine one
        let (block_meta, tx_meta) = self.mine_one()?;

        Ok((tx_meta.tx_hash, block_meta.seqno, tx_meta.out_msg_hashes))
    }

    pub fn mine_one(&mut self) -> anyhow::Result<(BlockMeta, TxMeta)> {
        // 1. Select message
        let msg_hash = self
            .pool
            .pop_next(self.globals.queue_policy, &self.history.msg_by_hash)
            .context("Queue empty")?;

        // 2. Load inbound message
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
        let old_meta = self.latest.accounts.get(&dst).cloned();

        // 4. Allocate LT & time
        let lt = self.globals.global_lt + self.globals.lt_step;
        self.globals.global_lt = lt;
        let gen_utime = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as u32;

        // 5. Execute
        let config_boc = self
            .cas
            .get(&self.globals.config_boc_hash)
            .context("Config missing")?;
        let ctx = ExecContext {
            lt,
            gen_utime,
            rand_seed: None,
        };

        let exec_result = self
            .executor
            .execute(&shard_account_boc, &msg_boc, &ctx, &config_boc)?;

        // 6. Store outputs & 7. Derive hashes
        let tx_hash = compute_boc_hash(&exec_result.tx_boc)?;
        self.cas.put(exec_result.tx_boc.clone(), tx_hash);

        let mut balance_cache = None;
        let mut status = AccountStatus::Nonexist;
        let mut code_hash = None;
        let mut data_hash = None;
        let mut frozen_hash = None;

        let new_account_hash = if let Some(acc_boc) = &exec_result.new_account_boc {
            let h = compute_boc_hash(acc_boc)?;
            self.cas.put(acc_boc.clone(), h);

            // Parse for meta
            if let Ok(cell) = Boc::decode(acc_boc)
                && let Ok(sa) = cell.parse::<ShardAccount>()
                && let Ok(opt_acc) = sa.account.load()
                && let Some(acc) = opt_acc.0
            {
                balance_cache = Some(acc.balance.tokens.into());
                status = match acc.state {
                    AccountState::Uninit => AccountStatus::Uninit,
                    AccountState::Active(state) => {
                        if let Some(cell) = state.code {
                            let ch = Hash256(*cell.repr_hash().as_array());
                            let boc = Boc::encode(cell);
                            self.cas.put(boc, ch);
                            code_hash = Some(ch);
                        }
                        if let Some(cell) = state.data {
                            let dh = Hash256(*cell.repr_hash().as_array());
                            let boc = Boc::encode(cell);
                            self.cas.put(boc, dh);
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
            Some(h)
        } else {
            None
        };

        let mut out_msg_hashes = Vec::new();
        for out_boc in &exec_result.out_msgs_boc {
            let h = compute_boc_hash(out_boc)?;
            self.cas.put(out_boc.clone(), h);
            out_msg_hashes.push(h);

            let out_meta = parse_msg_meta(out_boc, h)?;
            self.history.msg_by_hash.insert(h, out_meta);
        }

        // 8. Build dev block
        let seqno = self.globals.head_seqno + 1;
        let block_boc = create_dev_block_boc(seqno, tx_hash)?;
        let block_hash = compute_boc_hash(&block_boc)?;
        self.cas.put(block_boc, block_hash);

        let block_meta = BlockMeta {
            seqno,
            prev_seqno: if seqno > 1 { Some(seqno - 1) } else { None },
            gen_utime,
            start_lt: lt,
            end_lt: lt,
            tx_hash,
            block_boc_hash: block_hash,
        };

        let compute_exit_code = exec_result.compute_exit_code();
        let action_result_code = exec_result.action_result_code();

        let info = exec_result.tx.info.load().ok();
        let (storage_fees, other_fees) =
            if let Some(tycho_types::models::TxInfo::Ordinary(ord)) = info {
                let storage: u128 = ord
                    .storage_phase
                    .map(|p| p.storage_fees_collected.into())
                    .unwrap_or(0);
                let total: u128 = exec_result.tx.total_fees.tokens.into();
                (storage, total.saturating_sub(storage))
            } else {
                (0, exec_result.tx.total_fees.tokens.into())
            };
        let total_fees = exec_result.tx.total_fees.tokens.into();

        let tx_meta = TxMeta {
            tx_hash,
            account: dst,
            lt,
            now: gen_utime,
            success: compute_exit_code == Some(0) && action_result_code == Some(0),
            compute_exit_code,
            action_result_code,
            total_fees: Some(total_fees),
            storage_fees: Some(storage_fees),
            other_fees: Some(other_fees),
            in_msg_hash: Some(msg_hash),
            out_msg_hashes: out_msg_hashes.clone(),
            block_seqno: seqno,
        };

        // 9. Prepare deltas
        let new_meta = new_account_hash.map(|hash| AccountMeta {
            account_hash: hash,
            status,
            cached_balance: balance_cache,
            last_trans_lt: Some(lt),
            last_trans_hash: Some(tx_hash),
            code_hash,
            data_hash,
            frozen_hash,
        });

        let delta = AccountDelta {
            addr: dst,
            old_hash: old_meta.as_ref().map(|m| m.account_hash),
            new_hash: new_account_hash,
            old_meta,
            new_meta,
        };

        // 10. Commit
        let pending = PendingCommit {
            block_meta: block_meta.clone(),
            tx_meta: tx_meta.clone(),
            delta,
            out_msg_hashes,
            msg_to_tx: vec![(msg_hash, tx_hash)],
        };

        self.apply_commit(pending)?;

        Ok((block_meta, tx_meta))
    }

    fn apply_commit(&mut self, pending: PendingCommit) -> anyhow::Result<()> {
        tracing::info!(
            "Applying block commit: seqno={}, tx_hash={}",
            pending.block_meta.seqno,
            pending.tx_meta.tx_hash.to_hex()
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

            // Save transaction
            let tx_data = serde_json::to_vec(&pending.tx_meta)?;
            conn.execute(
                "INSERT OR REPLACE INTO transactions (hash, data, account, lt, seqno) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    pending.tx_meta.tx_hash.0.to_vec(),
                    tx_data,
                    pending.tx_meta.account.addr.to_vec(),
                    pending.tx_meta.lt,
                    pending.block_meta.seqno
                ],
            )?;

            // Save account state
            if let Some(new_meta) = &pending.delta.new_meta {
                let account_data = serde_json::to_vec(new_meta)?;
                conn.execute(
                    "INSERT OR REPLACE INTO accounts (address, data) VALUES (?1, ?2)",
                    params![pending.delta.addr.addr.to_vec(), account_data],
                )?;
            }

            // Save messages
            for h in &pending.out_msg_hashes {
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
        if let Some(new_meta) = &pending.delta.new_meta {
            self.latest
                .accounts
                .insert(pending.delta.addr, new_meta.clone());
        } else {
            self.latest.accounts.remove(&pending.delta.addr);
        }

        // History
        self.history.blocks.push(pending.block_meta.clone());

        let seqno = pending.block_meta.seqno;
        if self.history.deltas_by_seqno.len() < seqno as usize {
            self.history
                .deltas_by_seqno
                .resize(seqno as usize, Vec::new());
        }
        // seqno is 1-based, index is seqno-1
        if seqno > 0 {
            self.history.deltas_by_seqno[seqno as usize - 1].push(pending.delta);
        }

        self.history
            .tx_by_hash
            .insert(pending.tx_meta.tx_hash, pending.tx_meta.clone());

        for (msg, tx) in pending.msg_to_tx {
            self.history.msg_to_tx.insert(msg, tx);
        }

        // Indexes
        let key = ReverseLtKey(cmp::Reverse(pending.tx_meta.lt), pending.tx_meta.tx_hash);
        self.indexes
            .tx_by_account
            .entry(pending.tx_meta.account)
            .or_default()
            .insert(key, pending.tx_meta.tx_hash);
        self.indexes
            .tx_by_block
            .insert(seqno, pending.tx_meta.tx_hash);

        // Enqueue out msgs
        for h in pending.out_msg_hashes {
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
            if let Ok((_, meta)) = fetch_remote_shard_account(addr, &provider, &mut self.cas) {
                self.latest.accounts.insert(*addr, meta.clone());
                return Some(meta);
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

        // search backwards from seqno to find the state as it was after block 'seqno'
        for s in (1..=seqno).rev() {
            if s as usize > self.history.deltas_by_seqno.len() {
                continue;
            }
            let deltas = &self.history.deltas_by_seqno[s as usize - 1];
            for delta in deltas {
                if delta.addr == *addr {
                    return delta.new_meta.clone();
                }
            }
        }
        None
    }

    pub fn get_cell(&self, hash: &Hash256) -> Option<BocBytes> {
        self.cas.get(hash)
    }

    pub fn get_transactions(
        &self,
        addr: &Addr,
        limit: usize,
        lt: Option<Lt>,
        hash: Option<Hash256>,
    ) -> Vec<TransactionInfo> {
        let index = if let Some(map) = self.indexes.tx_by_account.get(addr) {
            map
        } else {
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
            .map(|tx| {
                let in_msg = tx.in_msg_hash.and_then(|h| self.get_message_info(&h));
                let out_msgs = tx
                    .out_msg_hashes
                    .iter()
                    .filter_map(|h| self.get_message_info(h))
                    .collect();
                TransactionInfo {
                    meta: tx,
                    in_msg,
                    out_msgs,
                }
            })
            .collect()
    }

    pub fn get_block_header(&self, seqno: Seqno) -> Option<BlockMeta> {
        if seqno == 0 || seqno as usize > self.history.blocks.len() {
            None
        } else {
            Some(self.history.blocks[seqno as usize - 1].clone())
        }
    }

    pub fn find_block_by_lt(&self, lt: Lt) -> Option<BlockMeta> {
        self.history
            .blocks
            .iter()
            .find(|b| lt >= b.start_lt && lt <= b.end_lt)
            .cloned()
    }

    pub fn find_block_by_unixtime(&self, utime: u32) -> Option<BlockMeta> {
        // Find block with gen_utime closest but not greater than utime
        self.history
            .blocks
            .iter()
            .filter(|b| b.gen_utime <= utime)
            .next_back()
            .cloned()
    }

    pub fn get_block_transactions(&self, block_meta: &BlockMeta) -> Option<Vec<TxMeta>> {
        let tx_hash = self.indexes.tx_by_block.get(&block_meta.seqno)?;
        let tx = self.history.tx_by_hash.get(tx_hash).cloned()?;
        Some(vec![tx])
    }

    pub fn get_message_info(&self, hash: &Hash256) -> Option<MessageInfo> {
        let meta = self.history.msg_by_hash.get(hash).cloned()?;
        let boc = self.cas.get(&meta.msg_boc_hash)?;
        Some(MessageInfo { meta, boc })
    }

    pub fn get_transaction_by_hash(&self, hash: &Hash256) -> Option<TransactionInfo> {
        let tx = self.history.tx_by_hash.get(hash).cloned()?;
        let in_msg = tx.in_msg_hash.and_then(|h| self.get_message_info(&h));
        let out_msgs = tx
            .out_msg_hashes
            .iter()
            .filter_map(|h| self.get_message_info(h))
            .collect();
        Some(TransactionInfo {
            meta: tx,
            in_msg,
            out_msgs,
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

                    // Find transaction that produced this message
                    let mut found_parent = false;
                    for (h, t) in &self.history.tx_by_hash {
                        if t.out_msg_hashes.contains(in_msg_hash) {
                            if visited_up.contains(h) {
                                // Cycle detected
                                break;
                            }
                            root_hash = *h;
                            curr_tx_hash = *h;
                            visited_up.insert(*h);
                            found_parent = true;
                            break;
                        }
                    }
                    if !found_parent {
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

        let mut trace = self
            .build_trace_node(&root_hash)
            .ok_or_else(|| anyhow::anyhow!("Root transaction not found"))?;
        trace.external_hash = external_hash;
        Ok(trace)
    }

    fn build_trace_node(&self, tx_hash: &Hash256) -> Option<TraceNode> {
        let tx_info = self.get_transaction_by_hash(tx_hash)?;
        let mut children = Vec::new();

        for out_msg in &tx_info.meta.out_msg_hashes {
            if let Some(child_tx_hash) = self.history.msg_to_tx.get(out_msg)
                && let Some(child_node) = self.build_trace_node(child_tx_hash)
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
        if let Some(meta) = self.latest.accounts.get(addr)
            && let Some(boc) = self.cas.get(&meta.account_hash)
        {
            return Ok(boc);
        }

        if let StateSource::Remote(provider) = &self.state_source {
            let provider = provider.clone();
            if let Ok(Some(boc)) = self.fetch_remote_shard_account(addr, &provider) {
                return Ok(boc);
            }
        }

        // Create empty shard account
        let sa = ShardAccount {
            account: tycho_types::cell::Lazy::new(&tycho_types::models::OptionalAccount(None))?,
            last_trans_hash: tycho_types::prelude::HashBytes([0u8; 32]),
            last_trans_lt: 0,
        };
        let mut builder = CellBuilder::new();
        sa.store_into(&mut builder, tycho_types::cell::Cell::empty_context())?;
        let cell = builder.build()?;
        Ok(Boc::encode(cell))
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
        self.latest.accounts.insert(*addr, meta);
        Ok(Some(boc))
    }

    pub fn has_pending_messages(&self) -> bool {
        !self.pool.external.is_empty() || !self.pool.internal.is_empty()
    }

    pub fn faucet(&mut self, addr: &Addr, amount: u128) -> anyhow::Result<Value> {
        let mut giver_meta = self
            .latest
            .accounts
            .get(&GIVER_ADDR)
            .cloned()
            .context("Giver account not found")?;
        let giver_balance = giver_meta.cached_balance.unwrap_or(0);
        if giver_balance < amount {
            anyhow::bail!("Giver has insufficient balance");
        }

        use tonlib_core::cell::ArcCell;
        use tonlib_core::tlb_types::block::message::{CommonMsgInfo, IntMsgInfo, Message};
        use tonlib_core::tlb_types::tlb::TLB;
        use tonlib_core::types::TonAddress;

        let src = TonAddress::new(GIVER_ADDR.workchain, TonHash::from(&GIVER_ADDR.addr));
        let dst = TonAddress::new(addr.workchain, TonHash::from(&addr.addr));

        let message_info = IntMsgInfo {
            ihr_disabled: true,
            bounce: false,
            bounced: false,
            src: src.to_msg_address(),
            dest: dst.to_msg_address(),
            value: CurrencyCollection::new(amount.into()),
            ihr_fee: Grams::new(0u64.into()),
            fwd_fee: Grams::new(0u64.into()),
            created_at: 0,
            created_lt: 0,
        };

        let message = Message {
            info: CommonMsgInfo::Int(message_info),
            init: None,
            body: EitherRef::new(ArcCell::default()),
        };

        let boc = message.to_cell()?.to_boc(false)?;
        let hash = compute_boc_hash(&boc)?;
        self.cas.put(boc.clone(), hash);

        // 2. Register MsgMeta
        let msg_meta = parse_msg_meta(&boc, hash)?;
        self.history.msg_by_hash.insert(hash, msg_meta);

        // 3. Decrease Giver balance
        giver_meta.cached_balance = Some(giver_balance - amount);
        self.latest.accounts.insert(GIVER_ADDR, giver_meta);

        // 4. Enqueue internal message
        self.pool.push_internal(hash);

        // 5. Mine one
        let (block_meta, tx_meta) = self.mine_one()?;

        Ok(serde_json::json!({
            "ok": true,
            "result": {
                "tx_hash": tx_meta.tx_hash.to_hex(),
                "block_seqno": block_meta.seqno
            }
        }))
    }
}

fn compute_boc_hash(boc: &[u8]) -> anyhow::Result<Hash256> {
    let cell = Boc::decode(boc)?;
    let hash = cell.repr_hash();
    Ok(Hash256(*hash.as_array()))
}

fn parse_msg_meta(boc: &[u8], hash: Hash256) -> anyhow::Result<MsgMeta> {
    let cell = Boc::decode(boc)?;
    let msg = cell.parse::<Message<'_>>()?;

    let (src, dst, value, bounce, created_lt, created_at) = match msg.info {
        MsgInfo::Int(info) => (
            Some(convert_addr(&info.src)),
            Some(convert_addr(&info.dst)),
            Some(info.value.tokens.into()),
            Some(info.bounce),
            Some(info.created_lt),
            Some(info.created_at),
        ),
        MsgInfo::ExtIn(info) => (None, Some(convert_addr(&info.dst)), None, None, None, None),
        MsgInfo::ExtOut(info) => (
            Some(convert_addr(&info.src)),
            None,
            None,
            None,
            Some(info.created_lt),
            Some(info.created_at),
        ),
    };

    Ok(MsgMeta {
        msg_hash: hash,
        msg_boc_hash: hash,
        src,
        dst,
        value,
        bounce,
        created_lt,
        created_at,
    })
}

const fn convert_addr(addr: &tycho_types::models::IntAddr) -> Addr {
    let mut bytes = [0u8; 32];
    let (workchain, address) = match addr {
        tycho_types::models::IntAddr::Std(std) => (std.workchain as i32, std.address.0),
        tycho_types::models::IntAddr::Var(var) => (var.workchain, {
            // skipped from TVM 11
            [0u8; 32]
        }),
    };
    bytes.copy_from_slice(&address);
    Addr {
        workchain,
        addr: bytes,
    }
}

fn create_dev_block_boc(seqno: Seqno, tx_hash: Hash256) -> anyhow::Result<BocBytes> {
    let mut builder = CellBuilder::new();
    builder.store_u32(seqno)?;
    builder.store_u256(&tycho_types::prelude::HashBytes(tx_hash.0))?;
    let cell = builder.build()?;
    Ok(Boc::encode(cell))
}
