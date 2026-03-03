use crate::node::{GIVER_ADDR, GIVER_BALANCE, Node};
use crate::storage::{
    self, AccountDelta, AccountMeta, AccountStatus, BlockMeta, Globals, Indexes, JettonMasterMeta,
    MsgMeta, ReverseLtKey, TxMeta,
};
use crate::types::{Addr, BocBytes, Hash256, Lt, Seqno};
use core::cmp;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct NodeStateSnapshot {
    pub version: u32,
    pub globals: SnapshotGlobals,
    pub latest_accounts: Vec<(Addr, AccountMeta)>,
    pub history_blocks: Vec<BlockMeta>,
    pub history_deltas_by_seqno: Vec<Vec<AccountDelta>>,
    pub history_tx_by_hash: Vec<(Hash256, TxMeta)>,
    pub history_msg_by_hash: Vec<(Hash256, MsgMeta)>,
    pub history_msg_to_tx: Vec<(Hash256, Hash256)>,
    pub history_address_names: Vec<(Addr, String)>,
    pub history_jetton_masters: Vec<(Addr, JettonMasterMeta)>,
    pub history_jetton_wallets: Vec<(Addr, storage::JettonWalletMeta)>,
    pub cas_entries: Vec<(Hash256, BocBytes)>,
    pub pool_external: VecDeque<Hash256>,
    pub pool_internal: VecDeque<Hash256>,
    pub pool_rr_turn: bool,
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
        let path = path.as_ref();
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }

        let snapshot = self.build_snapshot()?;
        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer(writer, &snapshot)?;
        Ok(())
    }

    pub fn load_state_from_path<P: AsRef<Path>>(&mut self, path: P) -> anyhow::Result<()> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let snapshot: NodeStateSnapshot = serde_json::from_reader(reader)?;
        self.apply_snapshot(snapshot)
    }

    fn build_snapshot(&self) -> anyhow::Result<NodeStateSnapshot> {
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

        let mut history_jetton_masters = self
            .history
            .jetton_masters
            .iter()
            .map(|(addr, meta)| (*addr, meta.clone()))
            .collect::<Vec<_>>();
        history_jetton_masters.sort_by_key(|(addr, _)| *addr);

        let mut history_jetton_wallets = self
            .history
            .jetton_wallets
            .iter()
            .map(|(addr, meta)| (*addr, meta.clone()))
            .collect::<Vec<_>>();
        history_jetton_wallets.sort_by_key(|(addr, _)| *addr);

        let cas_entries = self.export_cas_entries()?;

        Ok(NodeStateSnapshot {
            version: 1,
            globals: SnapshotGlobals {
                head_seqno: self.globals.head_seqno,
                global_lt: self.globals.global_lt,
                lt_step: self.globals.lt_step,
                config_boc_hash: self.globals.config_boc_hash,
                queue_policy: self.globals.queue_policy,
                checkpoint_every: self.globals.checkpoint_every,
            },
            latest_accounts,
            history_blocks: self.history.blocks.clone(),
            history_deltas_by_seqno: self.history.deltas_by_seqno.clone(),
            history_tx_by_hash,
            history_msg_by_hash,
            history_msg_to_tx,
            history_address_names,
            history_jetton_masters,
            history_jetton_wallets,
            cas_entries,
            pool_external: self.pool.external.clone(),
            pool_internal: self.pool.internal.clone(),
            pool_rr_turn: self.pool.rr_turn,
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

    fn apply_snapshot(&mut self, snapshot: NodeStateSnapshot) -> anyhow::Result<()> {
        if snapshot.version != 1 {
            anyhow::bail!("Unsupported snapshot version: {}", snapshot.version);
        }

        let cas_by_hash = snapshot
            .cas_entries
            .into_iter()
            .collect::<HashMap<Hash256, BocBytes>>();

        if self.conn.is_some() {
            self.cas.boc_by_hash.clear();
        } else {
            self.cas.boc_by_hash = cas_by_hash;
        }

        self.latest.accounts = snapshot.latest_accounts.into_iter().collect();
        self.history.blocks = snapshot.history_blocks;
        self.history.deltas_by_seqno = snapshot.history_deltas_by_seqno;
        self.history.tx_by_hash = snapshot.history_tx_by_hash.into_iter().collect();
        self.history.msg_by_hash = snapshot.history_msg_by_hash.into_iter().collect();
        self.history.msg_to_tx = snapshot.history_msg_to_tx.into_iter().collect();
        self.history.address_names = snapshot.history_address_names.into_iter().collect();
        self.history.jetton_masters = snapshot.history_jetton_masters.into_iter().collect();
        self.history.jetton_wallets = snapshot.history_jetton_wallets.into_iter().collect();

        self.pool.external = snapshot.pool_external;
        self.pool.internal = snapshot.pool_internal;
        self.pool.rr_turn = snapshot.pool_rr_turn;

        self.globals = Globals {
            head_seqno: snapshot.globals.head_seqno,
            global_lt: snapshot.globals.global_lt,
            lt_step: snapshot.globals.lt_step,
            config_boc_hash: snapshot.globals.config_boc_hash,
            queue_policy: snapshot.globals.queue_policy,
            checkpoint_every: snapshot.globals.checkpoint_every,
        };

        self.latest
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

        self.rebuild_indexes();
        self.rebuild_global_libraries_from_accounts()?;
        Ok(())
    }

    fn rebuild_indexes(&mut self) {
        self.indexes = Indexes::new();
        for tx_meta in self.history.tx_by_hash.values() {
            let key = ReverseLtKey(cmp::Reverse(tx_meta.lt), tx_meta.tx_hash);
            self.indexes
                .tx_by_account
                .entry(tx_meta.account)
                .or_default()
                .insert(key, tx_meta.tx_hash);
            self.indexes
                .tx_by_block
                .insert(tx_meta.block_seqno, tx_meta.tx_hash);
        }
    }
}
