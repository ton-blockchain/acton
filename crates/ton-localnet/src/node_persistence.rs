use crate::node::GIVER_ADDR;
use crate::node_snapshot::NodeStateSnapshot;
use crate::storage::{
    AccountDelta, AccountMeta, BlockMeta, History, Indexes, LatestState, MasterchainBlockMeta,
    MsgMeta, PendingCommit, ReverseLtKey, TxMeta,
};
use crate::types::{Addr, BocBytes, Hash256, Seqno};
use anyhow::Context;
use core::cmp;
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use std::sync::{Arc, Mutex};

pub(crate) struct NodePersistence {
    conn: Arc<Mutex<Connection>>,
}

pub(crate) struct PersistedNodeState {
    pub latest: LatestState,
    pub history: History,
    pub indexes: Indexes,
    pub origin_seqno: Option<Seqno>,
    pub fork_seqno: Option<Seqno>,
    pub head_seqno: Seqno,
}

impl NodePersistence {
    pub(crate) fn open<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(path)?;
        init_schema(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub(crate) fn connection(&self) -> Arc<Mutex<Connection>> {
        Arc::clone(&self.conn)
    }

    #[allow(clippy::significant_drop_tightening)]
    pub(crate) fn load(&self) -> anyhow::Result<PersistedNodeState> {
        let mut history = History::new();
        let mut latest = LatestState::new();
        let mut indexes = Indexes::new();
        let mut head_seqno = 0;

        let conn = self.conn.lock().expect("Failed to lock DB connection");
        let origin_seqno = conn
            .query_row(
                "SELECT value FROM node_metadata WHERE key = 'origin_seqno'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        let fork_seqno = conn
            .query_row(
                "SELECT value FROM node_metadata WHERE key = 'fork_seqno'",
                [],
                |row| row.get(0),
            )
            .optional()?;

        let mut stmt = conn.prepare("SELECT data FROM blocks ORDER BY seqno ASC")?;
        let block_iter = stmt.query_map([], |row| {
            let data: Vec<u8> = row.get(0)?;
            serde_json::from_slice::<BlockMeta>(&data)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
        })?;
        for block in block_iter {
            let block = block?;
            if let Some(previous) = history.blocks.last() {
                let expected_seqno = previous
                    .seqno
                    .checked_add(1)
                    .context("SQLite block history seqno overflow")?;
                anyhow::ensure!(
                    block.seqno == expected_seqno,
                    "SQLite block history is not contiguous at seqno {expected_seqno}"
                );
            } else {
                anyhow::ensure!(
                    block.seqno > 0,
                    "SQLite block history cannot start at seqno zero"
                );
            }
            head_seqno = block.seqno;
            history.blocks.push(block);
        }

        let mut stmt = conn.prepare("SELECT data FROM masterchain_blocks ORDER BY seqno ASC")?;
        let block_iter = stmt.query_map([], |row| {
            let data: Vec<u8> = row.get(0)?;
            serde_json::from_slice::<MasterchainBlockMeta>(&data)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
        })?;
        for block in block_iter {
            history.masterchain_blocks.push(block?);
        }
        anyhow::ensure!(
            history.masterchain_blocks.is_empty()
                || history.masterchain_blocks.len() == history.blocks.len(),
            "SQLite masterchain history length does not match basechain history"
        );
        for (masterchain, basechain) in history.masterchain_blocks.iter().zip(&history.blocks) {
            anyhow::ensure!(
                masterchain.seqno == basechain.seqno,
                "SQLite masterchain block {} does not match basechain block {}",
                masterchain.seqno,
                basechain.seqno
            );
        }

        let mut stmt = conn.prepare("SELECT hash, data, account, lt, seqno FROM transactions")?;
        let tx_iter = stmt.query_map([], |row| {
            let hash: Hash256 = row.get(0)?;
            let data: Vec<u8> = row.get(1)?;
            let account: Addr = row.get(2)?;
            let lt: u64 = row.get(3)?;
            let seqno: u32 = row.get(4)?;
            let tx_meta = serde_json::from_slice::<TxMeta>(&data)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

            Ok((hash, tx_meta, account, lt, seqno))
        })?;
        for tx in tx_iter {
            let (hash, tx_meta, addr, lt, seqno) = tx?;
            anyhow::ensure!(hash == tx_meta.tx_hash, "SQLite transaction hash mismatch");
            anyhow::ensure!(
                addr == tx_meta.account,
                "SQLite transaction account mismatch for {}",
                hash.to_hex()
            );
            anyhow::ensure!(
                lt == tx_meta.lt && seqno == tx_meta.block_seqno,
                "SQLite transaction position mismatch for {}",
                hash.to_hex()
            );
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

        history
            .deltas_by_seqno
            .resize(history.blocks.len(), Vec::new());
        let mut stmt = conn.prepare("SELECT seqno, data FROM account_deltas ORDER BY seqno ASC")?;
        let delta_iter = stmt.query_map([], |row| {
            let seqno: Seqno = row.get(0)?;
            let data: Vec<u8> = row.get(1)?;
            let deltas = serde_json::from_slice::<Vec<AccountDelta>>(&data)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            Ok((seqno, deltas))
        })?;
        for row in delta_iter {
            let (seqno, deltas) = row?;
            let first_seqno = history
                .blocks
                .first()
                .context("SQLite account deltas exist without block history")?
                .seqno;
            let index = seqno
                .checked_sub(first_seqno)
                .and_then(|index| usize::try_from(index).ok())
                .filter(|index| *index < history.deltas_by_seqno.len())
                .with_context(|| {
                    format!("SQLite account deltas reference unknown block {seqno}")
                })?;
            anyhow::ensure!(
                history.blocks[index].seqno == seqno,
                "SQLite account deltas are not aligned with block {seqno}"
            );
            for delta in &deltas {
                indexes
                    .account_deltas_by_addr
                    .entry(delta.addr)
                    .or_default()
                    .insert(seqno, delta.clone());
            }
            history.deltas_by_seqno[index] = deltas;
        }

        let mut stmt = conn.prepare("SELECT address, data FROM accounts")?;
        let acc_iter = stmt.query_map([], |row| {
            let addr: Addr = row.get(0)?;
            let data: Vec<u8> = row.get(1)?;
            let meta = serde_json::from_slice::<AccountMeta>(&data)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            Ok((addr, meta))
        })?;
        for acc in acc_iter {
            let (addr, meta) = acc?;
            latest.accounts.insert(addr, meta);
        }

        let mut stmt = conn.prepare("SELECT hash, data FROM messages")?;
        let msg_iter = stmt.query_map([], |row| {
            let hash: Hash256 = row.get(0)?;
            let data: Vec<u8> = row.get(1)?;
            let meta = serde_json::from_slice::<MsgMeta>(&data)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            Ok((hash, meta))
        })?;
        for msg in msg_iter {
            let (hash, meta) = msg?;
            anyhow::ensure!(hash == meta.msg_hash, "SQLite message hash mismatch");
            history.msg_by_hash.insert(hash, meta);
        }

        Ok(PersistedNodeState {
            latest,
            history,
            indexes,
            origin_seqno,
            fork_seqno,
            head_seqno,
        })
    }

    pub(crate) fn set_chain_origins(
        &self,
        origin_seqno: Seqno,
        fork_seqno: Option<Seqno>,
    ) -> anyhow::Result<()> {
        let mut conn = self.conn.lock().expect("Failed to lock DB connection");
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT OR REPLACE INTO node_metadata (key, value) VALUES ('origin_seqno', ?1)",
            params![origin_seqno],
        )?;
        if let Some(fork_seqno) = fork_seqno {
            tx.execute(
                "INSERT OR REPLACE INTO node_metadata (key, value) VALUES ('fork_seqno', ?1)",
                params![fork_seqno],
            )?;
        } else {
            tx.execute("DELETE FROM node_metadata WHERE key = 'fork_seqno'", [])?;
        }
        tx.commit()?;
        drop(conn);
        Ok(())
    }

    pub(crate) fn persist_commit(
        &self,
        pending: &PendingCommit,
        history: &History,
        latest: &LatestState,
    ) -> anyhow::Result<()> {
        let mut conn = self.conn.lock().expect("Failed to lock DB connection");
        let tx = conn.transaction()?;

        let block_data = serde_json::to_vec(&pending.block_meta)?;
        tx.execute(
            "INSERT OR REPLACE INTO blocks (seqno, data) VALUES (?1, ?2)",
            params![pending.block_meta.seqno, block_data],
        )?;
        if let Some(masterchain_block_meta) = &pending.masterchain_block_meta {
            let block_data = serde_json::to_vec(masterchain_block_meta)?;
            tx.execute(
                "INSERT OR REPLACE INTO masterchain_blocks (seqno, data) VALUES (?1, ?2)",
                params![masterchain_block_meta.seqno, block_data],
            )?;
        }

        for tx_meta in &pending.tx_metas {
            let tx_data = serde_json::to_vec(tx_meta)?;
            tx.execute(
                "INSERT OR REPLACE INTO transactions (hash, data, account, lt, seqno) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    tx_meta.tx_hash.to_bytes(),
                    tx_data,
                    tx_meta.account.to_bytes(),
                    tx_meta.lt,
                    pending.block_meta.seqno
                ],
            )?;
        }

        let deltas_data = serde_json::to_vec(&pending.deltas)?;
        tx.execute(
            "INSERT OR REPLACE INTO account_deltas (seqno, data) VALUES (?1, ?2)",
            params![pending.block_meta.seqno, deltas_data],
        )?;

        for delta in &pending.deltas {
            if let Some(new_meta) = &delta.new_meta {
                let account_data = serde_json::to_vec(new_meta)?;
                tx.execute(
                    "INSERT OR REPLACE INTO accounts (address, data) VALUES (?1, ?2)",
                    params![delta.addr.to_bytes(), account_data],
                )?;
            } else {
                tx.execute(
                    "DELETE FROM accounts WHERE address = ?1",
                    params![delta.addr.to_bytes()],
                )?;
            }
        }

        // Faucet debits are applied when the message is enqueued and are not represented by
        // the destination transaction delta. Persist the resulting giver balance together with
        // the block that consumes the queued transfer, not while the message is still pending.
        if let Some(giver_meta) = latest.accounts.get(&GIVER_ADDR) {
            let account_data = serde_json::to_vec(giver_meta)?;
            tx.execute(
                "INSERT OR REPLACE INTO accounts (address, data) VALUES (?1, ?2)",
                params![GIVER_ADDR.to_bytes(), account_data],
            )?;
        }

        for h in pending
            .out_msg_hashes
            .iter()
            .chain(pending.msg_to_tx.iter().map(|(msg, _)| msg))
        {
            if let Some(msg_meta) = history.msg_by_hash.get(h) {
                let msg_data = serde_json::to_vec(msg_meta)?;
                tx.execute(
                    "INSERT OR REPLACE INTO messages (hash, data) VALUES (?1, ?2)",
                    params![h.to_bytes(), msg_data],
                )?;
            }
        }

        tx.commit()?;
        drop(conn);
        Ok(())
    }

    pub(crate) fn persist_account_meta(
        &self,
        addr: &Addr,
        meta: &AccountMeta,
    ) -> anyhow::Result<()> {
        let account_data = serde_json::to_vec(meta)?;
        self.conn
            .lock()
            .expect("Failed to lock DB connection")
            .execute(
                "INSERT OR REPLACE INTO accounts (address, data) VALUES (?1, ?2)",
                params![addr.to_bytes(), account_data],
            )?;

        Ok(())
    }

    #[allow(clippy::significant_drop_tightening)]
    pub(crate) fn export_cas_entries(&self) -> anyhow::Result<Vec<(Hash256, BocBytes)>> {
        let conn = self.conn.lock().expect("Failed to lock DB connection");
        let mut stmt = conn.prepare("SELECT hash, boc FROM cas")?;
        let iter = stmt.query_map([], |row| {
            let hash: Hash256 = row.get(0)?;
            let boc: BocBytes = row.get(1)?;
            Ok((hash, boc))
        })?;

        let mut entries = Vec::new();
        for row in iter {
            entries.push(row?);
        }
        entries.sort_by_key(|(hash, _)| *hash);
        Ok(entries)
    }

    #[allow(clippy::significant_drop_tightening)]
    pub(crate) fn replace_state(&self, snapshot: &NodeStateSnapshot) -> anyhow::Result<()> {
        let mut conn = self.conn.lock().expect("Failed to lock DB connection");
        let tx = conn.transaction()?;

        tx.execute("DELETE FROM cas", [])?;
        tx.execute("DELETE FROM blocks", [])?;
        tx.execute("DELETE FROM masterchain_blocks", [])?;
        tx.execute("DELETE FROM account_deltas", [])?;
        tx.execute("DELETE FROM transactions", [])?;
        tx.execute("DELETE FROM messages", [])?;
        tx.execute("DELETE FROM accounts", [])?;
        tx.execute(
            "INSERT OR REPLACE INTO node_metadata (key, value) VALUES ('origin_seqno', ?1)",
            params![snapshot.globals.origin_seqno],
        )?;
        if let Some(fork_seqno) = snapshot.globals.fork_seqno {
            tx.execute(
                "INSERT OR REPLACE INTO node_metadata (key, value) VALUES ('fork_seqno', ?1)",
                params![fork_seqno],
            )?;
        } else {
            tx.execute("DELETE FROM node_metadata WHERE key = 'fork_seqno'", [])?;
        }

        for (hash, boc) in &snapshot.cas_entries {
            tx.execute(
                "INSERT OR REPLACE INTO cas (hash, boc) VALUES (?1, ?2)",
                params![hash.to_bytes(), boc],
            )?;
        }

        for block in &snapshot.history_blocks {
            let block_data = serde_json::to_vec(block)?;
            tx.execute(
                "INSERT OR REPLACE INTO blocks (seqno, data) VALUES (?1, ?2)",
                params![block.seqno, block_data],
            )?;
        }

        for block in &snapshot.history_masterchain_blocks {
            let block_data = serde_json::to_vec(block)?;
            tx.execute(
                "INSERT OR REPLACE INTO masterchain_blocks (seqno, data) VALUES (?1, ?2)",
                params![block.seqno, block_data],
            )?;
        }

        for (index, deltas) in snapshot.history_deltas_by_seqno.iter().enumerate() {
            let seqno = snapshot.history_blocks[index].seqno;
            let data = serde_json::to_vec(deltas)?;
            tx.execute(
                "INSERT OR REPLACE INTO account_deltas (seqno, data) VALUES (?1, ?2)",
                params![seqno, data],
            )?;
        }

        for (hash, tx_meta) in &snapshot.history_tx_by_hash {
            let tx_data = serde_json::to_vec(tx_meta)?;
            tx.execute(
                "INSERT OR REPLACE INTO transactions (hash, data, account, lt, seqno) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    hash.to_bytes(),
                    tx_data,
                    tx_meta.account.to_bytes(),
                    tx_meta.lt,
                    tx_meta.block_seqno,
                ],
            )?;
        }

        for (hash, msg_meta) in &snapshot.history_msg_by_hash {
            let msg_data = serde_json::to_vec(msg_meta)?;
            tx.execute(
                "INSERT OR REPLACE INTO messages (hash, data) VALUES (?1, ?2)",
                params![hash.to_bytes(), msg_data],
            )?;
        }

        for (address, account_meta) in &snapshot.latest_accounts {
            let account_data = serde_json::to_vec(account_meta)?;
            tx.execute(
                "INSERT OR REPLACE INTO accounts (address, data) VALUES (?1, ?2)",
                params![address.to_bytes(), account_data],
            )?;
        }

        tx.commit()?;
        Ok(())
    }
}

fn init_schema(conn: &Connection) -> anyhow::Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS node_metadata (key TEXT PRIMARY KEY, value INTEGER NOT NULL)",
        [],
    )?;
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
        "CREATE TABLE IF NOT EXISTS account_deltas (seqno INTEGER PRIMARY KEY, data BLOB)",
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
    Ok(())
}
