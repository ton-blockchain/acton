use crate::node_snapshot::NodeStateSnapshot;
use crate::storage::{
    AccountMeta, BlockMeta, History, Indexes, LatestState, MasterchainBlockMeta, MsgMeta,
    PendingCommit, ReverseLtKey, TxMeta,
};
use crate::types::{Addr, BocBytes, Hash256, Seqno};
use core::cmp;
use rusqlite::types::Type;
use rusqlite::{Connection, params};
use serde_json::Value;
use std::path::Path;
use std::sync::{Arc, Mutex};

pub(crate) struct NodePersistence {
    conn: Arc<Mutex<Connection>>,
}

pub(crate) struct PersistedNodeState {
    pub latest: LatestState,
    pub history: History,
    pub indexes: Indexes,
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

        let mut stmt = conn.prepare("SELECT data FROM masterchain_blocks ORDER BY seqno ASC")?;
        let block_iter = stmt.query_map([], |row| {
            let data: Vec<u8> = row.get(0)?;
            serde_json::from_slice::<MasterchainBlockMeta>(&data)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
        })?;
        for block in block_iter {
            history.masterchain_blocks.push(block?);
        }

        let mut stmt = conn.prepare("SELECT hash, data, account, lt, seqno FROM transactions")?;
        let tx_iter = stmt.query_map([], |row| {
            let hash = hash_from_db_bytes(row.get(0)?)?;
            let data: Vec<u8> = row.get(1)?;
            let account = addr_from_db_bytes(row.get(2)?)?;
            let lt: u64 = row.get(3)?;
            let seqno: u32 = row.get(4)?;
            let tx_meta = serde_json::from_slice::<TxMeta>(&data)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

            Ok((hash, tx_meta, account, lt, seqno))
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

        let mut stmt = conn.prepare("SELECT address, data FROM accounts")?;
        let acc_iter = stmt.query_map([], |row| {
            let addr = addr_from_db_bytes(row.get(0)?)?;
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
            let hash = hash_from_db_bytes(row.get(0)?)?;
            let data: Vec<u8> = row.get(1)?;
            let meta = serde_json::from_slice::<MsgMeta>(&data)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            Ok((hash, meta))
        })?;
        for msg in msg_iter {
            let (hash, meta) = msg?;
            history.msg_by_hash.insert(hash, meta);
        }

        let mut stmt = conn.prepare("SELECT code_hash, data FROM compiler_abis")?;
        let abi_iter = stmt.query_map([], |row| {
            let hash = hash_from_db_bytes(row.get(0)?)?;
            let data: Vec<u8> = row.get(1)?;
            let compiler_abi = serde_json::from_slice::<Value>(&data)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            Ok((hash, compiler_abi))
        })?;
        for abi in abi_iter {
            let (hash, compiler_abi) = abi?;
            history.compiler_abis.insert(hash, compiler_abi);
        }

        let mut stmt = conn.prepare("SELECT code_hash, data FROM verified_sources")?;
        let source_iter = stmt.query_map([], |row| {
            let hash = hash_from_db_bytes(row.get(0)?)?;
            let data: Vec<u8> = row.get(1)?;
            let source = serde_json::from_slice::<Value>(&data)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            Ok((hash, source))
        })?;
        for source in source_iter {
            let (hash, value) = source?;
            history.verified_sources.insert(hash, value);
        }
        Ok(PersistedNodeState {
            latest,
            history,
            indexes,
            head_seqno,
        })
    }

    pub(crate) fn persist_commit(
        &self,
        pending: &PendingCommit,
        history: &History,
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
                    tx_meta.tx_hash.0.to_vec(),
                    tx_data,
                    tx_meta.account.addr.to_vec(),
                    tx_meta.lt,
                    pending.block_meta.seqno
                ],
            )?;
        }

        for delta in &pending.deltas {
            if let Some(new_meta) = &delta.new_meta {
                let account_data = serde_json::to_vec(new_meta)?;
                tx.execute(
                    "INSERT OR REPLACE INTO accounts (address, data) VALUES (?1, ?2)",
                    params![delta.addr.addr.to_vec(), account_data],
                )?;
            }
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
                    params![h.0.to_vec(), msg_data],
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
                params![addr.addr.to_vec(), account_data],
            )?;

        Ok(())
    }

    pub(crate) fn set_compiler_abi(
        &self,
        code_hash: Hash256,
        compiler_abi: &Value,
        stale_keys: &[Hash256],
    ) -> anyhow::Result<()> {
        let data = serde_json::to_vec(compiler_abi)?;
        let mut conn = self.conn.lock().expect("Failed to lock DB connection");
        let tx = conn.transaction()?;
        for stale_key in stale_keys {
            tx.execute(
                "DELETE FROM compiler_abis WHERE code_hash = ?1",
                params![stale_key.0.to_vec()],
            )?;
        }
        tx.execute(
            "INSERT OR REPLACE INTO compiler_abis (code_hash, data) VALUES (?1, ?2)",
            params![code_hash.0.to_vec(), data],
        )?;
        tx.commit()?;
        drop(conn);
        Ok(())
    }

    pub(crate) fn delete_compiler_abi(&self, code_hash: Hash256) -> anyhow::Result<()> {
        self.conn
            .lock()
            .expect("Failed to lock DB connection")
            .execute(
                "DELETE FROM compiler_abis WHERE code_hash = ?1",
                params![code_hash.0.to_vec()],
            )?;
        Ok(())
    }

    pub(crate) fn set_verified_source(
        &self,
        code_hash: Hash256,
        source: &Value,
    ) -> anyhow::Result<()> {
        let data = serde_json::to_vec(source)?;
        self.conn
            .lock()
            .expect("Failed to lock DB connection")
            .execute(
                "INSERT OR REPLACE INTO verified_sources (code_hash, data) VALUES (?1, ?2)",
                params![code_hash.0.to_vec(), data],
            )?;
        Ok(())
    }

    pub(crate) fn delete_verified_source(&self, code_hash: Hash256) -> anyhow::Result<()> {
        self.conn
            .lock()
            .expect("Failed to lock DB connection")
            .execute(
                "DELETE FROM verified_sources WHERE code_hash = ?1",
                params![code_hash.0.to_vec()],
            )?;
        Ok(())
    }

    #[allow(clippy::significant_drop_tightening)]
    pub(crate) fn export_cas_entries(&self) -> anyhow::Result<Vec<(Hash256, BocBytes)>> {
        let conn = self.conn.lock().expect("Failed to lock DB connection");
        let mut stmt = conn.prepare("SELECT hash, boc FROM cas")?;
        let iter = stmt.query_map([], |row| {
            let hash = hash_from_db_bytes(row.get(0)?)?;
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
        tx.execute("DELETE FROM transactions", [])?;
        tx.execute("DELETE FROM messages", [])?;
        tx.execute("DELETE FROM accounts", [])?;
        tx.execute("DELETE FROM compiler_abis", [])?;
        tx.execute("DELETE FROM verified_sources", [])?;

        for (hash, boc) in &snapshot.cas_entries {
            tx.execute(
                "INSERT OR REPLACE INTO cas (hash, boc) VALUES (?1, ?2)",
                params![hash.0.to_vec(), boc],
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

        for (hash, tx_meta) in &snapshot.history_tx_by_hash {
            let tx_data = serde_json::to_vec(tx_meta)?;
            tx.execute(
                "INSERT OR REPLACE INTO transactions (hash, data, account, lt, seqno) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
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
                params![hash.0.to_vec(), msg_data],
            )?;
        }

        for (address, account_meta) in &snapshot.latest_accounts {
            let account_data = serde_json::to_vec(account_meta)?;
            tx.execute(
                "INSERT OR REPLACE INTO accounts (address, data) VALUES (?1, ?2)",
                params![address.addr.to_vec(), account_data],
            )?;
        }

        for (code_hash, compiler_abi) in &snapshot.history_compiler_abis {
            let data = serde_json::to_vec(compiler_abi)?;
            tx.execute(
                "INSERT OR REPLACE INTO compiler_abis (code_hash, data) VALUES (?1, ?2)",
                params![code_hash.0.to_vec(), data],
            )?;
        }

        for (code_hash, source) in &snapshot.history_verified_sources {
            let data = serde_json::to_vec(source)?;
            tx.execute(
                "INSERT OR REPLACE INTO verified_sources (code_hash, data) VALUES (?1, ?2)",
                params![code_hash.0.to_vec(), data],
            )?;
        }

        tx.commit()?;
        Ok(())
    }
}

fn init_schema(conn: &Connection) -> anyhow::Result<()> {
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
    Ok(())
}

fn hash_from_db_bytes(bytes: Vec<u8>) -> rusqlite::Result<Hash256> {
    let bytes = bytes.try_into().map_err(|bytes: Vec<u8>| {
        invalid_blob_error(format!("expected 32-byte hash, got {}", bytes.len()))
    })?;
    Ok(Hash256(bytes))
}

fn addr_from_db_bytes(bytes: Vec<u8>) -> rusqlite::Result<Addr> {
    let addr = bytes.try_into().map_err(|bytes: Vec<u8>| {
        invalid_blob_error(format!("expected 32-byte address, got {}", bytes.len()))
    })?;
    Ok(Addr { workchain: 0, addr })
}

fn invalid_blob_error(message: String) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        Type::Blob,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            message,
        )),
    )
}
