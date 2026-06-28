use crate::node::{GIVER_ADDR, GIVER_BALANCE, Node};
use crate::storage::{
    self, AccountDelta, AccountMeta, AccountStatus, BlockMeta, Globals, Indexes, JettonMasterMeta,
    MasterchainBlockMeta, MsgMeta, NftItemMeta, ReverseLtKey, TxMeta,
};
use crate::types::{Addr, BocBytes, Hash256, Lt, Seqno};
use anyhow::Context;
use core::cmp;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

const NODE_STATE_SNAPSHOT_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct NodeStateSnapshot {
    pub version: u32,
    pub globals: SnapshotGlobals,
    pub time_offset_seconds: i64,
    pub next_block_timestamp: Option<u32>,
    pub latest_accounts: Vec<(Addr, AccountMeta)>,
    pub history_blocks: Vec<BlockMeta>,
    #[serde(default)]
    pub history_masterchain_blocks: Vec<MasterchainBlockMeta>,
    pub history_deltas_by_seqno: Vec<Vec<AccountDelta>>,
    pub history_tx_by_hash: Vec<(Hash256, TxMeta)>,
    pub history_msg_by_hash: Vec<(Hash256, MsgMeta)>,
    pub history_msg_to_tx: Vec<(Hash256, Hash256)>,
    pub history_address_names: Vec<(Addr, String)>,
    pub history_jetton_masters: Vec<(Addr, JettonMasterMeta)>,
    pub history_jetton_wallets: Vec<(Addr, storage::JettonWalletMeta)>,
    #[serde(default)]
    pub history_nft_items: Vec<(Addr, NftItemMeta)>,
    #[serde(default)]
    pub history_asset_detection_checked: Vec<Addr>,
    #[serde(default)]
    pub history_compiler_abis: Vec<(Hash256, Value)>,
    #[serde(default)]
    pub history_verified_sources: Vec<(Hash256, Value)>,
    pub cas_entries: Vec<(Hash256, BocBytes)>,
    pub pool_external: VecDeque<Hash256>,
    pub pool_internal: VecDeque<Hash256>,
    pub pool_rr_turn: bool,
    #[serde(default)]
    pub pending_freeze_current: VecDeque<Addr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SnapshotGlobals {
    pub head_seqno: Seqno,
    pub global_lt: Lt,
    pub lt_step: Lt,
    pub config_boc_hash: Hash256,
    pub queue_policy: storage::QueuePolicy,
    pub checkpoint_every: u32,
}

impl Node {
    pub fn dump_state_to_path<P: AsRef<Path>>(&self, path: P) -> anyhow::Result<()> {
        let snapshot = self.build_snapshot()?;
        write_snapshot_to_path(&snapshot, path)
    }

    pub fn load_state_from_path<P: AsRef<Path>>(&mut self, path: P) -> anyhow::Result<()> {
        let snapshot = read_snapshot_from_path(path)?;
        self.apply_snapshot(snapshot)
    }

    pub(crate) fn build_snapshot(&self) -> anyhow::Result<NodeStateSnapshot> {
        let mut latest_accounts = self
            .latest
            .accounts
            .iter()
            .map(|(addr, meta)| (*addr, meta.clone()))
            .collect::<Vec<_>>();
        latest_accounts.sort_by_key(|(addr, _)| *addr);

        let mut history_tx_by_hash = self
            .history
            .tx_by_hash
            .iter()
            .map(|(hash, tx)| (*hash, tx.clone()))
            .collect::<Vec<_>>();
        history_tx_by_hash.sort_by_key(|(hash, _)| *hash);

        let mut history_msg_by_hash = self
            .history
            .msg_by_hash
            .iter()
            .map(|(hash, msg)| (*hash, msg.clone()))
            .collect::<Vec<_>>();
        history_msg_by_hash.sort_by_key(|(hash, _)| *hash);

        let mut history_msg_to_tx = self
            .history
            .msg_to_tx
            .iter()
            .map(|(msg, tx)| (*msg, *tx))
            .collect::<Vec<_>>();
        history_msg_to_tx.sort_by_key(|(msg, _)| *msg);

        let mut history_address_names = self
            .history
            .address_names
            .iter()
            .map(|(addr, name)| (*addr, name.clone()))
            .collect::<Vec<_>>();
        history_address_names.sort_by_key(|(addr, _)| *addr);

        let history_jetton_masters = self
            .history
            .jetton_masters
            .iter()
            .map(|(addr, meta)| (*addr, meta.clone()))
            .collect::<Vec<_>>();

        let history_jetton_wallets = self
            .history
            .jetton_wallets
            .iter()
            .map(|(addr, meta)| (*addr, meta.clone()))
            .collect::<Vec<_>>();

        let history_nft_items = self
            .history
            .nft_items
            .iter()
            .map(|(addr, meta)| (*addr, meta.clone()))
            .collect::<Vec<_>>();

        let mut history_asset_detection_checked = self
            .history
            .asset_detection_checked
            .iter()
            .copied()
            .collect::<Vec<_>>();
        history_asset_detection_checked.sort();

        let mut history_compiler_abis = self
            .history
            .compiler_abis
            .iter()
            .map(|(hash, abi)| (*hash, abi.clone()))
            .collect::<Vec<_>>();
        history_compiler_abis.sort_by_key(|(hash, _)| *hash);

        let mut history_verified_sources = self
            .history
            .verified_sources
            .iter()
            .map(|(hash, source)| (*hash, source.clone()))
            .collect::<Vec<_>>();
        history_verified_sources.sort_by_key(|(hash, _)| *hash);

        let cas_entries = self.export_cas_entries()?;

        Ok(NodeStateSnapshot {
            version: NODE_STATE_SNAPSHOT_VERSION,
            globals: SnapshotGlobals {
                head_seqno: self.globals.head_seqno,
                global_lt: self.globals.global_lt,
                lt_step: self.globals.lt_step,
                config_boc_hash: self.globals.config_boc_hash,
                queue_policy: self.globals.queue_policy,
                checkpoint_every: self.globals.checkpoint_every,
            },
            time_offset_seconds: self.time_offset_seconds,
            next_block_timestamp: self.next_block_timestamp,
            latest_accounts,
            history_blocks: self.history.blocks.clone(),
            history_masterchain_blocks: self.history.masterchain_blocks.clone(),
            history_deltas_by_seqno: self.history.deltas_by_seqno.clone(),
            history_tx_by_hash,
            history_msg_by_hash,
            history_msg_to_tx,
            history_address_names,
            history_jetton_masters,
            history_jetton_wallets,
            history_nft_items,
            history_asset_detection_checked,
            history_compiler_abis,
            history_verified_sources,
            cas_entries,
            pool_external: self.pool.external.clone(),
            pool_internal: self.pool.internal.clone(),
            pool_rr_turn: self.pool.rr_turn,
            pending_freeze_current: self.pending_freeze_current.clone(),
        })
    }

    #[allow(clippy::significant_drop_tightening)]
    fn export_cas_entries(&self) -> anyhow::Result<Vec<(Hash256, BocBytes)>> {
        if let Some(conn) = &self.conn {
            let conn = conn.lock().expect("Failed to lock DB connection");
            let mut stmt = conn.prepare("SELECT hash, boc FROM cas")?;
            let iter = stmt.query_map([], |row| {
                let hash_bytes: Vec<u8> = row.get(0)?;
                let boc: Vec<u8> = row.get(1)?;
                Ok((hash_bytes, boc))
            })?;

            let mut entries = Vec::new();
            for row in iter {
                let (hash_bytes, boc) = row?;
                if hash_bytes.len() != 32 {
                    anyhow::bail!("Invalid hash length in cas table: {}", hash_bytes.len());
                }

                let mut hash = [0u8; 32];
                hash.copy_from_slice(&hash_bytes);
                entries.push((Hash256(hash), boc.into()));
            }
            entries.sort_by_key(|(hash, _)| *hash);
            Ok(entries)
        } else {
            let mut entries = self
                .cas
                .boc_by_hash
                .iter()
                .map(|(hash, boc)| (*hash, boc.clone()))
                .collect::<Vec<_>>();
            entries.sort_by_key(|(hash, _)| *hash);
            Ok(entries)
        }
    }

    pub(crate) fn apply_snapshot(&mut self, snapshot: NodeStateSnapshot) -> anyhow::Result<()> {
        if snapshot.version != NODE_STATE_SNAPSHOT_VERSION {
            anyhow::bail!("Unsupported snapshot version: {}", snapshot.version);
        }

        self.replace_persistent_state(&snapshot)?;

        let cas_by_hash = snapshot
            .cas_entries
            .into_iter()
            .collect::<FxHashMap<Hash256, BocBytes>>();

        if self.conn.is_some() {
            self.cas.boc_by_hash.clear();
        } else {
            self.cas.boc_by_hash = cas_by_hash;
        }
        self.cas.clear_cell_cache();

        self.latest.accounts = snapshot.latest_accounts.into_iter().collect();
        self.history.blocks = snapshot.history_blocks;
        self.history.masterchain_blocks = snapshot.history_masterchain_blocks;
        self.history.deltas_by_seqno = snapshot.history_deltas_by_seqno;
        self.history.tx_by_hash = snapshot.history_tx_by_hash.into_iter().collect();
        self.history.msg_by_hash = snapshot.history_msg_by_hash.into_iter().collect();
        self.history.msg_to_tx = snapshot.history_msg_to_tx.into_iter().collect();
        self.history.address_names = snapshot.history_address_names.into_iter().collect();
        self.history.jetton_masters = snapshot.history_jetton_masters.into_iter().collect();
        self.history.jetton_wallets = snapshot.history_jetton_wallets.into_iter().collect();
        self.history.nft_items = snapshot.history_nft_items.into_iter().collect();
        self.history.asset_detection_checked = snapshot
            .history_asset_detection_checked
            .into_iter()
            .collect();
        self.history.compiler_abis = snapshot.history_compiler_abis.into_iter().collect();
        self.history.verified_sources = snapshot.history_verified_sources.into_iter().collect();

        self.pool.external = snapshot.pool_external;
        self.pool.internal = snapshot.pool_internal;
        self.pool.rr_turn = snapshot.pool_rr_turn;
        self.pending_freeze_current = snapshot.pending_freeze_current;

        self.globals = Globals {
            head_seqno: snapshot.globals.head_seqno,
            global_lt: snapshot.globals.global_lt,
            lt_step: snapshot.globals.lt_step,
            config_boc_hash: snapshot.globals.config_boc_hash,
            queue_policy: snapshot.globals.queue_policy,
            checkpoint_every: snapshot.globals.checkpoint_every,
        };
        self.config_cell = self
            .cas
            .get_cell(&self.globals.config_boc_hash)
            .context("Config missing")?;
        self.latest_masterchain_state = None;
        self.latest_shard_state = None;
        self.time_offset_seconds = snapshot.time_offset_seconds;
        self.next_block_timestamp = snapshot.next_block_timestamp;
        if let Some(latest_block) = self.history.blocks.last() {
            self.bump_offset_to_at_least(latest_block.gen_utime)?;
        }

        self.latest
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

        self.rebuild_indexes();
        self.rebuild_global_libraries_from_accounts()?;
        Ok(())
    }

    #[allow(clippy::significant_drop_tightening)]
    fn replace_persistent_state(&self, snapshot: &NodeStateSnapshot) -> anyhow::Result<()> {
        let Some(conn) = &self.conn else {
            return Ok(());
        };

        {
            let mut conn = conn.lock().expect("Failed to lock DB connection");
            let tx = conn.transaction()?;

            tx.execute("DELETE FROM cas", [])?;
            tx.execute("DELETE FROM blocks", [])?;
            tx.execute("DELETE FROM masterchain_blocks", [])?;
            tx.execute("DELETE FROM transactions", [])?;
            tx.execute("DELETE FROM messages", [])?;
            tx.execute("DELETE FROM accounts", [])?;
            tx.execute("DELETE FROM compiler_abis", [])?;
            tx.execute("DELETE FROM verified_sources", [])?;

            for (hash, boc) in &snapshot.cas_entries {
                tx.execute(
                    "INSERT OR REPLACE INTO cas (hash, boc) VALUES (?1, ?2)",
                    rusqlite::params![hash.0.to_vec(), boc],
                )?;
            }

            for block in &snapshot.history_blocks {
                let block_data = serde_json::to_vec(block)?;
                tx.execute(
                    "INSERT OR REPLACE INTO blocks (seqno, data) VALUES (?1, ?2)",
                    rusqlite::params![block.seqno, block_data],
                )?;
            }

            for block in &snapshot.history_masterchain_blocks {
                let block_data = serde_json::to_vec(block)?;
                tx.execute(
                    "INSERT OR REPLACE INTO masterchain_blocks (seqno, data) VALUES (?1, ?2)",
                    rusqlite::params![block.seqno, block_data],
                )?;
            }

            for (hash, tx_meta) in &snapshot.history_tx_by_hash {
                let tx_data = serde_json::to_vec(tx_meta)?;
                tx.execute(
                    "INSERT OR REPLACE INTO transactions (hash, data, account, lt, seqno) VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![
                        hash.0.to_vec(),
                        tx_data,
                        tx_meta.account.addr.to_vec(),
                        tx_meta.lt,
                        tx_meta.block_seqno,
                    ],
                )?;
            }

            for (hash, msg_meta) in &snapshot.history_msg_by_hash {
                let msg_data = serde_json::to_vec(msg_meta)?;
                tx.execute(
                    "INSERT OR REPLACE INTO messages (hash, data) VALUES (?1, ?2)",
                    rusqlite::params![hash.0.to_vec(), msg_data],
                )?;
            }

            for (address, account_meta) in &snapshot.latest_accounts {
                let account_data = serde_json::to_vec(account_meta)?;
                tx.execute(
                    "INSERT OR REPLACE INTO accounts (address, data) VALUES (?1, ?2)",
                    rusqlite::params![address.addr.to_vec(), account_data],
                )?;
            }

            for (code_hash, compiler_abi) in &snapshot.history_compiler_abis {
                let data = serde_json::to_vec(compiler_abi)?;
                tx.execute(
                    "INSERT OR REPLACE INTO compiler_abis (code_hash, data) VALUES (?1, ?2)",
                    rusqlite::params![code_hash.0.to_vec(), data],
                )?;
            }

            for (code_hash, source) in &snapshot.history_verified_sources {
                let data = serde_json::to_vec(source)?;
                tx.execute(
                    "INSERT OR REPLACE INTO verified_sources (code_hash, data) VALUES (?1, ?2)",
                    rusqlite::params![code_hash.0.to_vec(), data],
                )?;
            }

            tx.commit()?;
        }
        Ok(())
    }

    fn rebuild_indexes(&mut self) {
        self.indexes = Indexes::new();
        for (index, deltas) in self.history.deltas_by_seqno.iter().enumerate() {
            let seqno = index as Seqno + 1;
            for delta in deltas {
                self.indexes
                    .account_deltas_by_addr
                    .entry(delta.addr)
                    .or_default()
                    .insert(seqno, delta.clone());
            }
        }

        for tx_meta in self.history.tx_by_hash.values() {
            let key = ReverseLtKey(cmp::Reverse(tx_meta.lt), tx_meta.tx_hash);
            self.indexes
                .tx_by_account
                .entry(tx_meta.account)
                .or_default()
                .insert(key, tx_meta.tx_hash);
            for out_msg_hash in &tx_meta.out_msg_hashes {
                self.indexes
                    .tx_by_out_msg
                    .insert(*out_msg_hash, tx_meta.tx_hash);
            }
        }

        for block in &self.history.blocks {
            self.indexes
                .tx_by_block
                .insert(block.seqno, block.tx_hashes.clone());
        }
    }
}

pub(crate) fn read_snapshot_from_path<P: AsRef<Path>>(
    path: P,
) -> anyhow::Result<NodeStateSnapshot> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let snapshot = serde_json::from_reader(reader)?;
    Ok(snapshot)
}

pub(crate) fn write_snapshot_to_path<P: AsRef<Path>>(
    snapshot: &NodeStateSnapshot,
    path: P,
) -> anyhow::Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }

    let file = File::create(path)?;
    let writer = BufWriter::new(file);
    serde_json::to_writer(writer, snapshot)?;
    Ok(())
}
