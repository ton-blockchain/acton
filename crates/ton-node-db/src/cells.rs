//! Cell records use TON's `CellStorage.cpp` layout, not the BoC container layout.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{Context, Result, ensure};
use rocksdb::{DB, MergeOperands, Options};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, HashBytes};

pub(crate) fn open_database(path: &Path, cells: bool) -> Result<DB> {
    let mut options = Options::default();

    // Large snapshots contain thousands of SSTs. Open table readers on demand
    // instead of exhausting file descriptors and loading every index at startup.
    options.set_max_open_files(128);

    if cells {
        // WALs and SSTs can contain pending reference-count deltas. Reading only
        // the base values would lose those updates, even in a read-only snapshot.
        options.set_merge_operator("MergeOperatorAddCellRefcnt", merge_value, merge_deltas);
    }

    DB::open_for_read_only(&options, path, false)
        .with_context(|| format!("cannot open RocksDB snapshot {}", path.display()))
}

fn merge_deltas(_: &[u8], existing: Option<&[u8]>, operands: &MergeOperands) -> Option<Vec<u8>> {
    let mut delta = match existing {
        Some(bytes) => i32::from_le_bytes(bytes.try_into().ok()?),
        None => 0,
    };

    for operand in operands {
        delta = delta.checked_add(i32::from_le_bytes(operand.try_into().ok()?))?;
    }

    Some(delta.to_le_bytes().to_vec())
}

fn merge_value(_: &[u8], existing: Option<&[u8]>, operands: &MergeOperands) -> Option<Vec<u8>> {
    let mut value = existing?.to_vec();
    let first = i32::from_le_bytes(value.get(..4)?.try_into().ok()?);
    let offset = if first == -1 { 4 } else { 0 };
    let mut refcount = i32::from_le_bytes(value.get(offset..offset + 4)?.try_into().ok()?);

    for operand in operands {
        refcount = refcount.checked_add(i32::from_le_bytes(operand.try_into().ok()?))?;
    }

    if refcount <= 0 {
        return None;
    }
    value[offset..offset + 4].copy_from_slice(&refcount.to_le_bytes());

    Some(value)
}

pub(super) struct Reference {
    pub level_mask: u8,
    pub hashes: Vec<HashBytes>,
    pub depths: Vec<u16>,
}

impl Reference {
    pub(super) fn hash(&self) -> HashBytes {
        self.hashes[self.hashes.len() - 1]
    }

    pub(super) fn check(&self, cell: &Cell) -> Result<()> {
        ensure!(
            cell.level_mask().to_byte() == self.level_mask,
            "child level mask mismatch"
        );
        let mut index = 0;

        for level in 0..=3 {
            if level == 0 || self.level_mask & (1 << (level - 1)) != 0 {
                ensure!(
                    cell.hash(level) == &self.hashes[index],
                    "child hash mismatch"
                );
                ensure!(
                    cell.depth(level) == self.depths[index],
                    "child depth mismatch"
                );
                index += 1;
            }
        }

        Ok(())
    }
}

pub(super) enum StoredCell {
    Boc(Cell),
    Raw {
        descriptor: u8,
        bits: u16,
        data: Vec<u8>,
        refs: Vec<Reference>,
    },
}

impl StoredCell {
    pub(super) fn parse(mut bytes: &[u8]) -> Result<Self> {
        let mut refcount = i32::from_le_bytes(take(&mut bytes, 4)?.try_into()?);
        let is_boc = refcount == -1;
        if is_boc {
            refcount = i32::from_le_bytes(take(&mut bytes, 4)?.try_into()?);
        }
        ensure!(refcount > 0, "invalid cell reference count {refcount}");

        if is_boc {
            return Ok(Self::Boc(
                Boc::decode(bytes).context("invalid embedded cell BoC")?,
            ));
        }

        let descriptor = take(&mut bytes, 2)?;
        let d1 = descriptor[0];
        let d2 = descriptor[1];
        let refs_count = d1 & 7;
        ensure!(
            refs_count <= 4,
            "invalid cell reference count in descriptor"
        );

        if d1 & 16 != 0 {
            let hashes = (d1 >> 5).count_ones() as usize + 1;
            take(&mut bytes, hashes * 34)?;
        }

        let data = take(&mut bytes, usize::from(d2).div_ceil(2))?.to_vec();
        let mut bits = u16::try_from(data.len() * 8)?;
        if d2 & 1 != 0 {
            let last = *data.last().context("missing partial cell byte")?;
            ensure!(last & 0x7f != 0, "invalid cell termination bit");
            bits -= u16::try_from(last.trailing_zeros() + 1)?;
        }

        let mut refs = Vec::with_capacity(usize::from(refs_count));
        for _ in 0..refs_count {
            let level_mask = take(&mut bytes, 1)?[0];
            ensure!(level_mask <= 7, "invalid child level mask");
            let count = level_mask.count_ones() + 1;
            let mut hashes = Vec::new();
            let mut depths = Vec::new();

            for _ in 0..count {
                hashes.push(HashBytes::from(<[u8; 32]>::try_from(take(
                    &mut bytes, 32,
                )?)?));
            }
            for _ in 0..count {
                depths.push(u16::from_be_bytes(take(&mut bytes, 2)?.try_into()?));
            }
            refs.push(Reference {
                level_mask,
                hashes,
                depths,
            });
        }
        ensure!(bytes.is_empty(), "trailing bytes in cell record");

        Ok(Self::Raw {
            descriptor: d1,
            bits,
            data,
            refs,
        })
    }

    fn build(self, loaded: &HashMap<HashBytes, Cell>) -> Result<Cell> {
        let (descriptor, bits, data, refs) = match self {
            Self::Boc(cell) => return Ok(cell),
            Self::Raw {
                descriptor,
                bits,
                data,
                refs,
            } => (descriptor, bits, data, refs),
        };

        let mut builder = CellBuilder::new();
        builder.set_exotic(descriptor & 8 != 0);
        builder.store_raw(&data, bits)?;

        for reference in refs {
            let child = loaded
                .get(&reference.hash())
                .context("missing child cell")?;
            reference.check(child)?;
            builder.store_reference(child.clone())?;
        }

        let cell = builder.build()?;
        ensure!(
            cell.level_mask().to_byte() == descriptor >> 5,
            "cell level mask mismatch"
        );

        Ok(cell)
    }
}

/// Reconstructs a DAG iteratively, checking each record against its RocksDB key.
/// The cache belongs to this load, so a reader does not retain previous states.
pub(crate) fn load(db: &DB, root: HashBytes, max_cells: usize) -> Result<Cell> {
    let mut loaded = HashMap::new();
    let mut visiting = HashSet::new();
    let mut stack = vec![(root, None)];

    while let Some((hash, pending)) = stack.pop() {
        if loaded.contains_key(&hash) {
            continue;
        }

        if let Some(record) = pending {
            let cell = StoredCell::build(record, &loaded)
                .with_context(|| format!("invalid cell {hash}"))?;
            ensure!(
                cell.repr_hash() == &hash,
                "cell {hash} representation hash mismatch"
            );
            loaded.insert(hash, cell);
            visiting.remove(&hash);
            continue;
        }

        ensure!(visiting.insert(hash), "cycle in cell graph at {hash}");
        ensure!(
            loaded.len() + visiting.len() <= max_cells,
            "cell graph exceeds {max_cells} records"
        );
        let value = db
            .get_pinned(hash.as_slice())?
            .with_context(|| format!("cell {hash} is absent from the snapshot"))?;
        let record =
            StoredCell::parse(&value).with_context(|| format!("cannot decode cell {hash}"))?;
        let children = match &record {
            StoredCell::Boc(_) => Vec::new(),
            StoredCell::Raw { refs, .. } => refs.iter().map(Reference::hash).collect(),
        };

        stack.push((hash, Some(record)));
        for child in children.into_iter().rev() {
            stack.push((child, None));
        }
    }

    loaded.remove(&root).context("state root was not loaded")
}

fn take<'a>(bytes: &mut &'a [u8], size: usize) -> Result<&'a [u8]> {
    ensure!(bytes.len() >= size, "truncated cell record");
    let (result, rest) = bytes.split_at(size);
    *bytes = rest;

    Ok(result)
}
