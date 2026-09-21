//! Read-only access to extracted TON validator database snapshots.
//!
//! Available since trunk.

mod cells;
mod lazy;
mod state;
mod tl;

pub mod package;

pub use lazy::ReadStats;
pub use state::{AccountSnapshot, StateView};

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail, ensure};
use rocksdb::{DB, Direction, IteratorMode};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, HashBytes};
use tycho_types::models::{Block, BlockId, ShardStateUnsplit, StdAddr};

use package::PackageReader;

/// A retained state root associated with a full block ID.
/// TON links these descriptors for state garbage collection. The links are
/// descriptor-key hashes, not blockchain predecessor or successor hashes.
#[derive(Debug, Clone, Serialize)]
pub struct StateRecord {
    pub block_id: BlockId,
    pub root_hash: HashBytes,
    pub previous_descriptor: HashBytes,
    pub next_descriptor: HashBytes,
}

/// Independent progress markers persisted by the validator.
/// `shard_client` identifies the masterchain block whose referenced shards have
/// been processed. The initialization marker is saved independently and need
/// not identify the newest retained block.
#[derive(Debug, Default, Serialize)]
pub struct Checkpoints {
    pub database_version: Option<i32>,
    pub init_block: Option<BlockId>,
    pub shard_client: Option<BlockId>,
    pub gc_block: Option<BlockId>,
}

/// Logical key/value sizes after WAL replay and merge resolution.
/// These sizes differ from physical SST and WAL file sizes.
#[derive(Debug, Default, Serialize)]
pub struct KeyValueSummary {
    pub path: PathBuf,
    pub records: u64,
    pub key_bytes: u64,
    pub value_bytes: u64,
}

/// Contents of one package. Counts include duplicate files written by TON.
#[derive(Debug, Serialize)]
pub struct PackageSummary {
    pub path: PathBuf,
    pub bytes: u64,
    pub entries: BTreeMap<String, u64>,
}

/// A standalone state BoC stored under `static` or `archive/states`.
/// Its filename can contain a file-reference hash rather than its root hash.
#[derive(Debug, Serialize)]
pub struct StateFileSummary {
    pub path: PathBuf,
    pub bytes: u64,
}

/// Physical storage inventory and decoded validator checkpoints.
/// Inventory counts do not establish consensus validity or verify every block.
#[derive(Debug, Serialize)]
pub struct DatabaseSummary {
    pub checkpoints: Checkpoints,
    pub databases: Vec<KeyValueSummary>,
    pub packages: Vec<PackageSummary>,
    pub state_files: Vec<StateFileSummary>,
    pub cell_records: u64,
    pub retained_states: usize,
    pub unique_masterchain_blocks: usize,
    pub unique_shard_blocks: usize,
}

/// Read-only view of an extracted validator-engine database.
/// The directory must remain unchanged for the reader's lifetime. RocksDB
/// snapshots alone cannot make the node's separate package files consistent.
pub struct NodeDb {
    directory: PathBuf,
    cells: Arc<DB>,
}

impl NodeDb {
    /// Opens `celldb` without creating files, repairing the database, or taking
    /// its writer lock. Existing WAL records are included in the view.
    pub fn open(directory: &Path) -> Result<Self> {
        Ok(Self {
            directory: directory.to_path_buf(),
            cells: Arc::new(cells::open_database(&directory.join("celldb"), true)?),
        })
    }

    /// Reads named progress markers using the SHA-256 keys from TON's TL schema.
    pub fn checkpoints(&self) -> Result<Checkpoints> {
        let db = cells::open_database(&self.directory.join("state"), false)?;
        let mut result = Checkpoints::default();

        for key in [
            tl::CheckpointKey::Init,
            tl::CheckpointKey::ShardClient,
            tl::CheckpointKey::Gc,
            tl::CheckpointKey::Version,
        ] {
            let hash = Sha256::digest(tl_proto::serialize(&key));
            let Some(value) = db.get_pinned(hash)? else {
                continue;
            };

            match key {
                tl::CheckpointKey::Init => {
                    result.init_block = Some(tl::read::<tl::InitBlock>(&value)?.block.try_into()?);
                }
                tl::CheckpointKey::ShardClient => {
                    result.shard_client =
                        Some(tl::read::<tl::ShardClient>(&value)?.block.try_into()?);
                }
                tl::CheckpointKey::Gc => {
                    result.gc_block = Some(tl::read::<tl::GcBlock>(&value)?.block.try_into()?);
                }
                tl::CheckpointKey::Version => {
                    result.database_version = Some(tl::read::<tl::DbVersion>(&value)?.version);
                }
            }
        }

        Ok(result)
    }

    /// Lists state descriptors retained by the node's garbage collector.
    /// Historical blocks can remain in packages after their states are removed.
    pub fn states(&self) -> Result<Vec<StateRecord>> {
        let mut states = Vec::new();

        for entry in self
            .cells
            .iterator(IteratorMode::From(b"desc", Direction::Forward))
        {
            let (key, value) = entry?;
            if !key.starts_with(b"desc") {
                break;
            }
            if key.as_ref() == b"desczero" {
                continue;
            }

            let value: tl::CellState = tl::read(&value).with_context(|| {
                format!("invalid state descriptor {}", String::from_utf8_lossy(&key))
            })?;
            states.push(StateRecord {
                block_id: value.block_id.try_into()?,
                root_hash: value.root_hash.into(),
                previous_descriptor: value.prev.into(),
                next_descriptor: value.next.into(),
            });
        }
        states.sort_by_key(|state| state.block_id);

        Ok(states)
    }

    /// Reads a retained state descriptor by its TON database key, without
    /// scanning cell records or historical state descriptors.
    pub fn state_record(&self, id: &BlockId) -> Result<StateRecord> {
        let value = self
            .cells
            .get_pinned(tl::state_key(id))?
            .with_context(|| format!("state for block {id} is not retained"))?;
        let value: tl::CellState = tl::read(&value)?;
        let block_id = value.block_id.try_into()?;
        ensure!(block_id == *id, "state descriptor block ID mismatch");

        Ok(StateRecord {
            block_id,
            root_hash: value.root_hash.into(),
            previous_descriptor: value.prev.into(),
            next_descriptor: value.next.into(),
        })
    }

    /// Opens a lazy view of one retained state. The read budget belongs to this
    /// view and covers all its queries and subsequent masterchain updates.
    /// The view keeps the database open even if this NodeDb is dropped.
    pub fn state(&self, id: &BlockId, max_cells: usize) -> Result<StateView> {
        StateView::open(Arc::clone(&self.cells), self.state_record(id)?, max_cells)
    }

    /// Reads an account at the shard frontier of an exact masterchain block.
    /// Missing retained state is an error; an absent account is `None` inside
    /// the result. The returned account owns all its cells and performs no I/O.
    pub fn get_account(
        &self,
        masterchain: &BlockId,
        address: &StdAddr,
        max_cells: usize,
    ) -> Result<AccountSnapshot> {
        ensure!(
            masterchain.shard.is_masterchain(),
            "expected a masterchain block"
        );
        let master = self.state(masterchain, max_cells)?;
        let shard_id = master.account_shard(address)?;

        if shard_id == *masterchain {
            let account = master.get_account(address)?;
            return Ok(AccountSnapshot {
                masterchain_block: *masterchain,
                shard_block: shard_id,
                account,
                reads: master.read_stats(),
            });
        }

        let master_reads = master.read_stats();
        let shard = self.state(&shard_id, max_cells.saturating_sub(master_reads.records))?;
        let account = shard.get_account(address)?;
        let shard_reads = shard.read_stats();

        Ok(AccountSnapshot {
            masterchain_block: *masterchain,
            shard_block: shard_id,
            account,
            reads: ReadStats {
                records: master_reads.records + shard_reads.records,
                bytes: master_reads.bytes + shard_reads.bytes,
            },
        })
    }

    /// Reconstructs one retained state into an owned cell DAG.
    /// Checks cell hashes, reference depths, and the state's shard and sequence
    /// number. The caller bounds the number of database cell records in memory.
    /// Consensus signatures and state transitions are outside this reader.
    pub fn load_state(&self, record: &StateRecord, max_cells: usize) -> Result<Cell> {
        let root = cells::load(&self.cells, record.root_hash, max_cells)?;
        let state = root
            .parse::<ShardStateUnsplit>()
            .context("invalid shard state TL-B")?;
        ensure!(
            state.shard_ident == record.block_id.shard,
            "state shard mismatch"
        );
        ensure!(
            state.seqno == record.block_id.seqno,
            "state sequence number mismatch"
        );

        Ok(root)
    }

    /// Locates and reads an exact block, checking both its file and cell hashes.
    /// This initial reader scans package headers instead of using TON's archive
    /// routing indexes. Payload allocation is bounded by `max_bytes`.
    pub fn read_block(&self, id: &BlockId, max_bytes: usize) -> Result<Vec<u8>> {
        for path in self.package_paths()? {
            let mut package = PackageReader::open(&path)?;
            while let Some(entry) = package.next_entry()? {
                if entry.kind() != "block" || entry.block_id()? != Some(*id) {
                    continue;
                }

                let bytes = package.read_entry(&entry, max_bytes)?;
                ensure!(
                    Sha256::digest(&bytes)[..] == id.file_hash.0,
                    "block {id} file hash mismatch in {}",
                    path.display()
                );
                let cell = Boc::decode(&bytes).context("invalid block BoC")?;
                ensure!(
                    cell.repr_hash() == &id.root_hash,
                    "block {id} root hash mismatch"
                );
                let info = cell.parse::<Block>()?.load_info()?;
                ensure!(
                    info.shard == id.shard && info.seqno == id.seqno,
                    "block header mismatch"
                );

                return Ok(bytes);
            }
        }

        bail!("block {id} is absent from {}", self.directory.display())
    }

    /// Scans the logical databases and package headers. Block payloads and
    /// state DAGs are loaded only by explicit read requests.
    pub fn inspect(&self) -> Result<DatabaseSummary> {
        let files = files_under(&self.directory)?;
        let mut databases = Vec::new();
        let mut cell_records = 0;

        for current in files
            .iter()
            .filter(|path| path.file_name().is_some_and(|x| x == "CURRENT"))
        {
            let path = current.parent().context("database path has no parent")?;
            let relative = path.strip_prefix(&self.directory)?.to_path_buf();
            let is_cells = relative == Path::new("celldb");
            let other = if is_cells {
                None
            } else {
                Some(cells::open_database(path, false)?)
            };
            let db = other.as_ref().unwrap_or(&self.cells);
            let mut summary = KeyValueSummary {
                path: relative,
                ..Default::default()
            };

            for entry in db.iterator(IteratorMode::Start) {
                let (key, value) = entry?;
                summary.records += 1;
                summary.key_bytes += key.len() as u64;
                summary.value_bytes += value.len() as u64;
                if is_cells && key.len() == 32 {
                    cell_records += 1;
                }
            }
            databases.push(summary);
        }

        let mut packages = Vec::new();
        let mut blocks = BTreeSet::new();
        for path in self.package_paths()? {
            let mut package = PackageReader::open(&path)?;
            let mut entries = BTreeMap::new();
            while let Some(entry) = package
                .next_entry()
                .with_context(|| format!("cannot inspect package {}", path.display()))?
            {
                *entries.entry(entry.kind().to_owned()).or_default() += 1;
                if entry.kind() == "block" {
                    blocks.extend(entry.block_id()?);
                }
            }

            packages.push(PackageSummary {
                path: path.strip_prefix(&self.directory)?.to_path_buf(),
                bytes: fs::metadata(&path)?.len(),
                entries,
            });
        }

        let mut state_files = Vec::new();
        for path in files {
            let relative = path.strip_prefix(&self.directory)?;
            if relative.starts_with("static") || relative.starts_with("archive/states") {
                state_files.push(StateFileSummary {
                    path: relative.to_path_buf(),
                    bytes: fs::metadata(&path)?.len(),
                });
            }
        }

        Ok(DatabaseSummary {
            checkpoints: self.checkpoints()?,
            databases,
            packages,
            state_files,
            cell_records,
            retained_states: self.states()?.len(),
            unique_masterchain_blocks: blocks.iter().filter(|id| id.is_masterchain()).count(),
            unique_shard_blocks: blocks.iter().filter(|id| !id.is_masterchain()).count(),
        })
    }

    fn package_paths(&self) -> Result<Vec<PathBuf>> {
        let mut files = files_under(&self.directory.join("archive/packages"))?;
        files.extend(files_under(&self.directory.join("files/packages"))?);
        files.retain(|path| path.extension().is_some_and(|ext| ext == "pack"));
        files.sort();

        Ok(files)
    }
}

fn files_under(directory: &Path) -> Result<Vec<PathBuf>> {
    let mut directories = vec![directory.to_path_buf()];
    let mut files = Vec::new();

    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(&directory)
            .with_context(|| format!("cannot read {}", directory.display()))?
        {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() {
                directories.push(entry.path());
            } else if kind.is_file() {
                files.push(entry.path());
            }
        }
    }
    files.sort();

    Ok(files)
}
