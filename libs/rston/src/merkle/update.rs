use std::borrow::Borrow;
#[cfg(all(feature = "rayon", feature = "sync"))]
use std::sync::Arc;
#[cfg(all(feature = "rayon", feature = "sync"))]
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

#[cfg(all(feature = "rayon", feature = "sync"))]
use super::sync_util::{ChildrenBuilder, ExtCell, ExtCellParts, Promise};
use super::{FilterAction, MerkleFilter, MerkleProofBuilder, make_pruned_branch};
use crate::cell::*;
use crate::error::Error;
use crate::util::unlikely;

/// Parsed Merkle update representation.
///
/// NOTE: Serialized into `MerkleUpdate` cell.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MerkleUpdate {
    /// Representation hash of the original cell.
    pub old_hash: HashBytes,
    /// Representation hash of the updated cell.
    pub new_hash: HashBytes,
    /// Representation depth of the original cell.
    pub old_depth: u16,
    /// Representation depth of the updated cell.
    pub new_depth: u16,
    /// Partially pruned tree with unchanged cells of the origin cell.
    #[cfg_attr(feature = "serde", serde(with = "crate::boc::Boc"))]
    pub old: Cell,
    /// Partially pruned tree with all cells that are not in the original cell.
    #[cfg_attr(feature = "serde", serde(with = "crate::boc::Boc"))]
    pub new: Cell,
}

impl Eq for MerkleUpdate {}
impl PartialEq for MerkleUpdate {
    fn eq(&self, other: &Self) -> bool {
        self.old_hash == other.old_hash
            && self.new_hash == other.new_hash
            && self.old_depth == other.old_depth
            && self.new_depth == other.new_depth
            && self.old.as_ref() == other.old.as_ref()
            && self.new.as_ref() == other.new.as_ref()
    }
}

impl Default for MerkleUpdate {
    fn default() -> Self {
        let empty_cell = Cell::empty_cell();
        Self {
            old_hash: *EMPTY_CELL_HASH,
            new_hash: *EMPTY_CELL_HASH,
            old_depth: 0,
            new_depth: 0,
            old: empty_cell.clone(),
            new: empty_cell,
        }
    }
}

impl<'a> LoadCell<'a> for MerkleUpdate {
    fn load_from_cell(cell: &'a DynCell) -> Result<Self, Error> {
        if unlikely(!cell.is_exotic()) {
            return Err(Error::UnexpectedOrdinaryCell);
        }

        let mut s = cell.as_slice_allow_exotic();
        if s.size_bits() != Self::BITS || s.size_refs() != Self::REFS {
            return Err(Error::CellUnderflow);
        }

        if ok!(s.load_u8()) != CellType::MerkleUpdate.to_byte() {
            return Err(Error::InvalidCell);
        }

        let res = Self {
            old_hash: ok!(s.load_u256()),
            new_hash: ok!(s.load_u256()),
            old_depth: ok!(s.load_u16()),
            new_depth: ok!(s.load_u16()),
            old: ok!(s.load_reference_cloned()),
            new: ok!(s.load_reference_cloned()),
        };
        debug_assert!(s.is_data_empty() && s.is_refs_empty());

        if res.old.as_ref().hash(0) == &res.old_hash
            && res.old.as_ref().depth(0) == res.old_depth
            && res.new.as_ref().hash(0) == &res.new_hash
            && res.new.as_ref().depth(0) == res.new_depth
        {
            Ok(res)
        } else {
            Err(Error::InvalidCell)
        }
    }
}

impl Store for MerkleUpdate {
    fn store_into(&self, b: &mut CellBuilder, _: &dyn CellContext) -> Result<(), Error> {
        if !b.has_capacity(Self::BITS, Self::REFS) {
            return Err(Error::CellOverflow);
        }

        b.set_exotic(true);
        ok!(b.store_u8(CellType::MerkleUpdate.to_byte()));
        ok!(b.store_u256(&self.old_hash));
        ok!(b.store_u256(&self.new_hash));
        ok!(b.store_u32(((self.old_depth as u32) << 16) | self.new_depth as u32));
        ok!(b.store_reference(self.old.clone()));
        b.store_reference(self.new.clone())
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for MerkleUpdate {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let old = Cell::arbitrary(u).and_then(crate::arbitrary::check_max_depth)?;
        let new = Cell::arbitrary(u).and_then(crate::arbitrary::check_max_depth)?;
        Ok(Self {
            old_hash: *old.hash(0),
            new_hash: *new.hash(0),
            old_depth: old.depth(0),
            new_depth: new.depth(0),
            old,
            new,
        })
    }

    fn size_hint(_: usize) -> (usize, Option<usize>) {
        (4, None)
    }
}

impl MerkleUpdate {
    /// The number of data bits that the Merkle update occupies.
    pub const BITS: u16 = 8 + (256 + 16) * 2;
    /// The number of references that the Merkle update occupies.
    pub const REFS: u8 = 2;

    /// Starts building a Merkle update between the specified cells,
    /// using old cells determined by filter.
    pub fn create<'a, F>(old: &'a DynCell, new: &'a DynCell, f: F) -> MerkleUpdateBuilder<'a, F>
    where
        F: MerkleFilter + 'a,
    {
        MerkleUpdateBuilder::new(old, new, f)
    }

    /// Tries to apply this Merkle update to the specified cell,
    /// producing a new cell and using an empty cell context.
    pub fn apply(&self, old: &Cell) -> Result<Cell, Error> {
        self.apply_ext_with_stats(old, Cell::empty_context())
            .map(|r| r.cell)
    }

    /// Tries to apply this Merkle update to the specified cell,
    /// producing a new cell with stats and using an empty cell context.
    pub fn apply_with_stats(&self, old: &Cell) -> Result<MerkleApplyResult, Error> {
        self.apply_ext_with_stats(old, Cell::empty_context())
    }

    /// Tries to apply this Merkle update to the specified cell,
    /// producing a new cell with stats and using an empty cell context.
    pub fn apply_ext_with_stats(
        &self,
        old: &Cell,
        context: &dyn CellContext,
    ) -> Result<MerkleApplyResult, Error> {
        if old.as_ref().repr_hash() != &self.old_hash {
            return Err(Error::InvalidData);
        }

        if self.old_hash == self.new_hash {
            return Ok(MerkleApplyResult {
                cell: old.clone(),
                stats: MerkleStats::default(),
            });
        }

        // Collect old cells
        let old_cells = {
            // Collect and check old cells tree
            let old_cell_hashes = ok!(self.find_old_cells());

            let mut visited = ahash::HashSet::default();
            let mut old_cells = ahash::HashMap::default();

            // Insert root
            let mut merkle_depth = 0u8;

            visited.insert(old.repr_hash());
            old_cells.insert(*old.hash(merkle_depth), old.clone());
            merkle_depth += old.descriptor().is_merkle() as u8;
            let mut stack = vec![old.references()];

            'outer: while let Some(iter) = stack.last_mut() {
                let cloned = iter.clone().cloned();
                for (child_ref, child) in std::iter::zip(&mut *iter, cloned) {
                    if !visited.insert(child_ref.repr_hash()) {
                        continue;
                    }

                    let hash = child_ref.hash(merkle_depth);
                    if !old_cell_hashes.contains(hash) {
                        // Skip new cells
                        continue;
                    }

                    // Store an owned cell with original merkle depth
                    old_cells.insert(*hash, child);

                    // Increase the current merkle depth if needed
                    merkle_depth += child_ref.descriptor().is_merkle() as u8;
                    // And proceed to processing this child
                    stack.push(child_ref.references());
                    continue 'outer;
                }

                // Decrease the current merkle depth if needed
                merkle_depth -= iter.cell().descriptor().is_merkle() as u8;
                // And return to the previous depth
                stack.pop();
            }

            old_cells
        };

        // Apply changed cells
        let mut applier = MerkleUpdateApplier::<_, &HashBytes> {
            new_cells: Default::default(),
            context,
            find_cell: move |hash: &HashBytes| old_cells.get(hash).cloned(),
            find_in_new_cells: false,
        };

        let new = applier.run(self.new.as_ref(), 0)?;

        if new.as_ref().repr_hash() == &self.new_hash {
            // Note: +1 for root
            let new_cells_count = applier.new_cells.len() + 1;

            Ok(MerkleApplyResult {
                cell: new,
                stats: MerkleStats { new_cells_count },
            })
        } else {
            Err(Error::InvalidData)
        }
    }

    /// Applies new cell using the specified method to find an existing one.
    ///
    /// Must only be used for trusted merkle updates.
    pub fn apply_trusted<F: FindCell>(&self, find_cell: F) -> Result<Cell, Error> {
        self.apply_trusted_ext_with_stats(find_cell, Cell::empty_context())
            .map(|r| r.cell)
    }

    /// Applies new cell using the specified method to find an existing one.
    ///
    /// Must only be used for trusted merkle updates.
    pub fn apply_trusted_with_stats<F: FindCell>(
        &self,
        find_cell: F,
    ) -> Result<MerkleApplyResult, Error> {
        self.apply_trusted_ext_with_stats(find_cell, Cell::empty_context())
    }

    /// Applies new cell using the specified method to find an existing one.
    ///
    /// Must only be used for trusted merkle updates.
    pub fn apply_trusted_ext_with_stats<F: FindCell>(
        &self,
        find_cell: F,
        context: &dyn CellContext,
    ) -> Result<MerkleApplyResult, Error> {
        if self.old_hash == self.new_hash {
            let Some(cell) = find_cell.find_cell(&self.old_hash) else {
                return Err(Error::InvalidData);
            };
            return Ok(MerkleApplyResult {
                cell,
                stats: Default::default(),
            });
        }

        // Apply changed cells
        let mut applier = MerkleUpdateApplier::<F, &HashBytes> {
            new_cells: Default::default(),
            context,
            find_cell,
            find_in_new_cells: false,
        };

        let new = applier.run(self.new.as_ref(), 0)?;

        if new.as_ref().repr_hash() == &self.new_hash {
            // Note: +1 for root
            let new_cells_count = applier.new_cells.len() + 1;

            Ok(MerkleApplyResult {
                cell: new,
                stats: MerkleStats { new_cells_count },
            })
        } else {
            Err(Error::InvalidData)
        }
    }

    /// Tries to apply Merkle update in parallel
    #[cfg(all(feature = "rayon", feature = "sync"))]
    pub fn par_apply(
        &self,
        old: &Cell,
        old_split_at: &ahash::HashSet<HashBytes>,
    ) -> Result<Cell, Error> {
        self.par_apply_ext_with_stats(old, old_split_at, Cell::empty_context())
            .map(|r| r.cell)
    }

    /// Tries to apply Merkle update in parallel with stats
    #[cfg(all(feature = "rayon", feature = "sync"))]
    pub fn par_apply_with_stats(
        &self,
        old: &Cell,
        old_split_at: &ahash::HashSet<HashBytes>,
    ) -> Result<MerkleApplyResult, Error> {
        self.par_apply_ext_with_stats(old, old_split_at, Cell::empty_context())
    }

    /// Tries to apply Merkle update in parallel with collecting stats
    #[cfg(all(feature = "rayon", feature = "sync"))]
    pub fn par_apply_ext_with_stats(
        &self,
        old: &Cell,
        old_split_at: &ahash::HashSet<HashBytes>,
        context: &(dyn CellContext + Send + Sync),
    ) -> Result<MerkleApplyResult, Error> {
        if old.as_ref().repr_hash() != &self.old_hash {
            return Err(Error::InvalidData);
        }

        if self.old_hash == self.new_hash {
            return Ok(MerkleApplyResult {
                cell: old.clone(),
                stats: MerkleStats::default(),
            });
        }

        // Finds original old cell roots by their hashes.
        #[allow(clippy::too_many_arguments)]
        fn find_old_subtrees<'a, 's>(
            cell_ref: &'a DynCell,
            cell: Cell,
            mut merkle_depth: u8,
            scope: Option<&rayon::Scope<'s>>,
            split_at: &'s ahash::HashSet<HashBytes>,
            visited: &'s dashmap::DashSet<&'a HashBytes, ahash::RandomState>,
            old_cells: &'s dashmap::DashMap<&'a HashBytes, Cell, ahash::RandomState>,
            old_cell_hashes: &'s ahash::HashSet<&'a HashBytes>,
        ) {
            let original_merkle_depth = merkle_depth;

            // Insert root
            if !visited.insert(cell_ref.repr_hash()) {
                return;
            }
            old_cells.insert(cell_ref.hash(merkle_depth), cell);
            merkle_depth += cell_ref.descriptor().is_merkle() as u8;

            let mut stack = vec![cell_ref.references()];
            'outer: while let Some(iter) = stack.last_mut() {
                let cloned = iter.clone().cloned();
                for (cell_ref, cell) in std::iter::zip(&mut *iter, cloned) {
                    if let Some(scope) = scope
                        && split_at.contains(cell.repr_hash())
                    {
                        scope.spawn(move |_| {
                            find_old_subtrees(
                                cell_ref,
                                cell,
                                merkle_depth,
                                None,
                                split_at,
                                visited,
                                old_cells,
                                old_cell_hashes,
                            );
                        });
                        continue;
                    }

                    if !visited.insert(cell_ref.repr_hash()) {
                        continue;
                    }

                    let hash = cell_ref.hash(merkle_depth);
                    if !old_cell_hashes.contains(hash) {
                        // Skip new cells
                        continue;
                    }

                    // Store an owned cell with original merkle depth
                    old_cells.insert(hash, cell);

                    // Increase the current merkle depth if needed
                    merkle_depth += cell_ref.descriptor().is_merkle() as u8;
                    // And proceed to processing this child
                    stack.push(cell_ref.references());
                    continue 'outer;
                }

                // Decrease the current merkle depth if needed
                merkle_depth -= iter.cell().descriptor().is_merkle() as u8;
                // And return to the previous depth
                stack.pop();
            }

            assert_eq!(merkle_depth, original_merkle_depth);
        }

        // Collect old cells
        let old_cells = {
            // Collect and check old cells tree
            let old_cell_hashes = self.par_find_old_cells(old_split_at)?;

            let visited = Default::default();
            let old_cells = Default::default();

            rayon::scope(|scope| {
                find_old_subtrees(
                    old.as_ref(),
                    old.clone(),
                    0,
                    Some(scope),
                    old_split_at,
                    &visited,
                    &old_cells,
                    &old_cell_hashes,
                );
            });

            old_cells
        };

        // Apply changed cells
        let (new, new_cells_count) = {
            let applier = ParMerkleUpdateApplier::<_> {
                new_cells: Default::default(),
                total_new_cells: Default::default(),
                context,
                find_cell: move |hash: &HashBytes| old_cells.get(hash).map(|c| c.clone()),
                find_in_new_cells: false,
            };

            let new = rayon::scope(|scope| applier.run(self.new.as_ref(), 0, 0, Some(scope)))?;
            let cell = new.resolve(context)?;

            // Note: +1 for root
            let count = applier
                .total_new_cells
                .load(Ordering::Relaxed)
                .saturating_add(1);

            (cell, count)
        };

        if new.as_ref().repr_hash() == &self.new_hash {
            Ok(MerkleApplyResult {
                cell: new,
                stats: MerkleStats { new_cells_count },
            })
        } else {
            Err(Error::InvalidData)
        }
    }

    /// Tries to apply Merkle update in parallel
    #[cfg(all(feature = "rayon", feature = "sync"))]
    pub fn par_apply_trusted<F>(&self, find_cell: F) -> Result<Cell, Error>
    where
        F: FindCell + Send + Sync + 'static,
    {
        self.par_apply_trusted_ext_with_stats(find_cell, Cell::empty_context())
            .map(|r| r.cell)
    }

    /// Tries to apply Merkle update in parallel with stats
    #[cfg(all(feature = "rayon", feature = "sync"))]
    pub fn par_apply_trusted_with_stats<F>(&self, find_cell: F) -> Result<MerkleApplyResult, Error>
    where
        F: FindCell + Send + Sync + 'static,
    {
        self.par_apply_trusted_ext_with_stats(find_cell, Cell::empty_context())
    }

    /// Tries to apply Merkle update in parallel with collecting stats
    #[cfg(all(feature = "rayon", feature = "sync"))]
    pub fn par_apply_trusted_ext_with_stats<F>(
        &self,
        find_cell: F,
        context: &(dyn CellContext + Send + Sync),
    ) -> Result<MerkleApplyResult, Error>
    where
        F: FindCell + Send + Sync + 'static,
    {
        if self.old_hash == self.new_hash {
            let Some(cell) = find_cell.find_cell(&self.old_hash) else {
                return Err(Error::InvalidData);
            };
            return Ok(MerkleApplyResult {
                cell,
                stats: Default::default(),
            });
        }

        // Apply changed cells
        let (new, new_cells_count) = {
            let applier = ParMerkleUpdateApplier {
                new_cells: Default::default(),
                total_new_cells: Default::default(),
                context,
                find_cell,
                find_in_new_cells: false,
            };

            let new = rayon::scope(|scope| applier.run(self.new.as_ref(), 0, 0, Some(scope)))?;
            let cell = new.resolve(context)?;

            // Note: +1 for root
            let count = applier
                .total_new_cells
                .load(Ordering::Relaxed)
                .saturating_add(1);

            (cell, count)
        };

        if new.as_ref().repr_hash() == &self.new_hash {
            Ok(MerkleApplyResult {
                cell: new,
                stats: MerkleStats { new_cells_count },
            })
        } else {
            Err(Error::InvalidData)
        }
    }

    /// Computes the removed cells diff using the original cell.
    pub fn compute_removed_cells<'a>(
        &self,
        old: &'a DynCell,
    ) -> Result<ahash::HashMap<&'a HashBytes, u32>, Error> {
        use std::collections::hash_map;

        if old.repr_hash() != &self.old_hash || self.old.hash(0) != old.repr_hash() {
            return Err(Error::InvalidData);
        }

        if self.old_hash == self.new_hash {
            // No cells were removed
            return Ok(Default::default());
        }

        let mut new_cells = ahash::HashSet::default();

        // Compute a list of all hashes in the `new` merkle update tree
        {
            // TODO: check if `new_cells` set can be used instead of `visited`
            let mut visited = ahash::HashSet::default();
            let mut merkle_depth = self.new.descriptor().is_merkle() as u8;
            let mut stack = vec![self.new.references()];

            visited.insert(self.new.repr_hash());
            new_cells.insert(self.new.hash(0));

            'outer: while let Some(iter) = stack.last_mut() {
                for child in &mut *iter {
                    if !visited.insert(child.repr_hash()) {
                        continue;
                    }

                    // Track new cells
                    new_cells.insert(child.hash(merkle_depth));

                    // Unchanged cells (as pruned branches) must be presented in the old tree
                    let descriptor = child.descriptor();
                    if descriptor.is_pruned_branch() {
                        continue;
                    }

                    // Increase the current merkle depth if needed
                    merkle_depth += descriptor.is_merkle() as u8;
                    // And proceed to processing this child
                    stack.push(child.references());
                    continue 'outer;
                }

                merkle_depth -= iter.cell().descriptor().is_merkle() as u8;
                stack.pop();
            }

            debug_assert_eq!(merkle_depth, 0);
        }

        // Traverse old cells
        let mut result = ahash::HashMap::default();
        result.insert(old.repr_hash(), 1);

        let mut stack = Vec::new();
        if !new_cells.contains(old.repr_hash()) {
            stack.push(old.references());
        }

        'outer: while let Some(iter) = stack.last_mut() {
            for child in &mut *iter {
                let hash = child.repr_hash();
                match result.entry(hash) {
                    hash_map::Entry::Occupied(mut entry) => {
                        *entry.get_mut() += 1;
                        continue;
                    }
                    hash_map::Entry::Vacant(entry) => {
                        entry.insert(1);
                    }
                }

                // Skip empty or used subtrees
                if child.reference_count() == 0 || new_cells.contains(hash) {
                    continue;
                }

                stack.push(child.references());
                continue 'outer;
            }
            stack.pop();
        }

        Ok(result)
    }

    fn find_old_cells(&self) -> Result<ahash::HashSet<&HashBytes>, Error> {
        let mut visited = ahash::HashSet::default();
        let mut old_cells = ahash::HashSet::default();

        // Traverse old cells
        let mut merkle_depth = 0u8;

        // Insert root
        visited.insert(self.old.repr_hash());
        old_cells.insert(self.old.hash(merkle_depth));
        merkle_depth += self.old.descriptor().is_merkle() as u8;
        let mut stack = vec![self.old.references()];

        'outer: while let Some(iter) = stack.last_mut() {
            for child in &mut *iter {
                if !visited.insert(child.repr_hash()) {
                    continue;
                }

                // Store cell with original merkle depth
                old_cells.insert(child.hash(merkle_depth));

                // Skip children for pruned branches
                let descriptor = child.descriptor();
                if descriptor.is_pruned_branch() {
                    continue;
                }

                // Increase the current merkle depth if needed
                merkle_depth += descriptor.is_merkle() as u8;
                // And proceed to processing this child
                stack.push(child.references());
                continue 'outer;
            }

            // Decrease the current merkle depth if needed
            merkle_depth -= iter.cell().descriptor().is_merkle() as u8;
            // And return to the previous depth
            stack.pop();
        }

        debug_assert_eq!(merkle_depth, 0);

        // Traverse new cells

        // Insert root
        visited.clear();
        visited.insert(self.new.repr_hash());
        stack.push(self.new.references());
        merkle_depth += self.new.descriptor().is_merkle() as u8;

        'outer: while let Some(iter) = stack.last_mut() {
            for child in &mut *iter {
                // Skip visited cells
                if !visited.insert(child.repr_hash()) {
                    continue;
                }

                // Unchanged cells (as pruned branches) must be presented in the old tree
                let descriptor = child.descriptor();
                if descriptor.is_pruned_branch() {
                    if descriptor.level_mask().level() == merkle_depth + 1
                        && !old_cells.contains(child.hash(merkle_depth))
                    {
                        return Err(Error::InvalidData);
                    }
                } else {
                    // Increase the current merkle depth if needed
                    merkle_depth += descriptor.is_merkle() as u8;
                    // And proceed to processing this child
                    stack.push(child.references());
                    continue 'outer;
                }
            }

            // Decrease the current merkle depth if needed
            merkle_depth -= iter.cell().descriptor().is_merkle() as u8;
            // And return to the previous depth
            stack.pop();
        }

        debug_assert_eq!(merkle_depth, 0);

        // Done
        Ok(old_cells)
    }

    #[cfg(all(feature = "rayon", feature = "sync"))]
    fn par_find_old_cells<'a, 's: 'a>(
        &'a self,
        split_at: &'s ahash::HashSet<HashBytes>,
    ) -> Result<ahash::HashSet<&'a HashBytes>, Error> {
        fn traverse_old_cells<'a, 's>(
            cell: &'a DynCell,
            mut merkle_depth: u8,
            scope: Option<&rayon::Scope<'s>>,
            split_at: &'s ahash::HashSet<HashBytes>,
            visited: &'s dashmap::DashSet<&'a HashBytes, ahash::RandomState>,
            old_cells: &'s dashmap::DashSet<&'a HashBytes, ahash::RandomState>,
        ) {
            if !visited.insert(cell.repr_hash()) {
                return;
            }
            old_cells.insert(cell.hash(merkle_depth));

            let original_merkle_depth = merkle_depth;
            merkle_depth += cell.descriptor().is_merkle() as u8;
            let mut stack = vec![cell.references()];

            'outer: while let Some(iter) = stack.last_mut() {
                for child in &mut *iter {
                    let child_hash = child.repr_hash();

                    if let Some(scope) = scope
                        && split_at.contains(child_hash)
                    {
                        scope.spawn(move |_| {
                            traverse_old_cells(
                                child,
                                merkle_depth,
                                None,
                                split_at,
                                visited,
                                old_cells,
                            );
                        });
                        continue;
                    }

                    if !visited.insert(child.repr_hash()) {
                        continue;
                    }

                    // Store cell with original merkle depth
                    old_cells.insert(child.hash(merkle_depth));

                    // Skip children for pruned branches
                    let descriptor = child.descriptor();
                    if descriptor.is_pruned_branch() {
                        continue;
                    }

                    // Increase the current merkle depth if needed
                    merkle_depth += descriptor.is_merkle() as u8;
                    // And proceed to processing this child
                    stack.push(child.references());
                    continue 'outer;
                }

                // Decrease the current merkle depth if needed
                merkle_depth -= iter.cell().descriptor().is_merkle() as u8;
                // And return to the previous depth
                stack.pop();
            }

            debug_assert_eq!(merkle_depth, original_merkle_depth);
        }

        // TODO: Reuse stack from `traverse_old_cells`
        fn traverse_new_cells<'a, 's>(
            cell: &'a DynCell,
            mut merkle_depth: u8,
            scope: Option<&rayon::Scope<'s>>,
            visited: &'s dashmap::DashSet<&'a HashBytes, ahash::RandomState>,
            old_cells: &'s dashmap::DashSet<&'a HashBytes, ahash::RandomState>,
            is_invalid: &'s AtomicBool,
        ) {
            if !visited.insert(cell.repr_hash()) {
                return;
            }

            let original_merkle_depth = merkle_depth;

            let check_invalid = |descriptor: CellDescriptor, cell: &DynCell, merkle_depth: u8| {
                descriptor.level_mask().level() == merkle_depth + 1
                    && !old_cells.contains(cell.hash(merkle_depth))
            };

            let descriptor = cell.descriptor();
            if descriptor.is_pruned_branch() {
                if check_invalid(descriptor, cell, merkle_depth) {
                    is_invalid.store(true, Ordering::Relaxed);
                }
                return;
            } else {
                merkle_depth += descriptor.is_merkle() as u8;
            }

            let mut stack = vec![cell.references()];
            let mut traverse_depth = 1usize;
            'outer: while let Some(iter) = stack.last_mut() {
                for child in &mut *iter {
                    if let Some(scope) = scope
                        && traverse_depth > ROOT_SPLIT_DEPTH as usize
                        && child.repr_depth() > CHILD_SPLIT_DEPTH
                    {
                        scope.spawn(move |_| {
                            traverse_new_cells(
                                child,
                                merkle_depth,
                                None,
                                visited,
                                old_cells,
                                is_invalid,
                            );
                        });
                        continue;
                    }

                    // Skip visited cells
                    if !visited.insert(child.repr_hash()) {
                        continue;
                    }

                    // Unchanged cells (as pruned branches) must be presented in the old tree
                    let descriptor = child.descriptor();
                    if descriptor.is_pruned_branch() {
                        if unlikely(check_invalid(descriptor, child, merkle_depth)) {
                            is_invalid.store(true, Ordering::Release);
                            return;
                        }
                    } else {
                        // Increase the current merkle depth if needed
                        merkle_depth += descriptor.is_merkle() as u8;
                        // And proceed to processing this child
                        stack.push(child.references());
                        traverse_depth += 1;
                        continue 'outer;
                    }
                }

                // Decrease the current merkle depth if needed
                merkle_depth -= iter.cell().descriptor().is_merkle() as u8;
                traverse_depth -= 1;
                // And return to the previous depth
                stack.pop();
            }

            debug_assert_eq!(merkle_depth, original_merkle_depth);
        }

        let visited = Default::default();
        let old_cells = Default::default();

        rayon::scope(|scope| {
            traverse_old_cells(
                self.old.as_ref(),
                0,
                Some(scope),
                split_at,
                &visited,
                &old_cells,
            );
        });

        visited.clear();

        let mut is_invalid = AtomicBool::new(false);
        rayon::scope(|scope| {
            traverse_new_cells(
                self.new.as_ref(),
                0,
                Some(scope),
                &visited,
                &old_cells,
                &is_invalid,
            );
        });

        if *is_invalid.get_mut() {
            Err(Error::InvalidData)
        } else {
            Ok(old_cells.into_iter().collect())
        }
    }
}

/// A key type for [`MerkleUpdateApplier`].
pub trait MerkleUpdateApplierKey<'a>: Borrow<HashBytes> + Eq + std::hash::Hash {
    /// Make key from cell hash.
    fn from_ref(hash: &'a HashBytes) -> Self;
}

impl MerkleUpdateApplierKey<'_> for HashBytes {
    #[inline]
    fn from_ref(hash: &'_ HashBytes) -> Self {
        *hash
    }
}

impl<'a> MerkleUpdateApplierKey<'a> for &'a HashBytes {
    #[inline]
    fn from_ref(hash: &'a HashBytes) -> Self {
        hash
    }
}

/// Some storage that can provide cells by their repr hash.
pub trait FindCell {
    /// Find cell by its repr hash.
    fn find_cell(&self, hash: &HashBytes) -> Option<Cell>;
}

impl<F> FindCell for F
where
    for<'a> F: Fn(&'a HashBytes) -> Option<Cell>,
{
    #[inline]
    fn find_cell(&self, hash: &HashBytes) -> Option<Cell> {
        (self)(hash)
    }
}

/// Stateful applier which rebuilds a `new` cell with existing subtrees.
pub struct MerkleUpdateApplier<'c, F, K> {
    /// Rebuilt cells.
    pub new_cells: ahash::HashMap<K, Cell>,
    /// Cell builder context.
    pub context: &'c dyn CellContext,
    /// How to find an existing cell from the previous tree.
    pub find_cell: F,
    /// Whether to reuse new cells if not found in previous tree.
    pub find_in_new_cells: bool,
}

impl<'c, F: FindCell, K> MerkleUpdateApplier<'c, F, K> {
    /// Rebuilds the specified cell so that all its pruned subtrees
    /// are replaced with old subtrees.
    pub fn run<'a>(&mut self, cell: &'a DynCell, merkle_depth: u8) -> Result<Cell, Error>
    where
        K: MerkleUpdateApplierKey<'a>,
    {
        let descriptor = cell.descriptor();
        let child_merkle_depth = merkle_depth + descriptor.cell_type().is_merkle() as u8;

        // Start building a new cell
        let mut result = CellBuilder::new();
        result.set_exotic(descriptor.is_exotic());

        // Build all child cells
        for child_idx in 0..descriptor.reference_count() {
            let Some(child) = cell.reference(child_idx) else {
                return Err(Error::CellUnderflow);
            };
            let child_descriptor = child.descriptor();

            let child = if child_descriptor.is_pruned_branch() {
                // Replace pruned branches with old cells
                let mask = child_descriptor.level_mask();
                if mask.to_byte() & (1 << child_merkle_depth) != 0 {
                    // Use original hash for pruned branches
                    let child_hash = child.hash(mask.level() - 1);

                    if self.find_in_new_cells
                        && let Some(cell) = self.new_cells.get(child_hash)
                    {
                        cell.clone()
                    } else if let Some(cell) = self.find_cell.find_cell(child_hash) {
                        if self.find_in_new_cells {
                            self.new_cells.insert(K::from_ref(child_hash), cell.clone());
                        }
                        cell
                    } else {
                        return Err(Error::InvalidData);
                    }
                } else {
                    match cell.reference_cloned(child_idx) {
                        Some(child) => child,
                        None => return Err(Error::CellUnderflow),
                    }
                }
            } else {
                // Build a child cell if it hasn't been built before
                let child_hash = child.hash(child_merkle_depth);
                if let Some(child) = self.new_cells.get(child_hash) {
                    child.clone()
                } else {
                    let child = ok!(self.run(child.as_ref(), child_merkle_depth));
                    self.new_cells
                        .insert(K::from_ref(child_hash), child.clone());
                    child
                }
            };

            _ = result.store_reference(child);
        }

        _ = result.store_cell_data(cell);

        result.build_ext(self.context)
    }
}

/// Stateful parallel applier which rebuilds a `new` cell with existing subtrees.
#[cfg(all(feature = "rayon", feature = "sync"))]
pub struct ParMerkleUpdateApplier<'c, F> {
    /// Rebuilt cells.
    pub new_cells: dashmap::DashMap<HashBytes, ExtCell, ahash::RandomState>,
    /// Total number of new cells.
    pub total_new_cells: AtomicUsize,
    /// Cell builder context.
    pub context: &'c (dyn CellContext + Send + Sync),
    /// How to find an existing cell from the previous tree.
    pub find_cell: F,
    /// Whether to reuse new cells if not found in previous tree.
    pub find_in_new_cells: bool,
}

#[cfg(all(feature = "rayon", feature = "sync"))]
impl<F: FindCell + Send + Sync> ParMerkleUpdateApplier<'_, F> {
    // TODO: Replace with a non-recursive impl
    /// Rebuilds the specified cell so that all its pruned subtrees
    /// are replaced with old subtrees.
    pub fn run<'scope>(
        &'scope self,
        cell: &DynCell,
        merkle_depth: u8,
        traverse_depth: u16,
        scope: Option<&rayon::Scope<'scope>>,
    ) -> Result<ExtCell, Error> {
        let mut scope_total = 0;
        let res = self.run_inner(cell, merkle_depth, traverse_depth, &mut scope_total, scope);
        self.total_new_cells
            .fetch_add(scope_total, Ordering::Relaxed);
        res
    }

    fn run_inner<'scope>(
        &'scope self,
        cell: &DynCell,
        merkle_depth: u8,
        traverse_depth: u16,
        scope_total: &mut usize,
        scope: Option<&rayon::Scope<'scope>>,
    ) -> Result<ExtCell, Error> {
        let descriptor = cell.descriptor();
        let child_merkle_depth = merkle_depth + descriptor.cell_type().is_merkle() as u8;

        // Start building a new cell
        let mut builder = ChildrenBuilder::Ordinary(Default::default());

        // Build all child cells
        for child in cell.references().cloned() {
            let child_descriptor = child.as_ref().descriptor();

            let child = ok!(if child_descriptor.is_pruned_branch() {
                self.resolve_pruned_branch(child, child_descriptor, child_merkle_depth)
            } else {
                self.resolve_subtree(
                    child,
                    child_merkle_depth,
                    traverse_depth,
                    &mut *scope_total,
                    scope,
                )
            });

            ok!(builder.store_reference(child));
        }

        // Build the cell
        match builder {
            ChildrenBuilder::Ordinary(refs) => {
                let mut builder = CellBuilder::new();
                builder.set_exotic(cell.is_exotic());
                _ = builder.store_cell_data(cell);
                builder.set_references(refs);

                builder.build_ext(self.context).map(ExtCell::Ordinary)
            }
            ChildrenBuilder::Extended(refs) => {
                let mut builder = CellDataBuilder::new();
                builder.store_cell_data(cell)?;
                Ok(ExtCell::Partial(Arc::new(ExtCellParts {
                    data: builder,
                    is_exotic: cell.is_exotic(),
                    refs,
                })))
            }
        }
    }

    // Replace pruned branches with old cells
    fn resolve_pruned_branch(
        &self,
        cell: Cell,
        descriptor: CellDescriptor,
        merkle_depth: u8,
    ) -> Result<ExtCell, Error> {
        let mask = descriptor.level_mask();
        if mask.to_byte() & (1 << merkle_depth) != 0 {
            // Use original hash for pruned branches
            let child_hash = cell.as_ref().hash(mask.level() - 1);

            if self.find_in_new_cells
                && let Some(cell) = self.new_cells.get(child_hash)
            {
                return Ok(cell.clone());
            }

            match self.find_cell.find_cell(child_hash) {
                Some(cell) => {
                    if self.find_in_new_cells {
                        self.new_cells
                            .insert(*child_hash, ExtCell::Ordinary(cell.clone()));
                    }
                    Ok(ExtCell::Ordinary(cell))
                }
                None => Err(Error::InvalidData),
            }
        } else {
            Ok(ExtCell::Ordinary(cell))
        }
    }

    // Build a child cell if it hasn't been built before
    fn resolve_subtree<'scope>(
        &'scope self,
        cell: Cell,
        merkle_depth: u8,
        traverse_depth: u16,
        scope_total_new: &mut usize,
        scope: Option<&rayon::Scope<'scope>>,
    ) -> Result<ExtCell, Error> {
        let hash = cell.as_ref().hash(merkle_depth);

        if let Some(child) = self.new_cells.get(hash) {
            return Ok(child.clone());
        }

        if let Some(scope) = scope
            && traverse_depth > ROOT_SPLIT_DEPTH
            && cell.repr_depth() > CHILD_SPLIT_DEPTH
        {
            let cell = match self.new_cells.entry(*hash) {
                dashmap::Entry::Occupied(entry) => entry.get().clone(),
                dashmap::Entry::Vacant(entry) => {
                    let promise = Promise::new();
                    scope.spawn({
                        let promise = promise.clone();
                        move |_| {
                            let result =
                                self.run(cell.as_ref(), merkle_depth, traverse_depth + 1, None);
                            promise.set(result);
                        }
                    });
                    let cell = ExtCell::Deferred(promise);
                    *scope_total_new += 1;
                    entry.insert(cell.clone());
                    cell
                }
            };
            return Ok(cell);
        }

        let cell = ok!(self.run_inner(
            cell.as_ref(),
            merkle_depth,
            traverse_depth + 1,
            &mut *scope_total_new,
            scope
        ));
        *scope_total_new += self.new_cells.insert(*hash, cell.clone()).is_none() as usize;
        Ok(cell)
    }
}

/// Metadata collected during `MerkleUpdate` operations.
#[derive(Debug, Clone, Copy, Default)]
pub struct MerkleStats {
    /// Number of new (not pruned) cells.
    pub new_cells_count: usize,
}

/// Result of applying a Merkle update with metadata.
#[derive(Debug, Clone)]
pub struct MerkleApplyResult {
    /// The new cell.
    pub cell: Cell,
    /// Metadata.
    pub stats: MerkleStats,
}

/// Result of building a Merkle update with metadata.
#[derive(Debug, Clone)]
pub struct MerkleBuildResult {
    /// The Merkle update.
    pub update: MerkleUpdate,
    /// Metadata.
    pub stats: MerkleStats,
}

/// Helper struct to build a Merkle update.
pub struct MerkleUpdateBuilder<'a, F> {
    old: &'a DynCell,
    new: &'a DynCell,
    filter: F,
}

impl<'a, F> MerkleUpdateBuilder<'a, F>
where
    F: MerkleFilter,
{
    /// Creates a new Merkle update between the specified cells,
    /// using old cells determined by filter.
    pub fn new(old: &'a DynCell, new: &'a DynCell, f: F) -> Self {
        Self {
            old,
            new,
            filter: f,
        }
    }

    /// Builds a Merkle update with stats using the specified cell context.
    pub fn build_ext_with_stats(
        self,
        context: &dyn CellContext,
    ) -> Result<MerkleBuildResult, Error> {
        BuilderImpl {
            old: self.old,
            new: self.new,
            filter: &self.filter,
            context,
        }
        .build()
    }

    /// Builds a Merkle update using an empty cell context.
    pub fn build(self) -> Result<MerkleUpdate, Error> {
        self.build_ext_with_stats(Cell::empty_context())
            .map(|r| r.update)
    }

    /// Builds a Merkle update with stats using an empty cell context.
    pub fn build_with_stats(self) -> Result<MerkleBuildResult, Error> {
        self.build_ext_with_stats(Cell::empty_context())
    }
}

#[cfg(all(feature = "rayon", feature = "sync"))]
impl<'a, F> MerkleUpdateBuilder<'a, F>
where
    F: MerkleFilter + Send + Sync,
{
    /// Multithread build of a Merkle update with stats using the specified cell context
    /// and sets of cells which to handle in parallel.
    pub fn par_build_ext_with_stats(
        self,
        old_split_at: ahash::HashSet<HashBytes>,
        new_split_at: ahash::HashSet<HashBytes>,
        context: &(dyn CellContext + Send + Sync),
    ) -> Result<MerkleBuildResult, Error> {
        ParBuilderImpl {
            old: self.old,
            new: self.new,
            filter: &self.filter,
            context,
        }
        .build(old_split_at, new_split_at)
    }

    /// Multithread build of a Merkle update using the default cell context
    /// and sets of cells which to handle in parallel.
    pub fn par_build(
        self,
        old_split_at: ahash::HashSet<HashBytes>,
        new_split_at: ahash::HashSet<HashBytes>,
    ) -> Result<MerkleUpdate, Error> {
        self.par_build_ext_with_stats(old_split_at, new_split_at, Cell::empty_context())
            .map(|r| r.update)
    }

    /// Multithread build of a Merkle update with stats using the default cell context
    /// and sets of cells which to handle in parallel.
    pub fn par_build_with_stats(
        self,
        old_split_at: ahash::HashSet<HashBytes>,
        new_split_at: ahash::HashSet<HashBytes>,
    ) -> Result<MerkleBuildResult, Error> {
        self.par_build_ext_with_stats(old_split_at, new_split_at, Cell::empty_context())
    }
}

struct BuilderImpl<'a, 'b, 'c: 'a> {
    old: &'a DynCell,
    new: &'a DynCell,
    filter: &'b dyn MerkleFilter,
    context: &'c dyn CellContext,
}

impl<'a: 'b, 'b, 'c: 'a> BuilderImpl<'a, 'b, 'c> {
    fn build(self) -> Result<MerkleBuildResult, Error> {
        struct Resolver<'a> {
            pruned_branches: ahash::HashSet<&'a HashBytes>,
            visited: ahash::HashMap<&'a HashBytes, bool>,
            filter: &'a dyn MerkleFilter,
            changed_cells: ahash::HashSet<&'a HashBytes>,
        }

        impl<'a> Resolver<'a> {
            fn fill(&mut self, cell: &'a DynCell, mut skip_filter: bool) -> bool {
                let repr_hash = cell.repr_hash();

                // Skip visited cells
                if let Some(&result) = self.visited.get(repr_hash) {
                    return result;
                }

                let is_pruned = self.pruned_branches.contains(repr_hash);
                let process_children = if skip_filter {
                    true
                } else {
                    match self.filter.check(repr_hash) {
                        FilterAction::Skip => false,
                        FilterAction::Include => true,
                        FilterAction::IncludeSubtree => {
                            skip_filter = true;
                            true
                        }
                    }
                };

                let mut result = false;
                if process_children {
                    for child in cell.references() {
                        result |= self.fill(child, skip_filter);
                    }

                    if result {
                        self.changed_cells.insert(repr_hash);
                    }
                }

                result |= is_pruned;

                self.visited.insert(repr_hash, result);

                result
            }
        }

        struct InvertedFilter<F>(F);

        impl<F: MerkleFilter> MerkleFilter for InvertedFilter<F> {
            #[inline]
            fn check(&self, cell: &HashBytes) -> FilterAction {
                if self.0.check(cell) == FilterAction::Skip {
                    // TODO: check if FilterAction::IncludeSubtree is correct,
                    // because it is more optimal to just include the new subtree
                    FilterAction::Include
                } else {
                    FilterAction::Skip
                }
            }
        }

        let old_hash = self.old.repr_hash();
        let old_depth = self.old.repr_depth();
        let new_hash = self.new.repr_hash();
        let new_depth = self.new.repr_depth();

        // Handle the simplest case with empty Merkle update
        if old_hash == new_hash {
            let pruned = ok!(make_pruned_branch(self.old, 0, self.context));
            return Ok(MerkleBuildResult {
                update: MerkleUpdate {
                    old_hash: *old_hash,
                    new_hash: *old_hash,
                    old_depth,
                    new_depth: old_depth,
                    old: pruned.clone(),
                    new: pruned,
                },
                stats: MerkleStats::default(),
            });
        }

        // Create Merkle proof cell which contains only new cells
        let mut new_cells_count = 0usize;
        let (new, pruned_branches) = ok! {
            MerkleProofBuilder::<_>::new(
                self.new,
                InvertedFilter(self.filter)
            )
            .track_pruned_branches()
            .allow_different_root(true)
            .build_raw_ext(self.context, |new_cells| new_cells_count = new_cells.len())
        };

        let changed_cells = {
            // Prepare cell diff resolver
            let mut resolver = Resolver {
                pruned_branches,
                visited: Default::default(),
                filter: self.filter,
                changed_cells: Default::default(),
            };

            // Find all changed cells in the old cell tree
            if resolver.fill(self.old, false) {
                resolver.changed_cells.insert(old_hash);
            }

            resolver.changed_cells
        };

        // Create Merkle proof cell which contains only changed cells
        let old = ok! {
            MerkleProofBuilder::<_>::new(self.old, changed_cells)
                .allow_different_root(true)
                .build_raw_ext(self.context)
        };

        // Done
        Ok(MerkleBuildResult {
            update: MerkleUpdate {
                old_hash: *old_hash,
                new_hash: *new_hash,
                old_depth,
                new_depth,
                old,
                new,
            },
            stats: MerkleStats { new_cells_count },
        })
    }
}

#[cfg(all(feature = "rayon", feature = "sync"))]
struct ParBuilderImpl<'a, 'b, 'c: 'a> {
    old: &'a DynCell,
    new: &'a DynCell,
    filter: &'b (dyn MerkleFilter + Send + Sync),
    context: &'c (dyn CellContext + Send + Sync),
}

#[cfg(all(feature = "rayon", feature = "sync"))]
impl<'a: 'b, 'b, 'c: 'a> ParBuilderImpl<'a, 'b, 'c> {
    fn build(
        self,
        old_split_at: ahash::HashSet<HashBytes>,
        new_split_at: ahash::HashSet<HashBytes>,
    ) -> Result<MerkleBuildResult, Error> {
        enum CheckResult<'a> {
            Immediate(bool),
            Parts {
                hash: &'a HashBytes,
                items: Vec<CheckResult<'a>>,
                is_pruned: bool,
            },
            Deferred(Promise<bool>),
        }

        struct Resolver<'a> {
            pruned_branches: dashmap::DashSet<&'a HashBytes, ahash::RandomState>,
            changed_cells: dashmap::DashSet<&'a HashBytes, ahash::RandomState>,
            deferred: dashmap::DashMap<&'a HashBytes, Promise<bool>, ahash::RandomState>,
            filter: &'a (dyn MerkleFilter + Send + Sync),
        }

        impl<'a> Resolver<'a> {
            fn fill<'s>(&self, cell: &'a DynCell, split_at: &'s ahash::HashSet<HashBytes>) -> bool {
                let result = rayon::scope(|scope| {
                    SubResolver {
                        resolver: self,
                        split_at,
                        visited: Default::default(),
                    }
                    .fill_impl(cell, false, Some(scope))
                });
                self.finalize(result)
            }

            fn finalize(&self, result: CheckResult<'a>) -> bool {
                match result {
                    CheckResult::Immediate(result) => result,
                    CheckResult::Parts {
                        hash,
                        items,
                        is_pruned,
                    } => {
                        let mut result = false;
                        for item in items {
                            result |= self.finalize(item);
                        }

                        if result {
                            self.changed_cells.insert(hash);
                        }

                        result | is_pruned
                    }
                    CheckResult::Deferred(promise) => promise.wait_cloned(),
                }
            }
        }

        struct SubResolver<'a, 's, 'r> {
            resolver: &'r Resolver<'a>,
            split_at: &'s ahash::HashSet<HashBytes>,
            visited: ahash::HashMap<&'a HashBytes, bool>,
        }

        impl<'a, 's, 'r> SubResolver<'a, 's, 'r> {
            fn fill_impl<'scope>(
                &mut self,
                cell: &'a DynCell,
                mut skip_filter: bool,
                scope: Option<&rayon::Scope<'scope>>,
            ) -> CheckResult<'a>
            where
                'a: 'scope,
                's: 'scope,
                'r: 'scope,
            {
                let repr_hash = cell.repr_hash();

                // Skip visited cells
                if let Some(&result) = self.visited.get(repr_hash) {
                    return CheckResult::Immediate(result);
                }

                let is_pruned = self.resolver.pruned_branches.contains(repr_hash);
                let process_children = if skip_filter {
                    true
                } else {
                    match self.resolver.filter.check(repr_hash) {
                        FilterAction::Skip => false,
                        FilterAction::Include => true,
                        FilterAction::IncludeSubtree => {
                            skip_filter = true;
                            true
                        }
                    }
                };

                if !process_children {
                    self.visited.insert(repr_hash, is_pruned);
                    return CheckResult::Immediate(is_pruned);
                }

                if let Some(scope) = scope {
                    #[allow(clippy::collapsible_if)]
                    if unlikely(self.split_at.contains(repr_hash)) {
                        let entry = match self.resolver.deferred.entry(repr_hash) {
                            dashmap::Entry::Occupied(entry) => {
                                return CheckResult::Deferred(entry.get().clone());
                            }
                            dashmap::Entry::Vacant(entry) => entry,
                        };

                        let promise = Promise::new();
                        scope.spawn({
                            let promise = promise.clone();
                            let resolver = self.resolver;
                            let split_at = self.split_at;
                            move |_| {
                                let CheckResult::Immediate(changed) = SubResolver {
                                    resolver,
                                    split_at,
                                    visited: Default::default(),
                                }
                                .process_children_impl(cell, skip_filter, is_pruned, None) else {
                                    unreachable!("deferred tasks will not spawn");
                                };
                                promise.set(changed)
                            }
                        });
                        return CheckResult::Deferred(entry.insert_entry(promise).get().clone());
                    }
                }

                let result = self.process_children_impl(cell, skip_filter, is_pruned, scope);

                // Mark as visited only finished cells
                if let CheckResult::Immediate(has_changed) = result {
                    self.visited.insert(repr_hash, has_changed);
                }

                result
            }

            #[inline]
            fn process_children_impl<'scope>(
                &mut self,
                cell: &'a DynCell,
                skip_filter: bool,
                is_pruned: bool,
                scope: Option<&rayon::Scope<'scope>>,
            ) -> CheckResult<'a>
            where
                'a: 'scope,
                's: 'scope,
                'r: 'scope,
            {
                let cell_hash = cell.repr_hash();
                let mut children = cell.references();

                let mut items = 'immediate: {
                    let mut result = false;
                    for child in children.by_ref() {
                        match self.fill_impl(child, skip_filter, scope) {
                            CheckResult::Immediate(res) => {
                                result |= res;
                            }
                            res => break 'immediate vec![CheckResult::Immediate(result), res],
                        }
                    }

                    if result {
                        self.resolver.changed_cells.insert(cell_hash);
                    }

                    return CheckResult::Immediate(result | is_pruned);
                };

                for child in children {
                    items.push(self.fill_impl(child, skip_filter, scope));
                }

                CheckResult::Parts {
                    hash: cell_hash,
                    items,
                    is_pruned,
                }
            }
        }

        struct InvertedFilter<F>(F);

        impl<F: MerkleFilter + Send + Sync> MerkleFilter for InvertedFilter<F> {
            #[inline]
            fn check(&self, cell: &HashBytes) -> FilterAction {
                if self.0.check(cell) == FilterAction::Skip {
                    // TODO: check if FilterAction::IncludeSubtree is correct,
                    // because it is more optimal to just include the new subtree
                    FilterAction::Include
                } else {
                    FilterAction::Skip
                }
            }
        }

        let old_hash = self.old.repr_hash();
        let old_depth = self.old.repr_depth();
        let new_hash = self.new.repr_hash();
        let new_depth = self.new.repr_depth();

        // Handle the simplest case with empty Merkle update
        if old_hash == new_hash {
            let pruned = ok!(make_pruned_branch(self.old, 0, self.context));
            return Ok(MerkleBuildResult {
                update: MerkleUpdate {
                    old_hash: *old_hash,
                    new_hash: *old_hash,
                    old_depth,
                    new_depth: old_depth,
                    old: pruned.clone(),
                    new: pruned,
                },
                stats: MerkleStats::default(),
            });
        }

        // Create Merkle proof cell which contains only new cells
        let mut new_cells_count = 0usize;
        let (new, pruned_branches) = ok! {
            MerkleProofBuilder::<_>::new(
                self.new,
                InvertedFilter(self.filter)
            )
            .track_pruned_branches()
            .allow_different_root(true)
            .par_build_raw_ext(self.context, new_split_at, |new_cells| new_cells_count = new_cells.len())
        };

        // Prepare cell diff resolver and find all changed cells in the old cell tree.
        let changed_cells = {
            let resolver = Resolver {
                pruned_branches,
                deferred: Default::default(),
                changed_cells: Default::default(),
                filter: self.filter,
            };

            // Find all changed cells in the old cell tree
            if resolver.fill(self.old, &old_split_at) {
                resolver.changed_cells.insert(old_hash);
            }

            resolver.changed_cells
        };

        // Create Merkle proof cell which contains only changed cells
        let old = ok! {
            MerkleProofBuilder::<_>::new(self.old, changed_cells)
                .allow_different_root(true)
                .par_build_raw_ext(self.context, old_split_at)
        };

        // Done
        Ok(MerkleBuildResult {
            update: MerkleUpdate {
                old_hash: *old_hash,
                new_hash: *new_hash,
                old_depth,
                new_depth,
                old,
                new,
            },
            stats: MerkleStats { new_cells_count },
        })
    }
}

#[cfg(all(feature = "rayon", feature = "sync"))]
const ROOT_SPLIT_DEPTH: u16 = 3;
#[cfg(all(feature = "rayon", feature = "sync"))]
const CHILD_SPLIT_DEPTH: u16 = 5;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;

    fn visit_all_cells(cell: &Cell) -> ahash::HashSet<&HashBytes> {
        let mut result = ahash::HashSet::default();

        let mut stack = vec![cell.as_ref()];
        while let Some(cell) = stack.pop() {
            let repr_hash = cell.repr_hash();
            if !result.insert(repr_hash) {
                continue;
            }

            for child in cell.references() {
                stack.push(child);
            }
        }

        result
    }

    #[test]
    fn correct_store_load() {
        let default = MerkleUpdate::default();

        let mut builder = CellBuilder::new();
        default
            .store_into(&mut builder, Cell::empty_context())
            .unwrap();
        let cell = builder.build().unwrap();

        let parsed = cell.parse_exotic::<MerkleUpdate>().unwrap();
        assert_eq!(default, parsed);
    }

    #[test]
    fn dict_merkle_update() {
        // Create dict with keys 0..10
        let mut dict = Dict::<u32, u32>::new();
        for i in 0..10 {
            dict.add(i, i * 10).unwrap();
        }

        // Serialize old dict
        let old_dict_cell = CellBuilder::build_from(&dict).unwrap();
        let old_dict_hashes = visit_all_cells(&old_dict_cell);

        // Serialize new dict
        dict.set(0, 1).unwrap();
        let new_dict_cell = CellBuilder::build_from(dict).unwrap();

        assert_ne!(old_dict_cell.as_ref(), new_dict_cell.as_ref());

        // Create merkle update
        let merkle_update = MerkleUpdate::create(
            old_dict_cell.as_ref(),
            new_dict_cell.as_ref(),
            old_dict_hashes,
        )
        .build()
        .unwrap();

        {
            // Test serialization
            let mut builder = CellBuilder::new();
            merkle_update
                .store_into(&mut builder, Cell::empty_context())
                .unwrap();
            builder.build().unwrap();
        }

        let after_apply = merkle_update.apply(&old_dict_cell).unwrap();
        assert_eq!(after_apply.as_ref(), new_dict_cell.as_ref());
    }

    #[test]
    fn dict_merkle_update_with_usage_tree() {
        // Serialize old dict
        let old_dict_cell = {
            // Create dict with keys 0..10
            let mut dict = Dict::<u32, u32>::new();
            for i in 0..10 {
                dict.add(i, i * 10).unwrap();
            }

            CellBuilder::build_from(&dict).unwrap()
        };

        let usage_tree = UsageTree::new(UsageTreeMode::OnLoad);
        let old_dict_cell_tracked = usage_tree.track(&old_dict_cell);

        // Serialize new dict
        let new_dict_cell = {
            let mut dict = old_dict_cell_tracked.parse::<Dict<u32, u32>>().unwrap();
            dict.set(0, 1).unwrap();
            dict.set(10, 321).unwrap();
            CellBuilder::build_from(dict).unwrap()
        };

        assert_ne!(old_dict_cell.as_ref(), new_dict_cell.as_ref());

        // Create merkle update
        let merkle_update =
            MerkleUpdate::create(old_dict_cell.as_ref(), new_dict_cell.as_ref(), usage_tree)
                .build()
                .unwrap();

        // Test serialization
        let merkle_update_cell = CellBuilder::build_from(&merkle_update).unwrap();
        println!("BOC: {}", Boc::encode_base64(merkle_update_cell));

        let after_apply = merkle_update.apply(&old_dict_cell).unwrap();
        assert_eq!(after_apply.as_ref(), new_dict_cell.as_ref());
    }

    #[test]
    fn dict_removed_cells_diff() {
        // Create dict with keys 0..10
        let mut dict = Dict::<u32, u32>::new();
        for i in 0..10 {
            dict.add(i, 0).unwrap();
        }

        // Serialize old dict
        let old_dict_cell = CellBuilder::build_from(&dict).unwrap();
        let old_dict_hashes = visit_all_cells(&old_dict_cell);

        // Serialize new dict
        dict.set(0, 1).unwrap();
        let new_dict_cell = CellBuilder::build_from(dict).unwrap();

        assert_ne!(old_dict_cell.as_ref(), new_dict_cell.as_ref());

        // Create merkle update
        let merkle_update = MerkleUpdate::create(
            old_dict_cell.as_ref(),
            new_dict_cell.as_ref(),
            old_dict_hashes,
        )
        .build()
        .unwrap();

        // Test diff
        let mut refs_for_both = RefsStorage::default();
        refs_for_both.store_cell(old_dict_cell.as_ref());
        refs_for_both.store_cell(new_dict_cell.as_ref());

        let mut only_new_refs = RefsStorage::default();
        only_new_refs.store_cell(new_dict_cell.as_ref());

        let diff = merkle_update
            .compute_removed_cells(old_dict_cell.as_ref())
            .unwrap();
        assert!(refs_for_both.remove_batch(diff));

        assert_eq!(only_new_refs.refs, refs_for_both.refs);
    }

    #[test]
    fn dict_removed_all_cells_diff() {
        // Create dict with keys 0..10
        let mut dict = Dict::<u32, u32>::new();
        for i in 0..10 {
            dict.add(i, 0).unwrap();
        }

        // Serialize old dict
        let old_dict_cell = CellBuilder::build_from(&dict).unwrap();
        let old_dict_hashes = visit_all_cells(&old_dict_cell);

        // Serialize new dict
        let new_dict_cell = CellBuilder::build_from(Dict::<u32, u32>::new()).unwrap();

        assert_ne!(old_dict_cell.as_ref(), new_dict_cell.as_ref());

        // Create merkle update
        let merkle_update = MerkleUpdate::create(
            old_dict_cell.as_ref(),
            new_dict_cell.as_ref(),
            old_dict_hashes,
        )
        .build()
        .unwrap();

        // Test diff
        let mut refs_for_both = RefsStorage::default();
        refs_for_both.store_cell(old_dict_cell.as_ref());
        refs_for_both.store_cell(new_dict_cell.as_ref());

        let mut only_new_refs = RefsStorage::default();
        only_new_refs.store_cell(new_dict_cell.as_ref());

        let diff = merkle_update
            .compute_removed_cells(old_dict_cell.as_ref())
            .unwrap();
        assert!(refs_for_both.remove_batch(diff));

        assert_eq!(only_new_refs.refs, refs_for_both.refs);
    }

    #[test]
    fn build_invalid_update() {
        let old = CellBuilder::build_from((123u32, Cell::empty_cell())).unwrap();
        let new = CellBuilder::build_from((234u32, &old)).unwrap();

        for update in [
            // Invalid old hash
            MerkleUpdate {
                old_hash: HashBytes::ZERO,
                new_hash: *new.hash(0),
                old_depth: old.depth(0),
                new_depth: new.depth(0),
                old: old.clone(),
                new: new.clone(),
            },
            // Invalid new hash
            MerkleUpdate {
                old_hash: *old.hash(0),
                new_hash: HashBytes::ZERO,
                old_depth: 0,
                new_depth: 0,
                old: old.clone(),
                new: new.clone(),
            },
            // Invalid old depth
            MerkleUpdate {
                old_hash: *old.hash(0),
                new_hash: *new.hash(0),
                old_depth: 123,
                new_depth: new.depth(0),
                old: old.clone(),
                new: new.clone(),
            },
            // Invalid new depth
            MerkleUpdate {
                old_hash: *old.hash(0),
                new_hash: *new.hash(0),
                old_depth: old.depth(0),
                new_depth: 123,
                old,
                new,
            },
        ] {
            let err = CellBuilder::build_from(update).unwrap_err();
            assert_eq!(err, Error::InvalidCell);
        }
    }

    #[derive(Default)]
    struct RefsStorage<'a> {
        refs: ahash::HashMap<&'a HashBytes, u32>,
    }

    impl<'a> RefsStorage<'a> {
        fn store_cell(&mut self, root: &'a DynCell) {
            use std::collections::hash_map;

            *self.refs.entry(root.repr_hash()).or_default() += 1;

            let mut stack = vec![root.references()];
            'outer: while let Some(iter) = stack.last_mut() {
                for child in iter {
                    let hash = child.repr_hash();
                    match self.refs.entry(hash) {
                        hash_map::Entry::Occupied(mut entry) => {
                            *entry.get_mut() += 1;
                            continue;
                        }
                        hash_map::Entry::Vacant(entry) => {
                            entry.insert(1);
                        }
                    }

                    stack.push(child.references());
                    continue 'outer;
                }
                stack.pop();
            }
        }

        fn remove_batch(&mut self, mut batch: ahash::HashMap<&'a HashBytes, u32>) -> bool {
            self.refs.retain(|hash, refs| {
                if let Some(diff) = batch.remove(hash) {
                    *refs -= diff;
                }
                *refs != 0
            });
            batch.is_empty()
        }
    }

    /// Helper function to count new cells by traversing.
    fn count_new_cells(merkle_update: &MerkleUpdate) -> usize {
        if merkle_update.old_hash == merkle_update.new_hash {
            return 0;
        }

        let mut visited = ahash::HashSet::default();
        let mut count = 0;
        let mut stack = vec![merkle_update.new.as_ref()];

        while let Some(cell) = stack.pop() {
            let hash = cell.repr_hash();

            // Skip already visited cells
            if !visited.insert(hash) {
                continue;
            }

            // Count only non-pruned cells
            if !cell.descriptor().is_pruned_branch() {
                count += 1;

                // Add children only for non-pruned cells
                for child in cell.references() {
                    stack.push(child);
                }
            }
        }

        count
    }

    #[test]
    fn test_count_new_cells() {
        // Create dict with keys 0..20
        let mut dict = Dict::<u32, u32>::new();
        for i in 0..20 {
            dict.add(i, i * 10).unwrap();
        }

        // Serialize old dict
        let old_dict_cell = CellBuilder::build_from(&dict).unwrap();
        let old_dict_hashes = visit_all_cells(&old_dict_cell);

        // Modify dict
        dict.set(0, 1).unwrap();
        dict.set(5, 999).unwrap();
        dict.set(10, 9999).unwrap();
        dict.set(15, 99999).unwrap();
        let new_dict_cell = CellBuilder::build_from(dict).unwrap();

        assert_ne!(old_dict_cell.as_ref(), new_dict_cell.as_ref());

        // Create merkle update
        let merkle_update = MerkleUpdate::create(
            old_dict_cell.as_ref(),
            new_dict_cell.as_ref(),
            old_dict_hashes,
        )
        .build()
        .unwrap();

        // Count new cells via traversal
        let count_via_traversal = count_new_cells(&merkle_update);

        // Count new cells via apply
        let result = merkle_update.apply_with_stats(&old_dict_cell).unwrap();

        assert_eq!(result.cell.as_ref(), new_dict_cell.as_ref());
        assert_eq!(count_via_traversal, result.stats.new_cells_count,);
    }

    #[test]
    #[cfg(all(feature = "rayon", feature = "sync"))]
    fn test_par_apply_new_cells_count() {
        // Create dict with keys 0..20
        let mut dict = Dict::<u32, u32>::new();
        for i in 0..20 {
            dict.add(i, i * 10).unwrap();
        }

        // Serialize old dict
        let old_dict_cell = CellBuilder::build_from(&dict).unwrap();
        let old_dict_hashes = visit_all_cells(&old_dict_cell);

        // Modify dict
        // Modify dict
        dict.set(0, 1).unwrap();
        dict.set(5, 999).unwrap();
        dict.set(10, 9999).unwrap();
        dict.set(15, 99999).unwrap();
        let new_dict_cell = CellBuilder::build_from(dict).unwrap();

        assert_ne!(old_dict_cell.as_ref(), new_dict_cell.as_ref());

        // Create merkle update
        let merkle_update = MerkleUpdate::create(
            old_dict_cell.as_ref(),
            new_dict_cell.as_ref(),
            old_dict_hashes,
        )
        .build()
        .unwrap();

        // Count via traversal
        let count_via_traversal = count_new_cells(&merkle_update);

        // Count via par_apply
        let split_at = ahash::HashSet::default();
        let result = merkle_update
            .par_apply_with_stats(&old_dict_cell, &split_at)
            .unwrap();

        assert_eq!(result.cell.as_ref(), new_dict_cell.as_ref());
        assert_eq!(count_via_traversal, result.stats.new_cells_count,);
    }

    #[test]
    fn test_build_new_cells_count() {
        // Create dict with keys 0..20
        let mut dict = Dict::<u32, u32>::new();
        for i in 0..20 {
            dict.add(i, i * 10).unwrap();
        }

        // Serialize old dict
        let old_dict_cell = CellBuilder::build_from(&dict).unwrap();
        let old_dict_hashes = visit_all_cells(&old_dict_cell);

        // Modify dict
        dict.set(0, 1).unwrap();
        dict.set(5, 999).unwrap();
        dict.set(10, 9999).unwrap();
        dict.set(15, 99999).unwrap();
        let new_dict_cell = CellBuilder::build_from(dict).unwrap();

        assert_ne!(old_dict_cell.as_ref(), new_dict_cell.as_ref());

        // Create merkle update with stats
        let result = MerkleUpdate::create(
            old_dict_cell.as_ref(),
            new_dict_cell.as_ref(),
            old_dict_hashes,
        )
        .build_with_stats()
        .unwrap();

        // Count new cells via traversal
        let count_via_traversal = count_new_cells(&result.update);
        assert_eq!(count_via_traversal, result.stats.new_cells_count);
    }

    #[test]
    #[cfg(all(feature = "rayon", feature = "sync"))]
    fn test_par_build_new_cells_count() {
        // Create dict with keys 0..20
        let mut dict = Dict::<u32, u32>::new();
        for i in 0..20 {
            dict.add(i, i * 10).unwrap();
        }

        // Serialize old dict
        let old_dict_cell = CellBuilder::build_from(&dict).unwrap();
        let old_dict_hashes = visit_all_cells(&old_dict_cell);

        // Modify dict
        dict.set(0, 1).unwrap();
        dict.set(5, 999).unwrap();
        dict.set(10, 9999).unwrap();
        dict.set(15, 99999).unwrap();
        let new_dict_cell = CellBuilder::build_from(dict).unwrap();

        assert_ne!(old_dict_cell.as_ref(), new_dict_cell.as_ref());

        // Create merkle update with stats using parallel build
        let result = MerkleUpdate::create(
            old_dict_cell.as_ref(),
            new_dict_cell.as_ref(),
            old_dict_hashes,
        )
        .par_build_with_stats(Default::default(), Default::default())
        .unwrap();

        // Count new cells via traversal
        let count_via_traversal = count_new_cells(&result.update);
        assert_eq!(count_via_traversal, result.stats.new_cells_count);
    }
}
