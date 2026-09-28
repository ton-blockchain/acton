use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result, ensure};
use rocksdb::{DB, Options, WriteBatch, WriteOptions};
use rston::cell::{Cell, HashBytes};
use rston::models::{BlockId, StdAddr};
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::lazy::{Reader, RecordCache, RecordWeight};
use crate::state::StateUpdate;
use crate::{AccountSnapshot, NodeDb, StateView, cells};

const CHECKPOINT_KEY: &[u8] = b"ton-node-db.checkpoint";

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
struct StateRoot {
    block: BlockId,
    hash: HashBytes,
}

impl StateRoot {
    fn from_view(view: &StateView) -> Self {
        Self {
            block: view.block_id(),
            hash: *view.root.repr_hash(),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Checkpoint {
    version: u32,
    anchor: StateRoot,
    masterchain: StateRoot,
    shards: Vec<StateRoot>,
}

/// Durable state updates layered over an immutable validator snapshot.
///
/// A commit contains one masterchain step and its complete shard frontier.
/// New cells and the checkpoint share a synchronous `RocksDB` write batch; a
/// restart observes either the old checkpoint or the complete new checkpoint.
/// The update directory permits one writer. The snapshot must remain available
/// and unchanged, including after restart. Stored cells are append-only: this
/// store does not collect old cells or verify consensus signatures.
pub struct StateStore {
    state: StateSnapshot,
    workers: rayon::ThreadPool,
    write_failed: bool,
}

/// Read access to one complete committed masterchain and shard frontier.
///
/// Clones share database handles, immutable roots, and a bounded record cache,
/// not loaded state graphs.
/// Each query has its own cell read budget and can run alongside the writer.
/// Append-only cells keep this checkpoint readable after later commits or after
/// the writer is dropped. Drop all snapshots before reopening the update directory:
/// their shared database handle retains its exclusive filesystem lock.
#[derive(Clone)]
pub struct StateSnapshot {
    cells: Arc<DB>,
    updates: Arc<DB>,
    checkpoint: Arc<Checkpoint>,
    records: Arc<RecordCache>,
    max_cells: usize,
}

impl StateStore {
    /// Opens existing updates or starts at the snapshot's shard-client checkpoint.
    /// Falls back to its initialization checkpoint only when no shard-client
    /// checkpoint exists. All referenced shard states must be retained.
    /// `max_cells` bounds database reads per query or complete batch.
    pub fn open(snapshot_path: &Path, updates_path: &Path, max_cells: usize) -> Result<Self> {
        ensure!(max_cells > 0, "cell read limit must be positive");
        ensure_separate(snapshot_path, updates_path)?;

        let snapshot = NodeDb::open(snapshot_path)?;
        let markers = snapshot.checkpoints()?;
        let id = markers
            .shard_client
            .or(markers.init_block)
            .context("snapshot has no masterchain checkpoint")?;
        let anchor_view = snapshot.state(&id, max_cells)?;
        let anchor = StateRoot::from_view(&anchor_view);

        let mut options = Options::default();
        options.create_if_missing(true);
        options.set_max_open_files(128);
        fs::create_dir_all(updates_path)?;
        let updates =
            Arc::new(DB::open(&options, updates_path).with_context(|| {
                format!("cannot open state updates {}", updates_path.display())
            })?);
        let saved = updates.get(CHECKPOINT_KEY)?;
        let checkpoint = if let Some(bytes) = &saved {
            let checkpoint: Checkpoint =
                serde_json::from_slice(bytes).context("invalid state checkpoint")?;
            ensure!(
                checkpoint.version == 1,
                "unsupported state checkpoint version"
            );
            ensure!(
                checkpoint.anchor == anchor,
                "state updates belong to a different snapshot"
            );
            checkpoint
        } else {
            ensure!(
                updates
                    .iterator(rocksdb::IteratorMode::Start)
                    .next()
                    .transpose()?
                    .is_none(),
                "update database contains data without a state checkpoint"
            );
            let shards = anchor_view
                .shard_blocks()?
                .into_iter()
                .map(|id| {
                    snapshot
                        .state(&id, max_cells)
                        .map(|view| StateRoot::from_view(&view))
                })
                .collect::<Result<Vec<_>>>()?;

            Checkpoint {
                version: 1,
                anchor: anchor.clone(),
                masterchain: anchor,
                shards,
            }
        };

        let store = Self {
            state: StateSnapshot {
                cells: Arc::clone(&snapshot.cells),
                updates,
                checkpoint: Arc::new(checkpoint),
                records: Arc::new(RecordCache::with_weighter(
                    200_000,
                    32 * 1024 * 1024,
                    RecordWeight,
                )),
                max_cells,
            },
            workers: rayon::ThreadPoolBuilder::new()
                .num_threads(
                    std::thread::available_parallelism().map_or(1, |count| count.get().min(8)),
                )
                .thread_name(|index| format!("state-apply-{index}"))
                .build()
                .context("cannot start state application workers")?,
            write_failed: false,
        };
        let reader = store.state.reader();
        let master = store
            .state
            .view(&reader, &store.state.checkpoint.masterchain)?;
        let mut expected = master.shard_blocks()?;
        let mut actual = store
            .state
            .checkpoint
            .shards
            .iter()
            .map(|root| root.block)
            .collect::<Vec<_>>();
        expected.sort_unstable();
        actual.sort_unstable();
        ensure!(
            expected == actual,
            "saved state checkpoint has an incomplete shard frontier"
        );
        for root in &store.state.checkpoint.shards {
            store.state.view(&reader, root)?;
        }

        if saved.is_none() {
            let mut batch = WriteBatch::default();
            batch.put(
                CHECKPOINT_KEY,
                serde_json::to_vec(store.state.checkpoint.as_ref())?,
            );
            store.write(batch)?;
        }

        Ok(store)
    }

    /// The masterchain block whose state and complete shard frontier are durable.
    /// This can lag behind the separate P2P download checkpoint.
    #[must_use]
    pub fn head(&self) -> BlockId {
        self.state.head()
    }

    /// Pins the last successful commit for independent reads. Capturing a snapshot
    /// clones shared handles without reading or copying cells. It does not follow
    /// later commits; capture another snapshot to observe a newer frontier.
    #[must_use]
    pub fn snapshot(&self) -> StateSnapshot {
        self.state.clone()
    }

    /// Opens the current masterchain state. The view stays pinned to this block
    /// even if later commits advance the store.
    pub fn masterchain_state(&self) -> Result<StateView> {
        self.state.masterchain_state()
    }

    /// Queries the account at the last complete committed frontier. Returned
    /// cells are owned and do not depend on this store remaining open.
    pub fn get_account(&self, address: &StdAddr) -> Result<AccountSnapshot> {
        self.state.get_account(address)
    }

    /// Applies a successor masterchain block and all new shard blocks, in
    /// predecessor-first order. Each tuple contains the full ID and original
    /// block `BoC`. Checks hashes, ancestry, Merkle updates and the final frontier.
    /// Splits share the parent state; merges require both child states.
    /// Incomplete batches fail without advancing the checkpoint.
    /// Repeating the already committed masterchain ID is a no-op. After a write
    /// error, reopen the store to determine the durable checkpoint before retrying.
    pub fn apply_batch<'a>(
        &mut self,
        masterchain: (BlockId, &[u8]),
        shards: impl IntoIterator<Item = (BlockId, &'a [u8])>,
    ) -> Result<()> {
        ensure!(
            !self.write_failed,
            "reopen state store after a failed database write"
        );
        if masterchain.0 == self.head() {
            return Ok(());
        }

        self.apply_updates(
            StateUpdate::parse(masterchain.0, masterchain.1)?,
            shards.into_iter().map(|(id, boc)| {
                StateUpdate::parse(id, boc).with_context(|| format!("invalid shard block {id}"))
            }),
        )
    }

    /// Applies decoded blocks with the same atomicity and ancestry checks as
    /// [`Self::apply_batch`], without reading or decoding their `BoCs` again.
    /// Callers must verify each original `BoC` against its file hash before using
    /// this method: a cell proves its root hash, not its serialized file hash.
    /// Root hashes, headers, Merkle updates and the complete frontier are checked
    /// here. Shards must be in predecessor-first order; consensus verification
    /// remains the caller's responsibility.
    pub fn apply_roots<'a>(
        &mut self,
        masterchain: (BlockId, &Cell),
        shards: impl IntoIterator<Item = (BlockId, &'a Cell)>,
    ) -> Result<()> {
        ensure!(
            !self.write_failed,
            "reopen state store after a failed database write"
        );
        if masterchain.0 == self.head() {
            return Ok(());
        }

        self.apply_updates(
            StateUpdate::from_root(masterchain.0, masterchain.1)?,
            shards.into_iter().map(|(id, root)| {
                StateUpdate::from_root(id, root)
                    .with_context(|| format!("invalid shard block {id}"))
            }),
        )
    }

    fn apply_updates(
        &mut self,
        masterchain: StateUpdate,
        shards: impl IntoIterator<Item = Result<StateUpdate>>,
    ) -> Result<()> {
        let started = Instant::now();
        let reader = self.state.reader();
        let master = self
            .state
            .view(&reader, &self.state.checkpoint.masterchain)?;
        ensure!(
            master.block_id().shard.is_masterchain(),
            "expected masterchain state"
        );
        let master = masterchain.apply(&[&master], None)?;
        let master_applied = Instant::now();

        let mut states = self
            .state
            .checkpoint
            .shards
            .iter()
            .map(|root| Ok((root.block, self.state.view(&reader, root)?)))
            .collect::<Result<BTreeMap<_, _>>>()?;
        let mut consumed = HashSet::new();

        for update in shards {
            let update = update?;
            let id = update.id;
            ensure!(!states.contains_key(&id), "duplicate shard block {id}");
            let previous = update
                .predecessors
                .iter()
                .map(|parent| {
                    states.get(parent).with_context(|| {
                        format!("missing predecessor state {parent} for shard block {id}")
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            let state = update
                .apply(&previous, Some(&self.workers))
                .with_context(|| format!("cannot apply shard block {id}"))?;
            consumed.extend(update.predecessors);
            states.insert(id, state);
        }

        // Keep consumed states until both children of a split have used their
        // common parent. Only the final leaves belong in the durable checkpoint.
        states.retain(|id, _| !consumed.contains(id));
        let mut frontier = master.shard_blocks()?;
        frontier.sort_unstable();
        ensure!(
            states.keys().copied().eq(frontier),
            "incomplete shard state batch: final states do not match masterchain frontier"
        );

        let checkpoint = Checkpoint {
            version: 1,
            anchor: self.state.checkpoint.anchor.clone(),
            masterchain: StateRoot::from_view(&master),
            shards: states.values().map(StateRoot::from_view).collect(),
        };
        let applied = Instant::now();
        let mut batch = WriteBatch::default();
        let mut visited = HashSet::new();
        for state in std::iter::once(&master).chain(states.values()) {
            Self::persist_cells(state, &mut batch, &mut visited)?;
        }
        batch.put(CHECKPOINT_KEY, serde_json::to_vec(&checkpoint)?);
        let encoded = Instant::now();

        if let Err(error) = self.write(batch) {
            self.write_failed = true;
            return Err(error.context("state commit failed; reopen the store before continuing"));
        }
        self.state.checkpoint = Arc::new(checkpoint);

        debug!(
            operation = "state_commit",
            target = %masterchain.id,
            masterchain_us = master_applied.duration_since(started).as_micros(),
            apply_us = applied.duration_since(started).as_micros(),
            encode_us = encoded.duration_since(applied).as_micros(),
            write_us = encoded.elapsed().as_micros(),
            records = reader.stats().records,
            bytes = reader.stats().bytes,
            cache_hits = reader.stats().cache_hits,
            duration_ms = started.elapsed().as_millis(),
            outcome = "committed",
            "applied and persisted state cells",
        );

        Ok(())
    }

    fn write(&self, batch: WriteBatch) -> Result<()> {
        let mut options = WriteOptions::default();
        options.set_sync(true);
        self.state
            .updates
            .write_opt(batch, &options)
            .context("cannot persist state checkpoint")
    }

    fn persist_cells(
        state: &StateView,
        batch: &mut WriteBatch,
        visited: &mut HashSet<HashBytes>,
    ) -> Result<()> {
        state.reader.run(|| {
            let mut pending = vec![state.root.clone()];
            while let Some(cell) = pending.pop() {
                let hash = *cell.repr_hash();
                if !visited.insert(hash) {
                    continue;
                }
                // Loaded database records already own a persisted subgraph.
                // Cells from an update can be written again under the same hash;
                // deduplicating them against the snapshot would require random I/O.
                if state.reader.is_stored(&hash) {
                    continue;
                }

                batch.put(hash.as_slice(), cells::encode(cell.as_ref())?);
                for index in 0..cell.reference_count() {
                    pending.push(
                        cell.reference_cloned(index)
                            .context("missing state cell reference")?,
                    );
                }
            }

            Ok(())
        })
    }
}

impl StateSnapshot {
    /// Identifies the complete durable frontier used by every query on this handle.
    #[must_use]
    pub fn head(&self) -> BlockId {
        self.checkpoint.masterchain.block
    }

    /// Opens this checkpoint's masterchain state with a fresh cell read budget.
    pub fn masterchain_state(&self) -> Result<StateView> {
        self.view(&self.reader(), &self.checkpoint.masterchain)
    }

    /// Resolves the account's shard from this checkpoint and returns owned cells.
    /// Concurrent commits cannot change its block IDs, timestamp, or account data.
    pub fn get_account(&self, address: &StdAddr) -> Result<AccountSnapshot> {
        let reader = self.reader();
        let master = self.view(&reader, &self.checkpoint.masterchain)?;
        let shard_id = master.account_shard(address)?;
        let shard = if shard_id == self.head() {
            master
        } else {
            let root = self
                .checkpoint
                .shards
                .iter()
                .find(|root| root.block == shard_id)
                .context("account shard is absent from committed state frontier")?;
            self.view(&reader, root)?
        };
        let account = shard.get_account(address)?;

        Ok(AccountSnapshot {
            masterchain_block: self.head(),
            shard_block: shard_id,
            gen_utime: shard.gen_utime()?,
            account,
            reads: reader.stats(),
        })
    }

    fn reader(&self) -> Arc<Reader> {
        Reader::with_updates(
            Arc::clone(&self.cells),
            Some(Arc::clone(&self.updates)),
            Some(Arc::clone(&self.records)),
            self.max_cells,
        )
    }

    fn view(&self, reader: &Arc<Reader>, root: &StateRoot) -> Result<StateView> {
        StateView::load(Arc::clone(reader), root.block, root.hash)
    }
}

/// Resolve symlinks in existing ancestors before creating the writable database.
fn ensure_separate(snapshot: &Path, updates: &Path) -> Result<()> {
    let snapshot = dunce::canonicalize(snapshot)?;
    let absolute = std::path::absolute(updates)?;
    let ancestor = absolute
        .ancestors()
        .find(|path| path.exists())
        .context("state update path has no existing ancestor")?;
    let resolved = dunce::canonicalize(ancestor)?.join(absolute.strip_prefix(ancestor)?);
    let mut normalized = PathBuf::new();
    for component in resolved.components() {
        match component {
            Component::ParentDir => {
                normalized.pop();
            }
            Component::CurDir => {}
            other => normalized.push(other),
        }
    }
    ensure!(
        !normalized.starts_with(&snapshot) && !snapshot.starts_with(&normalized),
        "state update directory must be separate from the snapshot"
    );

    Ok(())
}
