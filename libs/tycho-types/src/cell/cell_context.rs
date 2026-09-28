use sha2::digest::Digest;

use crate::cell::{Cell, CellDescriptor, CellType, DynCell, HashBytes, LevelMask, MAX_REF_COUNT};
use crate::error::Error;
use crate::util::{ArrayVec, unlikely};

/// Gas accounting and resolcing exotic cells.
pub trait CellContext {
    /// Builds a new cell from cell parts.
    fn finalize_cell(&self, cell: CellParts<'_>) -> Result<Cell, Error>;

    /// Resolve an owned cell.
    fn load_cell(&self, cell: Cell, mode: LoadMode) -> Result<Cell, Error>;

    /// Resolve a cell reference.
    fn load_dyn_cell<'s: 'a, 'a>(
        &'s self,
        cell: &'a DynCell,
        mode: LoadMode,
    ) -> Result<&'a DynCell, Error>;
}

/// Dictionary insertion mode.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum LoadMode {
    /// Do not modify the default behavior.
    Noop = 0b00,
    /// Count the cost of loading the cell.
    UseGas = 0b01,
    /// Resolve exotic cells such as libraries or merkle stuff.
    Resolve = 0b10,
    /// Both `UseGas` and `Resolve`.
    Full = 0b11,
}

impl LoadMode {
    /// Returns `true` if this mode requires gas accounting.
    #[inline]
    pub const fn use_gas(self) -> bool {
        self as u8 & 0b01 != 0
    }

    /// Returns `true` if exotic cells are resolved in this mode.
    #[inline]
    pub const fn resolve(self) -> bool {
        self as u8 & 0b10 != 0
    }
}

/// Partially assembled cell.
pub struct CellParts<'a> {
    /// Length of this cell's data in bits.
    pub bit_len: u16,

    /// Well-formed cell descriptor.
    pub descriptor: CellDescriptor,

    /// Bitwise OR of child level masks.
    pub children_mask: LevelMask,

    /// Array of child cells.
    ///
    /// NOTE: it is guaranteed that the length of the array is consistent
    /// with the descriptor.
    pub references: ArrayVec<Cell, MAX_REF_COUNT>,

    /// Cell data slice.
    pub data: &'a [u8],
}

impl CellParts<'_> {
    /// Validates cell and computes all hashes.
    pub fn compute_hashes(&self) -> Result<Box<[(HashBytes, u16)]>, Error> {
        const HASH_BITS: usize = 256;
        const DEPTH_BITS: usize = 16;

        let mut descriptor = self.descriptor;
        let bit_len = self.bit_len as usize;
        let level_mask = descriptor.level_mask();
        let level = level_mask.level() as usize;

        let references = self.references.as_ref();

        // Just in case check once more if data is enough to fit the specified number of bits.
        if unlikely(self.data.len() < bit_len.div_ceil(8)) {
            return Err(Error::InvalidCell);
        }

        // `hashes_len` is guaranteed to be in range 1..4
        let mut hashes_len = level + 1;

        let (cell_type, computed_level_mask) = if unlikely(descriptor.is_exotic()) {
            let Some(&first_byte) = self.data.first() else {
                return Err(Error::InvalidCell);
            };

            match CellType::from_byte_exotic(first_byte) {
                // 8 bits type, 8 bits level mask, level x (hash, depth)
                Some(CellType::PrunedBranch) => {
                    if unlikely(level == 0) {
                        return Err(Error::InvalidCell);
                    }

                    let expected_bit_len = 8 + 8 + level * (HASH_BITS + DEPTH_BITS);
                    if unlikely(bit_len != expected_bit_len || !references.is_empty()) {
                        return Err(Error::InvalidCell);
                    }

                    let stored_mask = self.data.get(1).copied().unwrap_or_default();
                    if unlikely(level_mask != stored_mask) {
                        return Err(Error::InvalidCell);
                    }

                    hashes_len = 1;
                    (CellType::PrunedBranch, level_mask)
                }
                // 8 bits type, hash, depth
                Some(CellType::MerkleProof) => {
                    const EXPECTED_BIT_LEN: usize = 8 + HASH_BITS + DEPTH_BITS;
                    if unlikely(
                        bit_len != EXPECTED_BIT_LEN
                            || references.len() != 1
                            || self.data.len() < 35,
                    ) {
                        return Err(Error::InvalidCell);
                    }

                    let stored_hash = &self.data[1..33];
                    let stored_depth = u16::from_be_bytes([self.data[33], self.data[34]]);

                    let child = &references[0];
                    if unlikely(child.hash(0) != stored_hash || child.depth(0) != stored_depth) {
                        return Err(Error::InvalidCell);
                    }

                    (CellType::MerkleProof, self.children_mask.virtualize(1))
                }
                // 8 bits type, 2 x (hash, depth)
                Some(CellType::MerkleUpdate) => {
                    const EXPECTED_BIT_LEN: usize = 8 + 2 * (HASH_BITS + DEPTH_BITS);
                    if unlikely(
                        bit_len != EXPECTED_BIT_LEN
                            || references.len() != 2
                            || self.data.len() < 69,
                    ) {
                        return Err(Error::InvalidCell);
                    }

                    let stored_old_hash = &self.data[1..33];
                    let stored_new_hash = &self.data[33..65];

                    let stored_old_depth = u16::from_be_bytes([self.data[65], self.data[66]]);
                    let stored_new_depth = u16::from_be_bytes([self.data[67], self.data[68]]);

                    let old = &references[0];
                    let new = &references[1];
                    if unlikely(
                        old.hash(0) != stored_old_hash
                            || old.depth(0) != stored_old_depth
                            || new.hash(0) != stored_new_hash
                            || new.depth(0) != stored_new_depth,
                    ) {
                        return Err(Error::InvalidCell);
                    }

                    (CellType::MerkleUpdate, self.children_mask.virtualize(1))
                }
                // 8 bits type, hash
                Some(CellType::LibraryReference) => {
                    const EXPECTED_BIT_LEN: usize = 8 + HASH_BITS;
                    if unlikely(bit_len != EXPECTED_BIT_LEN || !references.is_empty()) {
                        return Err(Error::InvalidCell);
                    }

                    (CellType::LibraryReference, LevelMask::EMPTY)
                }
                _ => return Err(Error::InvalidCell),
            }
        } else {
            (CellType::Ordinary, self.children_mask)
        };

        if unlikely(computed_level_mask != level_mask) {
            return Err(Error::InvalidCell);
        }

        let level_offset = cell_type.is_merkle() as u8;
        let is_pruned = cell_type.is_pruned_branch();

        let mut hashes = Vec::<(HashBytes, u16)>::with_capacity(hashes_len);
        for level in 0..4 {
            // Skip non-zero levels for pruned branches and insignificant hashes for other cells
            if level != 0 && (is_pruned || !level_mask.contains(level)) {
                continue;
            }

            let mut hasher = sha2::Sha256::new();

            let level_mask = if is_pruned {
                level_mask
            } else {
                LevelMask::from_level(level)
            };

            descriptor.d1 &= !(CellDescriptor::LEVEL_MASK | CellDescriptor::STORE_HASHES_MASK);
            descriptor.d1 |= u8::from(level_mask) << 5;
            hasher.update([descriptor.d1, descriptor.d2]);

            if level == 0 {
                hasher.update(self.data);
            } else {
                // SAFETY: new hash is added on each iteration, so there will
                // definitely be a hash, when level>0
                let prev_hash = unsafe { hashes.last().unwrap_unchecked() };
                hasher.update(prev_hash.0.as_slice());
            }

            let mut depth = 0;
            for child in references {
                let child_depth = child.as_ref().depth(level + level_offset);
                let next_depth = match child_depth.checked_add(1) {
                    Some(next_depth) => next_depth,
                    None => return Err(Error::DepthOverflow),
                };
                depth = std::cmp::max(depth, next_depth);

                hasher.update(child_depth.to_be_bytes());
            }

            for child in references {
                let child_hash = child.as_ref().hash(level + level_offset);
                hasher.update(child_hash.as_slice());
            }

            let hash = hasher.finalize().into();
            hashes.push((hash, depth));
        }

        Ok(hashes.into_boxed_slice())
    }
}
