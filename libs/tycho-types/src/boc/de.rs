use std::ops::Deref;

use smallvec::SmallVec;

use super::BocTag;
use crate::cell::{Cell, CellContext, CellDescriptor, CellParts, LevelMask, MAX_REF_COUNT};
use crate::util::{ArrayVec, read_be_u32_fast, read_be_u64_fast, unlikely};

/// BOC deserialization options.
#[derive(Debug, Default, Clone)]
pub struct Options {
    /// The minimum allowed root count.
    pub min_roots: Option<usize>,
    /// The maximum allowed root count.
    pub max_roots: Option<usize>,
}

impl Options {
    /// Constructs decoder options to expect exactly the specified number of roots.
    pub const fn exact(number: usize) -> Self {
        Self {
            min_roots: Some(number),
            max_roots: Some(number),
        }
    }
}

/// Parsed BOC header.
pub struct BocHeader<'a> {
    ref_size: usize,
    cells: SmallVec<[&'a [u8]; CELLS_ON_STACK]>,
    roots: SmallVec<[u32; ROOTS_ON_STACK]>,
}

impl<'a> BocHeader<'a> {
    /// Decodes boc info from the specified bytes.
    pub fn decode(data: &'a [u8], options: &Options) -> Result<Self, Error> {
        let mut reader = BocReader::new(data.len());

        // 4 bytes - tag
        // 1 byte - flags
        // 1 byte - offset size
        if unlikely(!reader.require(6)) {
            return Err(Error::UnexpectedEof);
        }
        debug_assert!(data.len() >= 6);

        // SAFETY: we have already requested more than 6 bytes
        let [flags, offset_size] = unsafe { *(data.as_ptr().add(4) as *const [u8; 2]) };

        let has_index;
        let has_crc;
        let has_cache_bits;
        let ref_size;
        let supports_multiple_roots;

        // SAFETY: we have already requested more than 4 bytes
        let boc_tag = unsafe { reader.read_boc_tag(data) };
        match boc_tag {
            Some(BocTag::Indexed) => {
                has_index = true;
                has_crc = false;
                has_cache_bits = false;
                ref_size = flags as usize;
                supports_multiple_roots = false;
            }
            Some(BocTag::IndexedCrc32) => {
                has_index = true;
                has_crc = true;
                has_cache_bits = false;
                ref_size = flags as usize;
                supports_multiple_roots = false;
            }
            Some(BocTag::Generic) => {
                has_index = flags & 0b1000_0000 != 0;
                has_crc = flags & 0b0100_0000 != 0;
                has_cache_bits = flags & 0b0010_0000 != 0;
                ref_size = (flags & 0b0000_0111) as usize;
                supports_multiple_roots = true;
            }
            None => return Err(Error::UnknownBocTag),
        }

        if unlikely(has_cache_bits && !has_index) {
            return Err(Error::InvalidHeader);
        }
        if unlikely(ref_size == 0 || ref_size > std::mem::size_of::<u32>()) {
            return Err(Error::InvalidRefSize);
        }
        debug_assert!((1..=4).contains(&ref_size));

        let offset_size = offset_size as usize;
        if unlikely(offset_size == 0 || offset_size > std::mem::size_of::<usize>()) {
            return Err(Error::InvalidOffsetSize);
        }
        debug_assert!((1..=8).contains(&offset_size));

        reader.advance(6);

        // {ref_size} bytes - cell count
        // {ref_size} bytes - root count
        // {ref_size} bytes - absent cell count
        // {offset_size} bytes - total cells size
        if unlikely(!reader.require(ref_size * 3 + offset_size)) {
            return Err(Error::InvalidHeader);
        }
        debug_assert!(data.len() >= (6 + ref_size * 3 + offset_size));

        // SAFETY: we have already requested more than {ref_size}*3
        // and {ref_size} is in range 1..=4
        let (cell_count, root_count, absent_count) = unsafe {
            (
                reader.read_next_be_uint_fast(data, ref_size),
                reader.read_next_be_uint_fast(data, ref_size),
                reader.read_next_be_uint_fast(data, ref_size),
            )
        };

        // Validate root or absent cells
        if unlikely(root_count == 0) {
            return Err(Error::RootCellNotFound);
        }
        if unlikely(!supports_multiple_roots && root_count > 1) {
            return Err(Error::UnexpectedMultipleRoots);
        }
        if unlikely(root_count.saturating_add(absent_count) > cell_count) {
            return Err(Error::TooManyRootCells);
        }
        if unlikely(absent_count > 0) {
            return Err(Error::AbsentCellsNotSupported);
        }
        if let Some(min_roots) = options.min_roots
            && unlikely(root_count < min_roots)
        {
            return Err(Error::TooFewRootCells);
        }

        {
            let max_roots = options.max_roots.unwrap_or(MAX_ROOTS);
            if unlikely(root_count > max_roots) {
                return Err(Error::TooManyRootCells);
            }
            debug_assert!(absent_count == 0 && (1..=max_roots).contains(&root_count))
        }

        // SAFETY: we have already requested at least {ref_size}*3+{offset_size}
        // and {ref_size} is in range 1..=8
        let total_cells_size = unsafe { reader.read_next_be_uint_full(data, offset_size) };

        const MIN_CELL_SIZE: u64 = 2; // [d1, d2]

        // NOTE: `cell_count` is guaranteed to be in range of `u32`, so
        // `u32::MAX * (2 + 4)` fits into u64 and doesn't require saturating/checked mul,
        // `root_count` <= `cell_count` so this expression doesn't overflow
        let min_total_cell_size = (cell_count as u64) * (MIN_CELL_SIZE + ref_size as u64)
            - (root_count * ref_size) as u64;
        if unlikely(total_cells_size < min_total_cell_size) {
            return Err(Error::InvalidTotalSize);
        }

        // NOTE: `cell_count` is guaranteed to be in range of `u32`, so
        // `u32::MAX * 282` fits into u64 and doesn't require saturating/checked mul
        // 2 bytes - descriptor
        // 4 * (2 + 32) - inline hashes and depths if presented
        // 128 - max data length
        // 4*{ref_size} - max references
        let max_cell_size = 2 + 4 * (2 + 32) + 128 + (MAX_REF_COUNT as u64) * ref_size as u64; // ~282 bytes
        if unlikely(total_cells_size > (cell_count as u64) * max_cell_size) {
            return Err(Error::InvalidTotalSize);
        }

        // NOTE: `root_count` is in range ..=u32::MAX and `ref_size` is in range 1..=4
        if unlikely(!reader.require(root_count * ref_size)) {
            return Err(Error::UnexpectedEof);
        }
        debug_assert!(data.len() >= (6 + ref_size * 3 + offset_size + root_count * ref_size));

        let mut roots = SmallVec::with_capacity(root_count);
        if supports_multiple_roots {
            for _ in 0..root_count {
                // SAFETY: we have already requested for {root_count}*{ref_size}
                let root_index = unsafe { reader.read_next_be_uint_fast(data, ref_size) };
                if unlikely(root_index >= cell_count) {
                    return Err(Error::RootOutOfBounds);
                }
                roots.push(root_index as u32);
            }
        } else {
            roots.push(0);
        }

        // NOTE: `cell_count` is in range ..=u32::MAX, `offset_size` is in range 1..=8
        let index_size = has_index as u64 * cell_count as u64 * offset_size as u64;
        if unlikely(!reader.require((index_size + total_cells_size + has_crc as u64 * 4) as usize))
        {
            return Err(Error::UnexpectedEof);
        }

        if has_index {
            reader.advance(cell_count * offset_size);
        }

        #[cfg(not(fuzzing))]
        let cells_start_offset = reader.offset;

        let mut cells = SmallVec::with_capacity(cell_count);

        let data_ptr = data.as_ptr();
        for _ in 0..cell_count {
            // SAFETY: there are manual bounds checks for bytes offset
            let start_ptr = unsafe { data_ptr.add(reader.offset) };
            let total_len = ok!(CellParts::read_raw_cell_from_ptr(
                start_ptr,
                reader.len - reader.offset,
                ref_size
            ));
            reader.advance(total_len);

            // SAFETY: We have already requested {total_len} bytes
            let cell = unsafe { std::slice::from_raw_parts(start_ptr, total_len) };
            cells.push(cell);
        }

        // Check that `total_cells_size` is correct
        #[cfg(not(fuzzing))]
        if (cells_start_offset as u64).saturating_add(total_cells_size) != reader.offset as u64 {
            return Err(Error::InvalidTotalSize);
        }

        // Verify checksum if specified
        #[cfg(not(fuzzing))]
        if has_crc {
            if unlikely(!reader.require(4)) {
                return Err(Error::UnexpectedEof);
            }

            // SAFETY: we have already requested 4 bytes
            let is_checksum_correct = unsafe { reader.check_crc(data) };
            if !is_checksum_correct {
                return Err(Error::InvalidChecksum);
            }
        }

        Ok(Self {
            ref_size,
            cells,
            roots,
        })
    }

    /// Assembles cell tree from slices using the specified cell context.
    pub fn finalize(&self, context: &dyn CellContext) -> Result<ProcessedCells, Error> {
        let ref_size = self.ref_size;
        let cell_count = self.cells.len() as u32;

        // TODO: somehow reuse `cells` vec
        let mut res = SmallVec::<[Cell; CELLS_ON_STACK]>::new();
        if res.try_reserve_exact(cell_count as usize).is_err() {
            return Err(Error::InvalidTotalSize);
        }

        for raw_cell in self.cells().iter().rev() {
            // SAFETY: it is safe to construct `CellParts` from a `read_raw_cell_from_ptr` output
            let ctx = unsafe {
                ok!(CellParts::from_raw_cell(
                    raw_cell, &res, cell_count, ref_size
                ))
            };

            let cell = match context.finalize_cell(ctx) {
                Ok(cell) => cell,
                Err(_) => return Err(Error::InvalidCell),
            };
            res.push(cell);
        }

        Ok(ProcessedCells(res))
    }

    /// Cell index size in bytes. Guaranteed to be 4 at max.
    pub fn ref_size(&self) -> usize {
        self.ref_size
    }

    /// Slices of the unique cells.
    pub fn cells(&self) -> &[&'a [u8]] {
        &self.cells
    }

    /// Root indices.
    pub fn roots(&self) -> &[u32] {
        &self.roots
    }
}

/// Array of processed cells.
pub struct ProcessedCells(SmallVec<[Cell; CELLS_ON_STACK]>);

impl ProcessedCells {
    /// Returns a processed cell by index.
    pub fn get(&self, index: u32) -> Option<Cell> {
        self.0.get(self.0.len() - index as usize - 1).cloned()
    }
}

/// Wrapper around indexed bytes slice access
/// to eliminate bounds check.
struct BocReader {
    len: usize,
    offset: usize,
}

impl BocReader {
    #[inline(always)]
    const fn new(len: usize) -> Self {
        Self { len, offset: 0 }
    }

    #[inline(always)]
    const fn require(&self, len: usize) -> bool {
        self.offset + len <= self.len
    }

    #[inline(always)]
    fn advance(&mut self, bytes: usize) {
        self.offset += bytes;
    }

    #[inline(always)]
    unsafe fn read_boc_tag(&self, data: &[u8]) -> Option<BocTag> {
        unsafe { BocTag::from_bytes(*(data.as_ptr() as *const [u8; 4])) }
    }

    /// # Safety
    ///
    /// The following must be true:
    /// - size must be in range 1..=4.
    /// - data must be at least `self.offset + size` bytes long.
    #[inline(always)]
    unsafe fn read_next_be_uint_fast(&mut self, data: &[u8], size: usize) -> usize {
        unsafe {
            let res = read_be_u32_fast(data.as_ptr().add(self.offset), size) as usize;
            self.advance(size);
            res
        }
    }

    /// # Safety
    ///
    /// The following must be true:
    /// - size must be in range 1..=8.
    /// - data must be at least `self.offset + size` bytes long.
    #[inline(always)]
    unsafe fn read_next_be_uint_full(&mut self, data: &[u8], size: usize) -> u64 {
        unsafe {
            let res = read_be_u64_fast(data.as_ptr().add(self.offset), size);
            self.advance(size);
            res
        }
    }

    #[cfg(not(fuzzing))]
    #[inline(always)]
    unsafe fn check_crc(&self, data: &[u8]) -> bool {
        unsafe {
            let data_ptr = data.as_ptr();
            let crc_start_ptr = data_ptr.add(self.offset);

            let parsed_crc = u32::from_le_bytes(*(crc_start_ptr as *const [u8; 4]));
            let real_crc = crc32c::crc32c(std::slice::from_raw_parts(data_ptr, self.offset));

            parsed_crc == real_crc
        }
    }
}

impl Deref for BocReader {
    type Target = usize;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.offset
    }
}

impl<'a> CellParts<'a> {
    /// Reads cell parts from the raw cell slice.
    ///
    /// # Safety
    ///
    /// The following must be true:
    /// - `bytes` must be a correct bytes representation of cell.
    ///
    /// NOTE: It is safe to use an unmodified output from `CellParts::read_raw_cell`.
    pub unsafe fn from_raw_cell(
        raw_cell: &'a [u8],
        cells: &[Cell],
        cell_count: u32,
        ref_size: usize,
    ) -> Result<Self, Error> {
        unsafe {
            let raw_cell_ptr = raw_cell.as_ptr();

            let descriptor = CellDescriptor::new(*(raw_cell_ptr as *const [u8; 2]));
            let data_len = descriptor.byte_len() as usize;

            let mut data_ptr = raw_cell_ptr.add(2);
            if unlikely(descriptor.store_hashes()) {
                let level = descriptor.level_mask().level();
                debug_assert!(!descriptor.cell_type().is_pruned_branch());
                data_ptr = data_ptr.add((32 + 2) * (level as usize + 1));
            }

            let data = std::slice::from_raw_parts(data_ptr, data_len);
            data_ptr = data_ptr.add(data_len);

            let bit_len = if descriptor.is_aligned() {
                (data_len * 8) as u16
            } else if let Some(data) = data.last() {
                data_len as u16 * 8 - data.trailing_zeros() as u16 - 1
            } else {
                0
            };

            let mut references = ArrayVec::<Cell, MAX_REF_COUNT>::default();
            let mut children_mask = LevelMask::EMPTY;

            for _ in 0..descriptor.reference_count() {
                let child_index = read_be_u32_fast(data_ptr, ref_size);
                if child_index >= cell_count {
                    return Err(Error::InvalidRef);
                }

                let child = match cells.get((cell_count - child_index - 1) as usize) {
                    Some(child) => child.clone(),
                    None => return Err(Error::InvalidRefOrder),
                };

                children_mask |= child.descriptor().level_mask();

                references.push(child);

                data_ptr = data_ptr.add(ref_size);
            }

            Ok(CellParts {
                bit_len,
                descriptor,
                children_mask,
                references,
                data,
            })
        }
    }

    /// Reads a raw cell from the specified slice.
    /// The returned slice is guaranteed to be a correct bytes representation of cell.
    pub fn read_raw_cell<'b>(bytes: &mut &'b [u8], ref_size: usize) -> Result<&'b [u8], Error> {
        let total_len = ok!(Self::read_raw_cell_from_ptr(
            bytes.as_ptr(),
            bytes.len(),
            ref_size
        ));
        let (cell, rest) = bytes.split_at(total_len);
        *bytes = rest;
        Ok(cell)
    }

    fn read_raw_cell_from_ptr(
        bytes_ptr: *const u8,
        bytes_len: usize,
        ref_size: usize,
    ) -> Result<usize, Error> {
        const _: () = assert!(std::mem::size_of::<CellDescriptor>() == 2);

        if unlikely(bytes_len < 2) {
            return Err(Error::UnexpectedEof);
        }

        // SAFETY: we have already checked the reader has 2 bytes
        // and the `CellDescriptor` layout is fixed by `repr(C)`
        let descriptor = CellDescriptor::new(unsafe { *(bytes_ptr as *const [u8; 2]) });

        if unlikely(descriptor.is_absent()) {
            return Err(Error::AbsentCellsNotSupported);
        }

        // 0b11111111 -> 0b01111111 + 1 = 0b10000000 = byte len 128, max bit len = 1023
        // 0b11111110 -> 0b01111111 = byte len 127, bit len = 1016
        let data_len = descriptor.byte_len() as usize;
        let ref_count = descriptor.reference_count() as usize;
        if unlikely(ref_count > MAX_REF_COUNT) {
            return Err(Error::InvalidRef);
        }

        let mut data_offset = 0;
        if unlikely(descriptor.store_hashes()) {
            let level = descriptor.level_mask().level();
            if descriptor.is_exotic() && ref_count == 0 && level > 0 {
                // Pruned branch with `store_hashes` is invalid
                return Err(Error::UnnormalizedCell);
            }
            data_offset = (32 + 2) * (level as usize + 1);
        }

        let total_len = 2 + data_offset + data_len + ref_count * ref_size;
        if unlikely(bytes_len < total_len) {
            return Err(Error::UnexpectedEof);
        }

        if data_len > 0 && !descriptor.is_aligned() {
            // SAFETY: we have already requested 2+{data_len} bytes
            let byte_with_tag = unsafe { *bytes_ptr.add(2 + data_offset + data_len - 1) };
            if unlikely(byte_with_tag & 0x7f == 0) {
                return Err(Error::UnnormalizedCell);
            }
        }

        Ok(total_len)
    }
}

const CELLS_ON_STACK: usize = 16;
const ROOTS_ON_STACK: usize = 2;

const MAX_ROOTS: usize = 32;

/// Error type for BOC decoding related errors.
#[derive(Debug, Copy, Clone, thiserror::Error)]
pub enum Error {
    /// EOF encountered during another operation.
    #[error("unexpected EOF")]
    UnexpectedEof,
    /// Invalid magic bytes.
    #[error("unknown BOC tag")]
    UnknownBocTag,
    /// Invalid BOC header.
    #[error("invalid header")]
    InvalidHeader,
    /// References size is greater than 4.
    #[error("ref index does not fit in `u32` type")]
    InvalidRefSize,
    /// Offset size is greater than 8.
    #[error("cell offset does not fit in `usize` type")]
    InvalidOffsetSize,
    /// Root cell not found.
    #[error("root cell not found")]
    RootCellNotFound,
    /// Specified BOC tag doesn't support multiple roots.
    #[error("unexpected multiple roots")]
    UnexpectedMultipleRoots,
    /// The number of roots in BOC is greater than expected.
    #[error("too many root cells")]
    TooManyRootCells,
    /// Absent cells are legacy therefore not supported.
    #[error("absent cells are not supported")]
    AbsentCellsNotSupported,
    /// The number of roots in BOC is less than expected.
    #[error("too few root cells")]
    TooFewRootCells,
    /// Total cells size mismatch.
    #[error("invalid total cells size")]
    InvalidTotalSize,
    /// Invalid root cell index.
    #[error("root index out of bounds")]
    RootOutOfBounds,
    /// Invalid child reference.
    #[error("cell ref count not in range 0..=4")]
    InvalidRef,
    /// Suboptimal cells are treated as error.
    #[error("unnormalized cell")]
    UnnormalizedCell,
    /// Possible graph loop detected.
    #[error("invalid children order")]
    InvalidRefOrder,
    /// Failed to parse cell.
    #[error("invalid cell")]
    InvalidCell,
    /// Crc mismatch.
    #[error("invalid checksum")]
    InvalidChecksum,
}
