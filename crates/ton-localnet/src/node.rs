use crate::executor::{ExecContext, TvmExecutor};
use crate::localnet::compute_normalized_ext_in_hash;
use crate::remote::{RemoteProvider, fetch_remote_shard_account};
use crate::storage::{
    self, GlobalLibraryEntry, GlobalLibraryLookup, JettonMasterMeta, NftItemMeta,
};
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
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tycho_types::boc::Boc;
use tycho_types::boc::BocRepr;
use tycho_types::cell::{CellBuilder, CellFamily, Store};
use tycho_types::models::{
    AccountState, CurrencyCollection, IntAddr, IntMsgInfo, LibDescr, Message, MsgInfo,
    OwnedMessage, ShardAccount, StdAddr, StdAddrFormat,
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
    pub vm_global_libs_boc: Option<BocBytes>,
    pub vm_global_libs_dirty: bool,
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
        globals.global_lt = history.blocks.last().map_or(0, |b| b.end_lt);

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
            vm_global_libs_boc: None,
            vm_global_libs_dirty: true,
        };
        node.rebuild_global_libraries_from_accounts()?;
        Ok(node)
    }

    pub fn send_boc(
        &mut self,
        boc: BocBytes,
    ) -> anyhow::Result<(Hash256, Hash256, Seqno, Vec<Hash256>)> {
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

        Ok((
            hash,
            tx_meta.tx_hash,
            block_meta.seqno,
            tx_meta.out_msg_hashes,
        ))
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
            ignore_chksig: false,
        };
        let vm_global_libs = self.build_vm_global_libs_boc()?;

        let exec_result = self.executor.execute(
            &shard_account_boc,
            &msg_boc,
            &ctx,
            &config_boc,
            vm_global_libs.as_ref(),
        )?;

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
                            self.cas.put(boc.into(), ch);
                            code_hash = Some(ch);
                        }
                        if let Some(cell) = state.data {
                            let dh = Hash256(*cell.repr_hash().as_array());
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
                    .map_or(0, |p| p.storage_fees_collected.into());
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
        self.update_public_libraries_from_account_diff(
            &dst,
            Some(&shard_account_boc),
            exec_result.new_account_boc.as_ref(),
            lt,
        )?;

        self.detect_jetton_masters(&dst)?;
        self.detect_jetton_wallets(&dst)?;
        self.detect_nft_items(&dst)?;

        Ok((block_meta, tx_meta))
    }

    fn detect_jetton_wallets(&mut self, addr: &Addr) -> anyhow::Result<()> {
        let Some(meta) = self.latest.accounts.get(addr) else {
            return Ok(());
        };

        if meta.status != AccountStatus::Active {
            return Ok(());
        }

        let Some(code_hash) = meta.code_hash else {
            return Ok(());
        };
        let Some(data_hash) = meta.data_hash else {
            return Ok(());
        };

        let Some(code_boc) = self.cas.get(&code_hash) else {
            return Ok(());
        };
        let Some(data_boc) = self.cas.get(&data_hash) else {
            return Ok(());
        };

        let code = Boc::decode(&code_boc)?;
        let data = Boc::decode(&data_boc)?;

        if let Some(wallet_data) =
            ton_indexer::jettons::get_jetton_wallet_data(addr.to_string(), code, data)
        {
            let wallet_meta = storage::JettonWalletMeta {
                address: *addr,
                balance: wallet_data.balance.to_str_radix(10).parse().unwrap_or(0),
                code_hash,
                data_hash,
                jetton_address: self
                    .parse_addr_internal(&wallet_data.jetton_master_address)
                    .unwrap_or(*addr),
                last_transaction_lt: meta.last_trans_lt.unwrap_or(0),
                owner_address: self
                    .parse_addr_internal(&wallet_data.owner_address)
                    .unwrap_or(*addr),
            };

            self.history.jetton_wallets.insert(*addr, wallet_meta);
        }

        Ok(())
    }

    fn detect_jetton_masters(&mut self, addr: &Addr) -> anyhow::Result<()> {
        let Some(meta) = self.latest.accounts.get(addr) else {
            return Ok(());
        };

        if meta.status != AccountStatus::Active {
            return Ok(());
        }

        let Some(code_hash) = meta.code_hash else {
            return Ok(());
        };
        let Some(data_hash) = meta.data_hash else {
            return Ok(());
        };

        let Some(code_boc) = self.cas.get(&code_hash) else {
            return Ok(());
        };
        let Some(data_boc) = self.cas.get(&data_hash) else {
            return Ok(());
        };

        let code = Boc::decode(&code_boc)?;
        let data = Boc::decode(&data_boc)?;

        if let Some(jetton_data) =
            ton_indexer::jettons::get_jetton_data(addr.to_string(), code, data)
        {
            let wallet_code_hash = Hash256(*jetton_data.jetton_wallet_code.repr_hash().as_array());
            let jetton_content =
                ton_indexer::jettons::parse_jetton_content(jetton_data.jetton_content);

            let master_meta = JettonMasterMeta {
                address: *addr,
                admin_address: self
                    .parse_addr_internal(&jetton_data.admin_address)
                    .unwrap_or(*addr),
                code_hash,
                data_hash,
                jetton_content,
                jetton_wallet_code_hash: wallet_code_hash,
                last_transaction_lt: meta.last_trans_lt.unwrap_or(0),
                mintable: jetton_data.mintable,
                total_supply: jetton_data
                    .total_supply
                    .to_str_radix(10)
                    .parse()
                    .unwrap_or(0),
            };

            self.history.jetton_masters.insert(*addr, master_meta);
        }

        Ok(())
    }

    fn detect_nft_items(&mut self, addr: &Addr) -> anyhow::Result<()> {
        let Some(meta) = self.latest.accounts.get(addr) else {
            return Ok(());
        };

        if meta.status != AccountStatus::Active {
            return Ok(());
        }

        let Some(code_hash) = meta.code_hash else {
            return Ok(());
        };
        let Some(data_hash) = meta.data_hash else {
            return Ok(());
        };

        let Some(code_boc) = self.cas.get(&code_hash) else {
            return Ok(());
        };
        let Some(data_boc) = self.cas.get(&data_hash) else {
            return Ok(());
        };

        let code = Boc::decode(&code_boc)?;
        let data = Boc::decode(&data_boc)?;

        if let Some(nft_data) = ton_indexer::nfts::get_nft_item_data(addr.to_string(), code, data) {
            let nft_meta = NftItemMeta {
                address: *addr,
                code_hash,
                data_hash,
                collection_address: nft_data
                    .collection_address
                    .as_deref()
                    .and_then(|a| self.parse_addr_internal(a)),
                owner_address: nft_data
                    .owner_address
                    .as_deref()
                    .and_then(|a| self.parse_addr_internal(a)),
                content: ton_indexer::nfts::parse_nft_content(nft_data.individual_content),
                index: nft_data.index.to_str_radix(10),
                init: nft_data.init,
                last_transaction_lt: meta.last_trans_lt.unwrap_or(0),
            };

            self.history.nft_items.insert(*addr, nft_meta);
        }

        Ok(())
    }

    pub fn get_jetton_masters(
        &self,
        address: Option<Addr>,
        admin_address: Option<Addr>,
        limit: usize,
        offset: usize,
    ) -> anyhow::Result<Vec<JettonMasterMeta>> {
        let mut masters: Vec<_> = self
            .history
            .jetton_masters
            .values()
            .filter(|m| {
                if let Some(addr) = address
                    && m.address != addr
                {
                    return false;
                }
                if let Some(addr) = admin_address
                    && m.admin_address != addr
                {
                    return false;
                }
                true
            })
            .cloned()
            .collect();

        masters.sort_by_key(|m| m.address);

        let start = offset.min(masters.len());
        let end = (start + limit).min(masters.len());

        Ok(masters[start..end].to_vec())
    }

    pub fn get_jetton_wallets(
        &self,
        address: Option<Addr>,
        owner_address: Option<Addr>,
        jetton_address: Option<Addr>,
        exclude_zero_balance: bool,
        limit: usize,
        offset: usize,
    ) -> anyhow::Result<Vec<storage::JettonWalletMeta>> {
        let mut wallets: Vec<_> = self
            .history
            .jetton_wallets
            .values()
            .filter(|w| {
                if let Some(addr) = address
                    && w.address != addr
                {
                    return false;
                }
                if let Some(addr) = owner_address
                    && w.owner_address != addr
                {
                    return false;
                }
                if let Some(addr) = jetton_address
                    && w.jetton_address != addr
                {
                    return false;
                }
                if exclude_zero_balance && w.balance == 0 {
                    return false;
                }
                true
            })
            .cloned()
            .collect();

        wallets.sort_by_key(|w| w.address);

        let start = offset.min(wallets.len());
        let end = (start + limit).min(wallets.len());

        Ok(wallets[start..end].to_vec())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn get_nft_items(
        &self,
        address: Option<Addr>,
        owner_address: Option<Addr>,
        collection_address: Option<Addr>,
        index: Option<String>,
        sort_by_last_transaction_lt: bool,
        limit: usize,
        offset: usize,
    ) -> anyhow::Result<Vec<NftItemMeta>> {
        let mut items: Vec<_> = self
            .history
            .nft_items
            .values()
            .filter(|item| {
                if let Some(addr) = address
                    && item.address != addr
                {
                    return false;
                }
                if let Some(addr) = owner_address
                    && item.owner_address != Some(addr)
                {
                    return false;
                }
                if let Some(addr) = collection_address
                    && item.collection_address != Some(addr)
                {
                    return false;
                }
                if let Some(expected_index) = &index
                    && &item.index != expected_index
                {
                    return false;
                }
                true
            })
            .cloned()
            .collect();

        if sort_by_last_transaction_lt {
            items.sort_by(|a, b| {
                b.last_transaction_lt
                    .cmp(&a.last_transaction_lt)
                    .then_with(|| a.address.cmp(&b.address))
            });
        } else {
            items.sort_by_key(|item| item.address);
        }

        let start = offset.min(items.len());
        let end = (start + limit).min(items.len());

        Ok(items[start..end].to_vec())
    }

    fn parse_addr_internal(&self, s: &str) -> Option<Addr> {
        let (int_addr, _) = StdAddr::from_str_ext(s, StdAddrFormat::any()).ok()?;
        Some(Addr {
            workchain: i32::from(int_addr.workchain),
            addr: int_addr.address.0,
        })
    }

    #[must_use]
    pub fn get_libraries(&self, hashes: &[Hash256]) -> Vec<GlobalLibraryLookup> {
        hashes
            .iter()
            .map(|hash| GlobalLibraryLookup {
                hash: *hash,
                entry: self.global_libraries.get(hash).cloned(),
            })
            .collect()
    }

    pub(crate) fn rebuild_global_libraries_from_accounts(&mut self) -> anyhow::Result<()> {
        self.global_libraries.clear();

        let mut accounts: Vec<_> = self.latest.accounts.iter().collect();
        accounts.sort_by_key(|(address, _)| **address);
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
                entry.publishers.insert(*address);
                entry.first_seen_lt = entry.first_seen_lt.min(lt);
                entry.last_seen_lt = entry.last_seen_lt.max(lt);
            }
        }

        self.vm_global_libs_dirty = true;
        self.vm_global_libs_boc = None;
        Ok(())
    }

    fn build_vm_global_libs_boc(&mut self) -> anyhow::Result<Option<BocBytes>> {
        if !self.vm_global_libs_dirty {
            return Ok(self.vm_global_libs_boc.clone());
        }

        let mut libs = tycho_types::dict::Dict::<HashBytes, LibDescr>::new();
        for (hash, entry) in &self.global_libraries {
            if entry.publishers.is_empty() {
                continue;
            }

            let lib_cell = Boc::decode(&entry.lib_boc).with_context(|| {
                format!("Failed to decode stored library BOC {}", hash.to_hex())
            })?;
            let actual_hash = Hash256(*lib_cell.repr_hash().as_array());
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

        self.vm_global_libs_boc = libs.into_root().map(|cell| Boc::encode(cell).into());
        self.vm_global_libs_dirty = false;
        Ok(self.vm_global_libs_boc.clone())
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
                        self.vm_global_libs_dirty = true;
                    }
                    if self
                        .global_libraries
                        .get(&hash)
                        .is_some_and(|entry| entry.publishers.is_empty())
                    {
                        self.global_libraries.remove(&hash);
                        self.vm_global_libs_dirty = true;
                    }
                }
                (None, Some(new_lib)) => {
                    let new_hash = Hash256(*new_lib.repr_hash().as_array());
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
                    let stored_hash = Hash256(*stored_cell.repr_hash().as_array());
                    if stored_hash != hash {
                        anyhow::bail!(
                            "Global library store is corrupted for {} (stored hash {})",
                            hash.to_hex(),
                            stored_hash.to_hex()
                        );
                    }

                    if entry.publishers.insert(*account) {
                        entry.last_seen_lt = lt;
                        self.vm_global_libs_dirty = true;
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn extract_public_libraries_from_shard_account(
        shard_account_boc: &BocBytes,
    ) -> anyhow::Result<HashMap<Hash256, tycho_types::cell::Cell>> {
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
            let key_hash = Hash256(key_hash.0);
            let root_hash = Hash256(*simple_lib.root.repr_hash().as_array());
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

    #[must_use]
    pub fn get_cell(&self, hash: &Hash256) -> Option<BocBytes> {
        self.cas.get(hash)
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
            .map(|tx| {
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
                }
            })
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
    pub fn find_block_by_lt(&self, lt: Lt) -> Option<BlockMeta> {
        self.history
            .blocks
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
    pub fn get_block_transactions(&self, block_meta: &BlockMeta) -> Option<Vec<TxMeta>> {
        let tx_hash = self.indexes.tx_by_block.get(&block_meta.seqno)?;
        let tx = self.history.tx_by_hash.get(tx_hash).cloned()?;
        Some(vec![tx])
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
        let in_msg = tx.in_msg_hash.and_then(|h| self.get_message_info(&h));
        let out_msgs = tx
            .out_msg_hashes
            .iter()
            .filter_map(|h| self.get_message_info(h))
            .collect();
        let tx_boc = self.get_cell(hash).unwrap_or_default();
        Some(TransactionInfo {
            meta: tx,
            in_msg,
            out_msgs,
            tx_boc,
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

    pub fn get_traces_by_message_hash(&self, msg_hash: &Hash256) -> anyhow::Result<TraceNode> {
        let tx_hash = self
            .find_trace_tx_hash_by_message_hash(msg_hash)
            .ok_or_else(|| anyhow::anyhow!("Trace not found for message {}", msg_hash.to_hex()))?;
        self.get_traces(&tx_hash)
    }

    fn find_trace_tx_hash_by_message_hash(&self, msg_hash: &Hash256) -> Option<Hash256> {
        self.history.tx_by_hash.values().find_map(|tx| {
            if tx.in_msg_hash == Some(*msg_hash) || tx.out_msg_hashes.contains(msg_hash) {
                return Some(tx.tx_hash);
            }

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
        Self::empty_shard_account_boc()
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
        let msg_hash = compute_boc_hash(&boc)?;
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
        };

        let exec_result = self.executor.execute(
            &shard_account_boc,
            &boc,
            &ctx,
            &config_boc,
            vm_global_libs.as_ref(),
        )?;

        let tx_hash = compute_boc_hash(&exec_result.tx_boc)?;
        let mut out_msg_hashes = Vec::new();
        let mut out_msgs = Vec::new();
        for out_boc in &exec_result.out_msgs_boc {
            let out_hash = compute_boc_hash(out_boc)?;
            out_msg_hashes.push(out_hash);
            let out_meta = parse_msg_meta(out_boc, out_hash)?;
            out_msgs.push(MessageInfo {
                meta: out_meta,
                boc: out_boc.clone(),
            });
        }

        let compute_exit_code = exec_result.compute_exit_code();
        let action_result_code = exec_result.action_result_code();
        let info = exec_result.tx.info.load().ok();
        let (storage_fees, other_fees) =
            if let Some(tycho_types::models::TxInfo::Ordinary(ord)) = info {
                let storage: u128 = ord
                    .storage_phase
                    .map_or(0, |p| p.storage_fees_collected.into());
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
            out_msg_hashes,
            block_seqno,
        };

        collect_code_data_cells(
            exec_result.new_account_boc.as_ref(),
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
                    SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as u32,
                    self.globals.head_seqno,
                ));
            }

            let block = self
                .get_block_header(seqno)
                .ok_or_else(|| anyhow::anyhow!("Block {seqno} not found"))?;
            return Ok((
                block.end_lt.saturating_add(self.globals.lt_step),
                block.gen_utime,
                seqno,
            ));
        }

        Ok((
            self.globals.global_lt.saturating_add(self.globals.lt_step),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as u32,
            self.globals.head_seqno,
        ))
    }

    fn empty_shard_account_boc() -> anyhow::Result<BocBytes> {
        let sa = ShardAccount {
            account: tycho_types::cell::Lazy::new(&tycho_types::models::OptionalAccount(None))?,
            last_trans_hash: HashBytes([0u8; 32]),
            last_trans_lt: 0,
        };
        let mut builder = CellBuilder::new();
        sa.store_into(&mut builder, tycho_types::cell::Cell::empty_context())?;
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
        self.latest.accounts.insert(*addr, meta);
        Ok(Some(boc))
    }

    #[must_use]
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

        let src_addr = IntAddr::Std(StdAddr::new(
            GIVER_ADDR
                .workchain
                .try_into()
                .map_err(|_| anyhow::anyhow!("Invalid giver workchain {}", GIVER_ADDR.workchain))?,
            HashBytes(GIVER_ADDR.addr),
        ));
        let dst_addr = IntAddr::Std(StdAddr::new(
            addr.workchain
                .try_into()
                .map_err(|_| anyhow::anyhow!("Invalid destination workchain {}", addr.workchain))?,
            HashBytes(addr.addr),
        ));

        let message_info = IntMsgInfo {
            ihr_disabled: true,
            bounce: false,
            bounced: false,
            src: src_addr,
            dst: dst_addr,
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

        let boc = BocRepr::encode(message)?;
        let hash = compute_boc_hash(&boc)?;
        self.cas.put(boc.clone().into(), hash);

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
        let hash = Hash256(*code.repr_hash().as_array());
        code_cells
            .entry(hash)
            .or_insert_with(|| Boc::encode(code).into());
    }

    if let Some(data) = state.data {
        let hash = Hash256(*data.repr_hash().as_array());
        data_cells
            .entry(hash)
            .or_insert_with(|| Boc::encode(data).into());
    }
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

const fn convert_addr(addr: &IntAddr) -> Addr {
    let mut bytes = [0u8; 32];
    let (workchain, address) = match addr {
        IntAddr::Std(std) => (std.workchain as i32, std.address.0),
        IntAddr::Var(var) => (var.workchain, {
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
    builder.store_u256(&HashBytes(tx_hash.0))?;
    let cell = builder.build()?;
    Ok(Boc::encode(cell).into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::{ExecContext, ExecResult, TvmExecutor};
    use crate::node::StateSource;
    use base64::Engine;
    use serde_json::json;
    use std::sync::{Arc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};
    use ton_executor::DEFAULT_CONFIG;
    use tycho_types::cell::{Cell, CellBuilder, Lazy, Store};
    use tycho_types::dict::Dict;
    use tycho_types::models::{
        Account, CurrencyCollection, IntAddr, OptionalAccount, SimpleLib, StateInit, StdAddr,
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

    #[derive(Clone)]
    struct RecordingExecutor {
        recorded_libs: Arc<Mutex<Vec<Option<BocBytes>>>>,
    }

    impl TvmExecutor for RecordingExecutor {
        fn execute(
            &self,
            _shard_account: &BocBytes,
            _in_msg: &BocBytes,
            _ctx: &ExecContext,
            _config: &BocBytes,
            libs: Option<&BocBytes>,
        ) -> anyhow::Result<ExecResult> {
            self.recorded_libs
                .lock()
                .expect("recorded libs mutex poisoned")
                .push(libs.cloned());
            anyhow::bail!("forced executor failure")
        }
    }

    fn make_test_node(executor: Box<dyn TvmExecutor>) -> Node {
        let config_bytes = base64::engine::general_purpose::STANDARD
            .decode(DEFAULT_CONFIG)
            .expect("must decode default config");
        Node::new(executor, config_bytes.into(), StateSource::Local).expect("must create test node")
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

        let config_bytes = base64::engine::general_purpose::STANDARD
            .decode(DEFAULT_CONFIG)
            .expect("must decode default config");
        let node = Node::with_db_path(
            Box::new(NoopExecutor),
            config_bytes.into(),
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

        let config_bytes = base64::engine::general_purpose::STANDARD
            .decode(DEFAULT_CONFIG)
            .expect("must decode default config");
        let mut node = Node::with_db_path(
            Box::new(NoopExecutor),
            config_bytes.clone().into(),
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
            config_bytes.into(),
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

    fn test_addr(byte: u8) -> Addr {
        Addr {
            workchain: 0,
            addr: [byte; 32],
        }
    }

    fn store_test_account_meta(
        node: &mut Node,
        boc: &BocBytes,
        status: AccountStatus,
    ) -> AccountMeta {
        let account_hash = compute_boc_hash(boc).expect("must hash shard account");
        node.cas.put(boc.clone(), account_hash);
        let cached_balance = if status == AccountStatus::Nonexist {
            0
        } else {
            1_000_000_000
        };
        AccountMeta {
            account_hash,
            status,
            cached_balance: Some(cached_balance),
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
        let state_init = StateInit {
            split_depth: None,
            special: None,
            code: None,
            data: None,
            libraries,
        };
        let account = Account {
            address: IntAddr::Std(StdAddr::new(addr.workchain as i8, HashBytes(addr.addr))),
            storage_stat: Default::default(),
            last_trans_lt: 0,
            balance: CurrencyCollection::new(1_000_000_000),
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

    fn single_library_lookup(node: &Node, hash: Hash256) -> GlobalLibraryLookup {
        let mut entries = node.get_libraries(&[hash]);
        assert_eq!(entries.len(), 1, "expected one lookup result");
        entries.remove(0)
    }

    fn found_library_entry(node: &Node, hash: Hash256) -> Option<GlobalLibraryEntry> {
        single_library_lookup(node, hash).entry
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

        let entry = found_library_entry(&node, Hash256(hash.0))
            .expect("public library must appear globally");
        assert!(
            entry.publishers.contains(&account_a),
            "publisher A must be tracked"
        );
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

        let entry = found_library_entry(&node, Hash256(hash.0))
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

        let entry = found_library_entry(&node, Hash256(hash.0))
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
            found_library_entry(&node, Hash256(hash.0)).is_none(),
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
                found_library_entry(&node, Hash256(hash.0)).is_none(),
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
            found_library_entry(&node, Hash256(hash.0)).is_none(),
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
            found_library_entry(&node, Hash256(hash.0)).is_some(),
            "final state with public library must be indexed"
        );
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
        let executor = RecordingExecutor {
            recorded_libs: Arc::clone(&recorded_libs),
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
            found_library_entry(&node, Hash256(hash.0)).is_none(),
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
            found_library_entry(&node, Hash256(hash.0)).is_some(),
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
            found_library_entry(&node, Hash256(hash_a.0)).is_some(),
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
        node.vm_global_libs_dirty = true;
        node.vm_global_libs_boc = None;

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
        let entries = node.get_libraries(&[missing, Hash256(hash_b.0), Hash256(hash_a.0)]);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].hash, missing);
        assert!(
            entries[0].entry.is_none(),
            "missing hash must be returned as not found"
        );
        assert_eq!(entries[1].hash, Hash256(hash_b.0));
        assert!(entries[1].entry.is_some());
        assert_eq!(entries[2].hash, Hash256(hash_a.0));
        assert!(entries[2].entry.is_some());
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
        let high_account_hash =
            compute_boc_hash(&high_boc).expect("must hash high-lt shard account");
        node.cas.put(high_boc, high_account_hash);

        let mut low_libs = Dict::<HashBytes, SimpleLib>::new();
        low_libs
            .set(lib_hash, lib)
            .expect("must insert low-lt library");
        let low_boc = make_active_shard_account_boc(account_low_lt, low_libs);
        let low_account_hash = compute_boc_hash(&low_boc).expect("must hash low-lt shard account");
        node.cas.put(low_boc, low_account_hash);

        node.latest.accounts.insert(
            account_high_lt,
            AccountMeta {
                account_hash: high_account_hash,
                status: AccountStatus::Active,
                cached_balance: Some(0),
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
                cached_balance: Some(0),
                last_trans_lt: Some(5),
                last_trans_hash: None,
                code_hash: None,
                data_hash: None,
                frozen_hash: None,
            },
        );

        node.rebuild_global_libraries_from_accounts()
            .expect("must rebuild global libraries from accounts");
        let entry = found_library_entry(&node, Hash256(lib_hash.0))
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
