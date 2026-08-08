use crate::node::{GIVER_ADDR, GIVER_BALANCE, Node, StateSource};
use crate::storage::{
    self, AccountDelta, AccountMeta, AccountStatus, BlockMeta, Globals, Indexes, JettonMasterMeta,
    MasterchainBlockMeta, MsgMeta, NftItemMeta, ReverseLtKey, TxMeta,
};
use crate::types::{Addr, BocBytes, Hash256, Lt, Seqno};
use crate::virtual_clock::VirtualClock;
use anyhow::Context;
use core::cmp;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::File;
use std::io::{BufReader, Write};
use std::path::Path;
use tycho_types::boc::Boc;
use tycho_types::cell::Cell;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct NodeStateSnapshot {
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
    pub history_jetton_masters: Vec<(Addr, JettonMasterMeta)>,
    pub history_jetton_wallets: Vec<(Addr, storage::JettonWalletMeta)>,
    #[serde(default)]
    pub history_nft_items: Vec<(Addr, NftItemMeta)>,
    #[serde(default)]
    pub history_asset_detection_checked: Vec<Addr>,
    pub cas_entries: Vec<(Hash256, BocBytes)>,
    pub pool_external: VecDeque<Hash256>,
    pub pool_internal: VecDeque<Hash256>,
    pub pool_rr_turn: bool,
    #[serde(default)]
    pub pending_freeze_current: VecDeque<Addr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SnapshotGlobals {
    pub origin_seqno: Seqno,
    #[serde(default)]
    pub origin_gen_utime: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fork_seqno: Option<Seqno>,
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

    pub fn dump_state_to_json(&self) -> anyhow::Result<Vec<u8>> {
        snapshot_to_json(&self.build_snapshot()?)
    }

    pub fn load_state_from_path<P: AsRef<Path>>(&mut self, path: P) -> anyhow::Result<()> {
        let snapshot = read_snapshot_from_path(path)?;
        self.apply_snapshot(snapshot)
    }

    pub fn load_state_from_json(&mut self, json: &[u8]) -> anyhow::Result<()> {
        self.apply_snapshot(snapshot_from_json(json)?)
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

        let cas_entries = self.export_cas_entries()?;

        let fork_seqno = match &self.state_source {
            StateSource::Local => None,
            StateSource::Remote(provider) => provider
                .fork_block_number
                .map(Seqno::try_from)
                .transpose()
                .context("Fork block seqno does not fit snapshot block numbering")?,
        };
        let snapshot = NodeStateSnapshot {
            globals: SnapshotGlobals {
                origin_seqno: self.globals.origin_seqno,
                origin_gen_utime: self.globals.origin_gen_utime,
                fork_seqno,
                head_seqno: self.globals.head_seqno,
                global_lt: self.globals.global_lt,
                lt_step: self.globals.lt_step,
                config_boc_hash: self.globals.config_boc_hash,
                queue_policy: self.globals.queue_policy,
                checkpoint_every: self.globals.checkpoint_every,
            },
            time_offset_seconds: self.clock.offset_seconds(),
            next_block_timestamp: self.clock.next_block_timestamp(),
            latest_accounts,
            history_blocks: self.history.blocks.clone(),
            history_masterchain_blocks: self.history.masterchain_blocks.clone(),
            history_deltas_by_seqno: self.history.deltas_by_seqno.clone(),
            history_tx_by_hash,
            history_msg_by_hash,
            history_msg_to_tx,
            history_jetton_masters,
            history_jetton_wallets,
            history_nft_items,
            history_asset_detection_checked,
            cas_entries,
            pool_external: self.pool.external.clone(),
            pool_internal: self.pool.internal.clone(),
            pool_rr_turn: self.pool.rr_turn,
            pending_freeze_current: self.pending_freeze_current.clone(),
        };
        Self::validate_snapshot(&snapshot)?;
        Ok(snapshot)
    }

    #[allow(clippy::significant_drop_tightening)]
    fn export_cas_entries(&self) -> anyhow::Result<Vec<(Hash256, BocBytes)>> {
        if let Some(persistence) = &self.persistence {
            persistence.export_cas_entries()
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

    pub(crate) fn apply_snapshot(&mut self, mut snapshot: NodeStateSnapshot) -> anyhow::Result<()> {
        for block in &mut snapshot.history_masterchain_blocks {
            if block.config_boc_hash == Hash256::default() {
                block.config_boc_hash = snapshot.globals.config_boc_hash;
            }
        }
        for (_, wallet) in &mut snapshot.history_jetton_wallets {
            if wallet.jetton_wallet_code_hash == Hash256::default() {
                wallet.jetton_wallet_code_hash = wallet.code_hash;
            }
        }

        let config_cell = Self::validate_snapshot(&snapshot)?;
        let mut clock =
            VirtualClock::from_parts(snapshot.time_offset_seconds, snapshot.next_block_timestamp);
        if let Some(latest_block) = snapshot.history_blocks.last() {
            clock.bump_offset_to_at_least(latest_block.gen_utime)?;
        }

        if let Some(persistence) = &self.persistence {
            persistence.replace_state(&snapshot)?;
        }

        let cas_by_hash = snapshot.cas_entries.into_iter().collect();

        if self.persistence.is_some() {
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
        self.history.jetton_masters = snapshot.history_jetton_masters.into_iter().collect();
        self.history.jetton_wallets = snapshot.history_jetton_wallets.into_iter().collect();
        self.history.nft_items = snapshot.history_nft_items.into_iter().collect();
        self.history.asset_detection_checked = snapshot
            .history_asset_detection_checked
            .into_iter()
            .collect();
        self.pool.external = snapshot.pool_external;
        self.pool.internal = snapshot.pool_internal;
        self.pool.rr_turn = snapshot.pool_rr_turn;
        self.pending_freeze_current = snapshot.pending_freeze_current;

        self.globals = Globals {
            origin_seqno: snapshot.globals.origin_seqno,
            origin_gen_utime: snapshot.globals.origin_gen_utime,
            head_seqno: snapshot.globals.head_seqno,
            global_lt: snapshot.globals.global_lt,
            lt_step: snapshot.globals.lt_step,
            config_boc_hash: snapshot.globals.config_boc_hash,
            queue_policy: snapshot.globals.queue_policy,
            checkpoint_every: snapshot.globals.checkpoint_every,
        };
        if let StateSource::Remote(provider) = &mut self.state_source {
            provider.fork_block_number = Some(u64::from(
                snapshot
                    .globals
                    .fork_seqno
                    .unwrap_or(self.globals.origin_seqno),
            ));
        }
        self.config_cell = config_cell;
        self.latest_masterchain_state = None;
        self.latest_shard_state = None;
        self.clock = clock;

        self.latest
            .accounts
            .entry(GIVER_ADDR)
            .or_insert_with(|| AccountMeta {
                account_hash: Hash256([0; 32]),
                status: AccountStatus::Active,
                balance: GIVER_BALANCE,
                extra_currencies: Vec::new(),
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

    pub(crate) fn validate_snapshot(snapshot: &NodeStateSnapshot) -> anyhow::Result<Cell> {
        anyhow::ensure!(
            snapshot.history_blocks.len() == snapshot.history_deltas_by_seqno.len(),
            "Block history length {} does not match account delta history length {}",
            snapshot.history_blocks.len(),
            snapshot.history_deltas_by_seqno.len()
        );
        anyhow::ensure!(
            snapshot.history_masterchain_blocks.is_empty()
                || snapshot.history_masterchain_blocks.len() == snapshot.history_blocks.len(),
            "Masterchain history length {} does not match basechain history length {}",
            snapshot.history_masterchain_blocks.len(),
            snapshot.history_blocks.len()
        );
        for (index, block) in snapshot.history_blocks.iter().enumerate() {
            let expected_seqno = snapshot
                .globals
                .origin_seqno
                .checked_add(index as Seqno + 1)
                .context("Block history seqno overflow")?;
            anyhow::ensure!(
                block.seqno == expected_seqno,
                "Block history is not contiguous at seqno {expected_seqno}"
            );
        }
        for (index, block) in snapshot.history_masterchain_blocks.iter().enumerate() {
            let expected_seqno = snapshot
                .globals
                .origin_seqno
                .checked_add(index as Seqno + 1)
                .context("Masterchain block history seqno overflow")?;
            anyhow::ensure!(
                block.seqno == expected_seqno,
                "Masterchain block history is not contiguous at seqno {expected_seqno}"
            );
        }
        let expected_head_seqno = snapshot
            .globals
            .origin_seqno
            .checked_add(
                snapshot
                    .history_blocks
                    .len()
                    .try_into()
                    .context("Block history length does not fit localnet block numbering")?,
            )
            .context("Block history head seqno overflow")?;
        anyhow::ensure!(
            snapshot.globals.head_seqno == expected_head_seqno,
            "Head seqno {} does not match origin {} and block history length {}",
            snapshot.globals.head_seqno,
            snapshot.globals.origin_seqno,
            snapshot.history_blocks.len()
        );

        let mut cas = HashMap::with_capacity(snapshot.cas_entries.len());
        for (hash, boc) in &snapshot.cas_entries {
            anyhow::ensure!(
                !cas.contains_key(hash),
                "Duplicate CAS entry {}",
                hash.to_hex()
            );
            let cell =
                Boc::decode(boc).with_context(|| format!("Invalid CAS entry {}", hash.to_hex()))?;
            let actual_hash = Hash256::from(cell.repr_hash());
            anyhow::ensure!(
                actual_hash == *hash,
                "CAS entry hash mismatch: expected {}, got {}",
                hash.to_hex(),
                actual_hash.to_hex()
            );
            Self::collect_library_refs(&cell)
                .with_context(|| format!("Invalid cell tree in CAS entry {}", hash.to_hex()))?;
            cas.insert(*hash, boc);
        }

        let config_boc = cas
            .get(&snapshot.globals.config_boc_hash)
            .context("Config missing from snapshot CAS")?;
        let config_cell = Boc::decode(config_boc).context("Invalid config BOC in snapshot CAS")?;

        let validate_account_meta = |address: &Addr, meta: &AccountMeta| -> anyhow::Result<()> {
            if meta.account_hash.is_zero() && *address == GIVER_ADDR {
                return Ok(());
            }
            let account_boc = cas.get(&meta.account_hash).with_context(|| {
                format!(
                    "Account {address} references missing CAS entry {}",
                    meta.account_hash.to_hex()
                )
            })?;
            Self::extract_public_libraries_from_shard_account(account_boc)
                .with_context(|| format!("Invalid shard account BOC for {address}"))?;
            Self::collect_code_library_refs_from_shard_account(account_boc)
                .with_context(|| format!("Invalid account code for {address}"))?;
            Ok(())
        };

        let mut account_addresses = HashSet::with_capacity(snapshot.latest_accounts.len());
        for (address, meta) in &snapshot.latest_accounts {
            anyhow::ensure!(
                account_addresses.insert(*address),
                "Duplicate latest account {address}"
            );
            validate_account_meta(address, meta)?;
        }
        for deltas in &snapshot.history_deltas_by_seqno {
            for delta in deltas {
                anyhow::ensure!(
                    delta.old_hash == delta.old_meta.as_ref().map(|meta| meta.account_hash),
                    "Account {} old delta hash does not match its metadata",
                    delta.addr
                );
                anyhow::ensure!(
                    delta.new_hash == delta.new_meta.as_ref().map(|meta| meta.account_hash),
                    "Account {} new delta hash does not match its metadata",
                    delta.addr
                );
                if let Some(meta) = &delta.old_meta {
                    validate_account_meta(&delta.addr, meta)?;
                }
                if let Some(meta) = &delta.new_meta {
                    validate_account_meta(&delta.addr, meta)?;
                }
            }
        }

        let mut transactions = HashMap::with_capacity(snapshot.history_tx_by_hash.len());
        for (hash, meta) in &snapshot.history_tx_by_hash {
            anyhow::ensure!(
                *hash == meta.tx_hash,
                "Transaction key does not match its hash"
            );
            anyhow::ensure!(
                transactions.insert(*hash, meta).is_none(),
                "Duplicate transaction {}",
                hash.to_hex()
            );
        }
        let mut listed_transactions = HashSet::with_capacity(transactions.len());
        for block in &snapshot.history_blocks {
            for tx_hash in &block.tx_hashes {
                anyhow::ensure!(
                    listed_transactions.insert(*tx_hash),
                    "Transaction {} is listed in more than one block",
                    tx_hash.to_hex()
                );
                let meta = transactions.get(tx_hash).with_context(|| {
                    format!(
                        "Block {} references missing transaction {}",
                        block.seqno,
                        tx_hash.to_hex()
                    )
                })?;
                anyhow::ensure!(
                    meta.block_seqno == block.seqno,
                    "Transaction {} points to block {}, but is listed in block {}",
                    tx_hash.to_hex(),
                    meta.block_seqno,
                    block.seqno
                );
            }
        }
        for tx_hash in transactions.keys() {
            anyhow::ensure!(
                listed_transactions.contains(tx_hash),
                "Transaction {} is not listed in its block",
                tx_hash.to_hex()
            );
        }

        let mut messages = HashMap::with_capacity(snapshot.history_msg_by_hash.len());
        for (hash, meta) in &snapshot.history_msg_by_hash {
            anyhow::ensure!(
                *hash == meta.msg_hash,
                "Message key does not match its hash"
            );
            anyhow::ensure!(
                messages.insert(*hash, meta).is_none(),
                "Duplicate message {}",
                hash.to_hex()
            );
            anyhow::ensure!(
                cas.contains_key(&meta.msg_boc_hash),
                "Message {} references missing CAS entry {}",
                hash.to_hex(),
                meta.msg_boc_hash.to_hex()
            );
        }
        let mut mapped_messages = HashSet::with_capacity(snapshot.history_msg_to_tx.len());
        for (message_hash, tx_hash) in &snapshot.history_msg_to_tx {
            anyhow::ensure!(
                mapped_messages.insert(*message_hash),
                "Duplicate message-to-transaction mapping for {}",
                message_hash.to_hex()
            );
            anyhow::ensure!(
                messages.contains_key(message_hash),
                "Message-to-transaction mapping references missing message {}",
                message_hash.to_hex()
            );
            anyhow::ensure!(
                transactions.contains_key(tx_hash),
                "Message-to-transaction mapping references missing transaction {}",
                tx_hash.to_hex()
            );
            anyhow::ensure!(
                transactions[tx_hash].in_msg_hash == Some(*message_hash),
                "Message-to-transaction mapping does not match transaction {} inbound message",
                tx_hash.to_hex()
            );
        }
        for (tx_hash, meta) in &transactions {
            if let Some(message_hash) = meta.in_msg_hash {
                anyhow::ensure!(
                    mapped_messages.contains(&message_hash),
                    "Transaction {} inbound message {} has no reverse mapping",
                    tx_hash.to_hex(),
                    message_hash.to_hex()
                );
            }
            for message_hash in &meta.out_msg_hashes {
                anyhow::ensure!(
                    messages.contains_key(message_hash),
                    "Transaction {} references missing outbound message {}",
                    tx_hash.to_hex(),
                    message_hash.to_hex()
                );
            }
        }
        for hash in snapshot
            .pool_external
            .iter()
            .chain(snapshot.pool_internal.iter())
        {
            let meta = messages
                .get(hash)
                .with_context(|| format!("Queued message {} has no metadata", hash.to_hex()))?;
            anyhow::ensure!(
                cas.contains_key(&meta.msg_boc_hash),
                "Queued message {} references missing CAS entry {}",
                hash.to_hex(),
                meta.msg_boc_hash.to_hex()
            );
        }

        Ok(config_cell)
    }

    fn rebuild_indexes(&mut self) {
        self.indexes = Indexes::new();
        for (index, deltas) in self.history.deltas_by_seqno.iter().enumerate() {
            let seqno = self.history.blocks[index].seqno;
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

pub(crate) fn snapshot_from_json(json: &[u8]) -> anyhow::Result<NodeStateSnapshot> {
    Ok(serde_json::from_slice(json)?)
}

pub(crate) fn snapshot_to_json(snapshot: &NodeStateSnapshot) -> anyhow::Result<Vec<u8>> {
    Ok(serde_json::to_vec(snapshot)?)
}

pub(crate) fn write_snapshot_to_path<P: AsRef<Path>>(
    snapshot: &NodeStateSnapshot,
    path: P,
) -> anyhow::Result<()> {
    let path = path.as_ref();
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;

    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer(temp.as_file_mut(), snapshot)?;
    temp.as_file_mut().flush()?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|error| error.error)?;
    Ok(())
}
