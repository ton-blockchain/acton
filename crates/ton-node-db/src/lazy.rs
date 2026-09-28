//! Lazy references stay inside a state view. Its fallible API checks deferred
//! read errors before returning data or committing a new state root.

use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::{Context, Result, anyhow, ensure};
use quick_cache::{Weighter, sync::Cache};
use rocksdb::DB;
use rston::cell::{
    Cell, CellContext, CellDescriptor, CellFamily, CellImpl, CellInner, CellParts, DynCell,
    HashBytes, LevelMask,
};
use rston::util::ArrayVec;
use serde::Serialize;

use crate::cells::{Reference, StoredCell};

/// Logical cell-record reads performed by a state view, including its account queries.
///
/// Embedded bags of cells count as one record. Counts include cache hits; bytes
/// describe serialized records, not `RocksDB`'s physical I/O.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ReadStats {
    pub records: usize,
    pub bytes: usize,
    /// Records served from the shared serialized-record cache.
    pub cache_hits: usize,
}

/// Serialized, committed records only. Sharing lazy cells here would retain
/// their readers and state graphs. Content-addressed records remain valid across
/// checkpoints because both the snapshot and committed cell contents are immutable.
pub(crate) type RecordCache = Cache<HashBytes, Arc<[u8]>, RecordWeight>;

#[derive(Clone)]
pub(crate) struct RecordWeight;

impl Weighter<HashBytes, Arc<[u8]>> for RecordWeight {
    fn weight(&self, _: &HashBytes, value: &Arc<[u8]>) -> u64 {
        // Account for the key, Arc, and allocation header alongside record bytes.
        value.len() as u64 + 64
    }
}

pub(crate) struct Reader {
    db: Arc<DB>,
    updates: Option<Arc<DB>>,
    limit: usize,
    records: AtomicUsize,
    bytes: AtomicUsize,
    cache_hits: AtomicUsize,
    cache: Option<Arc<RecordCache>>,
    error: OnceLock<String>,
    stored: Mutex<HashSet<HashBytes>>,
}

impl Reader {
    pub(crate) fn new(db: Arc<DB>, limit: usize) -> Arc<Self> {
        Self::with_updates(db, None, None, limit)
    }

    pub(crate) fn with_updates(
        db: Arc<DB>,
        updates: Option<Arc<DB>>,
        cache: Option<Arc<RecordCache>>,
        limit: usize,
    ) -> Arc<Self> {
        Arc::new(Self {
            db,
            updates,
            limit,
            records: AtomicUsize::new(0),
            bytes: AtomicUsize::new(0),
            cache_hits: AtomicUsize::new(0),
            cache,
            error: OnceLock::new(),
            stored: Mutex::new(HashSet::new()),
        })
    }

    /// Recognizes records already read from either database. An unknown hash
    /// can still exist on disk; writing it again is safe in the append-only store.
    pub(crate) fn is_stored(&self, hash: &HashBytes) -> bool {
        self.stored
            .lock()
            .expect("stored cell hashes lock poisoned")
            .contains(hash)
    }

    pub(crate) fn stats(&self) -> ReadStats {
        ReadStats {
            records: self.records.load(Ordering::Relaxed),
            bytes: self.bytes.load(Ordering::Relaxed),
            cache_hits: self.cache_hits.load(Ordering::Relaxed),
        }
    }

    pub(crate) fn check(&self) -> Result<()> {
        if let Some(error) = self.error.get() {
            return Err(anyhow!("{error}"));
        }

        Ok(())
    }

    /// `CellImpl` cannot return I/O errors. No lazy cells may escape this guard:
    /// a failed read poisons the view, including apparently successful lookups.
    pub(crate) fn run<T>(&self, operation: impl FnOnce() -> Result<T>) -> Result<T> {
        self.check()?;
        let result = operation();
        self.check()?;

        result
    }

    #[allow(unsafe_code)]
    pub(crate) fn load(self: &Arc<Self>, hash: HashBytes) -> Result<Cell> {
        self.check()?;
        ensure!(
            self.records
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
                    (count < self.limit).then_some(count + 1)
                })
                .is_ok(),
            "lazy state read exceeds {} database records",
            self.limit
        );
        let cached = self.cache.as_ref().and_then(|cache| cache.get(&hash));
        let cache_hit = cached.is_some();
        let value: Arc<[u8]> = if let Some(value) = cached {
            self.cache_hits.fetch_add(1, Ordering::Relaxed);
            value
        } else {
            let updated = self
                .updates
                .as_ref()
                .map(|db| db.get_pinned(hash.as_slice()))
                .transpose()?
                .flatten();
            let value = match updated {
                Some(value) => value,
                None => self
                    .db
                    .get_pinned(hash.as_slice())?
                    .with_context(|| format!("cell {hash} is absent from the state databases"))?,
            };
            Arc::from(value.as_ref())
        };
        self.bytes.fetch_add(value.len(), Ordering::Relaxed);

        let cell = match StoredCell::parse(&value)
            .with_context(|| format!("cannot decode cell {hash}"))?
        {
            StoredCell::Boc(cell) => cell,
            StoredCell::Raw {
                descriptor,
                bits,
                data,
                refs,
            } => {
                let mut references = ArrayVec::new();
                let mut children_mask = 0;

                for reference in refs {
                    children_mask |= reference.level_mask;
                    let child = Arc::new(LazyCell {
                        reader: Arc::clone(self),
                        reference,
                        loaded: OnceLock::new(),
                    })
                    .into();
                    // SAFETY: StoredCell::parse rejects more than four refs;
                    // the destination is ArrayVec<Cell, MAX_REF_COUNT> (four).
                    unsafe { references.push(child) };
                }

                // CellBuilder reads child descriptors. Constructing CellParts
                // directly needs only stored child hashes and depths, so it
                // does not recursively fetch the entire state.
                Cell::empty_context()
                    .finalize_cell(CellParts {
                        bit_len: bits,
                        descriptor: CellDescriptor {
                            d1: descriptor & !16,
                            d2: ((bits / 8) * 2 + u16::from(bits % 8 != 0)) as u8,
                        },
                        children_mask: LevelMask::new(children_mask),
                        references,
                        data: &data,
                    })
                    .with_context(|| format!("invalid cell {hash}"))?
            }
        };
        ensure!(
            cell.repr_hash() == &hash,
            "cell {hash} representation hash mismatch"
        );
        if !cache_hit && let Some(cache) = &self.cache {
            cache.insert(hash, value);
        }

        // Only the record root is independently addressable. Children inside
        // an embedded BoC need not have their own database entries.
        if self.updates.is_some() {
            self.stored
                .lock()
                .expect("stored cell hashes lock poisoned")
                .insert(hash);
        }

        Ok(cell)
    }
}

struct LazyCell {
    reader: Arc<Reader>,
    reference: Reference,
    loaded: OnceLock<Option<Cell>>,
}

impl LazyCell {
    fn resolve(&self) -> &DynCell {
        self.loaded
            .get_or_init(|| {
                let result = self.reader.load(self.reference.hash()).and_then(|cell| {
                    self.reference.check(&cell)?;
                    Ok(cell)
                });

                match result {
                    Ok(cell) => Some(cell),
                    Err(error) => {
                        self.reader.error.get_or_init(|| {
                            format!("cannot load cell {}: {error:#}", self.reference.hash())
                        });
                        None
                    }
                }
            })
            .as_ref()
            .map_or_else(|| Cell::empty_cell_ref() as &DynCell, |cell| cell.as_ref())
    }
}

impl CellImpl for LazyCell {
    fn untrack(self: CellInner<Self>) -> Cell {
        self.into()
    }

    fn descriptor(&self) -> CellDescriptor {
        self.resolve().descriptor()
    }

    fn data(&self) -> &[u8] {
        self.resolve().data()
    }

    fn bit_len(&self) -> u16 {
        self.resolve().bit_len()
    }

    fn reference(&self, index: u8) -> Option<&DynCell> {
        self.resolve().reference(index)
    }

    fn reference_cloned(&self, index: u8) -> Option<Cell> {
        self.resolve().reference_cloned(index)
    }

    fn reference_repr_hash(&self, index: u8) -> Option<HashBytes> {
        self.resolve().reference_repr_hash(index)
    }

    fn virtualize(&self) -> &DynCell {
        self.resolve().virtualize()
    }

    fn hash(&self, level: u8) -> &HashBytes {
        &self.reference.hashes[LevelMask::new(self.reference.level_mask).hash_index(level) as usize]
    }

    fn depth(&self, level: u8) -> u16 {
        self.reference.depths[LevelMask::new(self.reference.level_mask).hash_index(level) as usize]
    }
}
