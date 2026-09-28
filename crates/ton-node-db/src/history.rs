//! Logical-time lookup over retained block files. State cells contain no history.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};
use rocksdb::{DB, Direction, IteratorMode, Options, WriteBatch};
use rston::boc::Boc;
use rston::cell::{Cell, Lazy};
use rston::models::{Block, BlockId, ShardIdent, StdAddr, Transaction};
use serde::{Deserialize, Serialize};
use tracing::info;

const MAX_BLOCK_BYTES: u64 = 64 * 1024 * 1024;

/// Persistent block lookup by shard and logical time, shared by readers and a writer.
///
/// The index owns metadata only. Indexed files must remain available and immutable.
/// The owner must serialize imports and insertions; readers can run concurrently.
/// It does not establish finality: callers must anchor queries to an applied account
/// state and verify transaction hashes while following its predecessor chain.
pub struct BlockIndex {
    db: DB,
}

#[derive(Serialize, Deserialize)]
struct Entry {
    id: BlockId,
    start_lt: u64,
    end_lt: u64,
    path: PathBuf,
}

/// A transaction with its containing block. The cell preserves the full message
/// and execution data for consumers that need more than decoded header fields.
pub struct BlockTransaction {
    pub block: BlockId,
    pub transaction: Lazy<Transaction>,
}

impl BlockIndex {
    /// Opens an index with an exclusive filesystem lock. The database is rebuildable
    /// from block files; its location must be separate from the state database.
    pub fn open(path: &Path) -> Result<Self> {
        let mut options = Options::default();
        options.create_if_missing(true);
        options.set_max_open_files(64);

        Ok(Self {
            db: DB::open(&options, path)
                .with_context(|| format!("cannot open block index {}", path.display()))?,
        })
    }

    /// Adds a downloaded block without decoding it again. The caller must have
    /// verified the original file hash and durably saved `path` before this call.
    /// Repeating the same block is idempotent; conflicting blocks are rejected.
    pub fn insert(&self, id: BlockId, root: &Cell, path: &Path) -> Result<()> {
        ensure!(
            root.repr_hash() == &id.root_hash,
            "indexed block root mismatch"
        );
        let header = root.parse::<Block>()?.load_info()?;
        ensure!(
            header.shard == id.shard && header.seqno == id.seqno,
            "indexed block header mismatch"
        );
        ensure!(header.start_lt < header.end_lt, "empty block LT range");

        let path = dunce::canonicalize(path)
            .with_context(|| format!("cannot locate block {}", path.display()))?;
        let key = block_key(id.shard, header.end_lt);
        if let Some(bytes) = self.db.get(key)? {
            let existing: Entry = serde_json::from_slice(&bytes)?;
            ensure!(existing.id == id, "conflicting block in history index");
        }

        let entry = Entry {
            id,
            start_lt: header.start_lt,
            end_lt: header.end_lt,
            path,
        };
        let mut batch = WriteBatch::default();
        batch.put(key, serde_json::to_vec(&entry)?);
        batch.put(file_key(&entry.path)?, []);
        self.db.write(batch).context("cannot persist block index")
    }

    /// Imports full `.boc` block files recursively, skipping `.proof.boc` files.
    /// Previously indexed paths are skipped. Complete import before serving reads
    /// so an unfinished import does not appear to be a gap in retained history.
    pub fn import_directory(&self, directory: &Path) -> Result<()> {
        if !directory.exists() {
            return Ok(());
        }

        let started = Instant::now();
        let mut progress = Instant::now();
        let mut pending = vec![dunce::canonicalize(directory)?];
        let mut imported = 0_u64;
        let mut skipped = 0_u64;

        while let Some(directory) = pending.pop() {
            for entry in fs::read_dir(&directory)? {
                let entry = entry?;
                let kind = entry.file_type()?;
                if kind.is_dir() {
                    pending.push(entry.path());
                    continue;
                }
                let path = entry.path();
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if !kind.is_file() || !name.ends_with(".boc") || name.ends_with(".proof.boc") {
                    continue;
                }

                if self.db.get(file_key(&path)?)?.is_some() {
                    skipped += 1;
                } else {
                    let bytes = read_block(&path)?;
                    let root = Boc::decode(&bytes)
                        .with_context(|| format!("invalid block {}", path.display()))?;
                    let header = root.parse::<Block>()?.load_info()?;
                    let id = BlockId {
                        shard: header.shard,
                        seqno: header.seqno,
                        root_hash: *root.repr_hash(),
                        file_hash: Boc::file_hash(&bytes),
                    };
                    self.insert(id, &root, &path)?;
                    imported += 1;
                }

                if progress.elapsed() >= Duration::from_secs(5) {
                    info!(
                        operation = "history_index",
                        target = %directory.display(),
                        imported,
                        skipped,
                        duration_ms = started.elapsed().as_millis(),
                        outcome = "indexing",
                        "indexing retained block files",
                    );
                    progress = Instant::now();
                }
            }
        }

        info!(
            operation = "history_index",
            target = %directory.display(),
            imported,
            skipped,
            duration_ms = started.elapsed().as_millis(),
            outcome = "ready",
            "retained block index is ready",
        );
        Ok(())
    }

    /// Opens an independent reader. Each reader retains at most one decoded block,
    /// so successive transactions in the same block require no additional file I/O.
    #[must_use]
    pub const fn reader(&self) -> TransactionReader<'_> {
        TransactionReader {
            index: self,
            cached: None,
        }
    }

    fn find(&self, address: &StdAddr, lt: u64) -> Result<Option<Entry>> {
        let Some(after) = lt.checked_add(1) else {
            return Ok(None);
        };
        let mut shard = ShardIdent::new(i32::from(address.workchain), 1 << 63)
            .context("invalid account workchain")?;

        loop {
            // Each historical shard has its own ordered LT range. Visiting the
            // account's parent prefixes also finds blocks before splits and merges.
            let key = block_key(shard, after);
            if let Some(entry) = self
                .db
                .iterator(IteratorMode::From(&key, Direction::Forward))
                .next()
            {
                let (found, bytes) = entry?;
                if found.starts_with(&key[..13]) {
                    let entry: Entry = serde_json::from_slice(&bytes)?;
                    if entry.start_lt <= lt && lt < entry.end_lt {
                        return Ok(Some(entry));
                    }
                }
            }

            if shard.is_masterchain() {
                return Ok(None);
            }
            let Some((left, right)) = shard.split() else {
                return Ok(None);
            };
            shard = if left.contains_address(address) {
                left
            } else {
                right
            };
        }
    }
}

/// Per-request block cache. Returned transaction cells are owned and remain valid
/// after this reader is dropped; no history is kept in memory between requests.
pub struct TransactionReader<'a> {
    index: &'a BlockIndex,
    cached: Option<(Entry, Block)>,
}

impl TransactionReader<'_> {
    /// Finds one account transaction at an exact LT. `None` means its block or
    /// transaction is unavailable locally, not that earlier account history is empty.
    /// The caller must compare the returned cell hash with its trusted cursor.
    pub fn get(&mut self, address: &StdAddr, lt: u64) -> Result<Option<BlockTransaction>> {
        ensure!(
            address.anycast.is_none(),
            "anycast addresses are not supported"
        );
        if lt == 0 {
            return Ok(None);
        }

        let cached = self.cached.as_ref().is_some_and(|(entry, _)| {
            entry.id.shard.contains_address(address) && entry.start_lt <= lt && lt < entry.end_lt
        });
        if !cached {
            let Some(entry) = self.index.find(address, lt)? else {
                return Ok(None);
            };
            let bytes = read_block(&entry.path)?;
            ensure!(
                Boc::file_hash(&bytes) == entry.id.file_hash,
                "history block file hash mismatch: {}",
                entry.path.display()
            );
            let root = Boc::decode(&bytes)?;
            ensure!(
                root.repr_hash() == &entry.id.root_hash,
                "history block root hash mismatch"
            );
            self.cached = Some((entry, root.parse::<Block>()?));
        }

        let (entry, block) = self.cached.as_ref().context("missing history block")?;
        let accounts = block.load_extra()?.account_blocks.load()?;
        let Some((_, account)) = accounts.get(address.address)? else {
            return Ok(None);
        };
        Ok(account
            .transactions
            .get(lt)?
            .map(|(_, transaction)| BlockTransaction {
                block: entry.id,
                transaction,
            }))
    }
}

fn block_key(shard: ShardIdent, end_lt: u64) -> [u8; 21] {
    let mut key = [b'b'; 21];
    key[1..5].copy_from_slice(&shard.workchain().to_be_bytes());
    key[5..13].copy_from_slice(&shard.prefix().to_be_bytes());
    key[13..].copy_from_slice(&end_lt.to_be_bytes());
    key
}

fn file_key(path: &Path) -> Result<Vec<u8>> {
    let path = path.to_str().context("block path must be UTF-8")?;
    Ok(format!("file:{path}").into_bytes())
}

fn read_block(path: &Path) -> Result<Vec<u8>> {
    ensure!(
        fs::metadata(path)?.len() <= MAX_BLOCK_BYTES,
        "history block exceeds 64 MiB: {}",
        path.display()
    );
    fs::read(path).with_context(|| format!("cannot read history block {}", path.display()))
}
