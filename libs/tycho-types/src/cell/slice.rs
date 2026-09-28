use std::num::{NonZeroU8, NonZeroU16, NonZeroU32};
use std::rc::Rc;
use std::sync::Arc;

use super::CellFamily;
use crate::cell::{
    Cell, CellTreeStats, CellType, DynCell, HashBytes, LevelMask, RefsIter, Size, StorageStat,
};
use crate::error::Error;
use crate::util::{Bitstring, unlikely};

/// A data structure that can be deserialized from the full cell.
pub trait LoadCell<'a>: Sized {
    /// Tries to load itself from cell.
    fn load_from_cell(cell: &'a DynCell) -> Result<Self, Error>;
}

impl<'a, T: LoadCell<'a>> LoadCell<'a> for Box<T> {
    #[inline]
    fn load_from_cell(cell: &'a DynCell) -> Result<Self, Error> {
        match <T as LoadCell>::load_from_cell(cell) {
            Ok(value) => Ok(Box::new(value)),
            Err(e) => Err(e),
        }
    }
}

impl<'a, T: LoadCell<'a>> LoadCell<'a> for Arc<T> {
    #[inline]
    fn load_from_cell(cell: &'a DynCell) -> Result<Self, Error> {
        match <T as LoadCell>::load_from_cell(cell) {
            Ok(value) => Ok(Arc::new(value)),
            Err(e) => Err(e),
        }
    }
}

impl<'a, T: LoadCell<'a>> LoadCell<'a> for Rc<T> {
    #[inline]
    fn load_from_cell(cell: &'a DynCell) -> Result<Self, Error> {
        match <T as LoadCell>::load_from_cell(cell) {
            Ok(value) => Ok(Rc::new(value)),
            Err(e) => Err(e),
        }
    }
}

/// A data structure that can be deserialized from ordinary cells.
pub trait Load<'a>: Sized {
    /// Tries to load itself from a cell slice.
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error>;
}

impl<'a, T: Load<'a>> Load<'a> for Box<T> {
    #[inline]
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        match <T as Load>::load_from(slice) {
            Ok(value) => Ok(Box::new(value)),
            Err(e) => Err(e),
        }
    }
}

impl<'a, T: Load<'a>> Load<'a> for Arc<T> {
    #[inline]
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        match <T as Load>::load_from(slice) {
            Ok(value) => Ok(Arc::new(value)),
            Err(e) => Err(e),
        }
    }
}

impl<'a, T: Load<'a>> Load<'a> for Rc<T> {
    #[inline]
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        match <T as Load>::load_from(slice) {
            Ok(value) => Ok(Rc::new(value)),
            Err(e) => Err(e),
        }
    }
}

impl<'a> Load<'a> for () {
    #[inline]
    fn load_from(_: &mut CellSlice<'a>) -> Result<Self, Error> {
        Ok(())
    }
}

macro_rules! impl_load_for_tuples {
    ($( ($($t:ident),+) ),*$(,)?) => {$(
        impl<'a, $($t: Load<'a>),+> Load<'a> for ($($t),*,) {
            fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
                Ok(($(ok!(<$t>::load_from(slice))),+))
            }
        }
    )*};
}

impl_load_for_tuples! {
    (T0, T1),
    (T0, T1, T2),
    (T0, T1, T2, T3),
    (T0, T1, T2, T3, T4),
    (T0, T1, T2, T3, T4, T5),
}

impl<'a, T: Load<'a>> Load<'a> for Option<T> {
    #[inline]
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        if ok!(slice.load_bit()) {
            match T::load_from(slice) {
                Ok(value) => Ok(Some(value)),
                Err(e) => Err(e),
            }
        } else {
            Ok(None)
        }
    }
}

impl<T: ExactSize> ExactSize for Option<T> {
    #[inline]
    fn exact_size(&self) -> Size {
        let mut total = Size { bits: 1, refs: 0 };
        if let Some(this) = self {
            total += this.exact_size();
        }
        total
    }
}

impl<'a> Load<'a> for CellSlice<'a> {
    #[inline]
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        Ok(slice.load_remaining())
    }
}

macro_rules! ok_map {
    ($expr:expr => $ty:ty) => {
        match $expr {
            core::result::Result::Ok(s) => core::result::Result::Ok(s as $ty),
            core::result::Result::Err(e) => core::result::Result::Err(e),
        }
    };
}

macro_rules! impl_primitive_loads {
    ($($type:ty => |$s:ident| $expr:expr),*$(,)?) => {
        $(impl Load<'_> for $type {
            #[inline]
            fn load_from($s: &mut CellSlice) -> Result<Self, Error> {
                $expr
            }
        })*
    };
}

impl_primitive_loads! {
    bool => |s| s.load_bit(),
    u8 => |s| s.load_u8(),
    i8 => |s| ok_map!(s.load_u8() => i8),
    u16 => |s| s.load_u16(),
    i16 => |s| ok_map!(s.load_u16() => i16),
    u32 => |s| s.load_u32(),
    i32 => |s| ok_map!(s.load_u32() => i32),
    u64 => |s| s.load_u64(),
    i64 => |s| ok_map!(s.load_u64() => i64),
    u128 => |s| s.load_u128(),
    i128 => |s| ok_map!(s.load_u128() => i128),
    NonZeroU8 => |s| match s.load_u8() {
        Ok(s) => match NonZeroU8::new(s) {
            Some(s) => Ok(s),
            None => Err(Error::InvalidData)
        }
        Err(e) => Err(e),
    },
    NonZeroU16 => |s| match s.load_u16() {
        Ok(s) => match NonZeroU16::new(s) {
            Some(s) => Ok(s),
            None => Err(Error::InvalidData)
        }
        Err(e) => Err(e),
    },
    NonZeroU32 => |s| match s.load_u32() {
        Ok(s) => match NonZeroU32::new(s) {
            Some(s) => Ok(s),
            None => Err(Error::InvalidData)
        }
        Err(e) => Err(e),
    },
    HashBytes => |s| s.load_u256(),
}

impl<'a> Load<'a> for &'a DynCell {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        slice.load_reference()
    }
}

impl ExactSize for DynCell {
    #[inline]
    fn exact_size(&self) -> Size {
        Size { bits: 0, refs: 1 }
    }
}

impl<'a> Load<'a> for Cell {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        slice.load_reference_cloned()
    }
}

impl ExactSize for Cell {
    #[inline]
    fn exact_size(&self) -> Size {
        Size { bits: 0, refs: 1 }
    }
}

/// Owned cell slice parts alias.
pub type CellSliceParts = (CellSliceRange, Cell);

impl ExactSize for CellSliceParts {
    #[inline]
    fn exact_size(&self) -> Size {
        self.0.size()
    }
}

/// Indices of the slice data and refs windows.
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, Hash)]
pub struct CellSliceRange {
    bits_start: u16,
    bits_end: u16,
    refs_start: u8,
    refs_end: u8,
}

impl CellSliceRange {
    /// Returns an empty slice range.
    pub const fn empty() -> Self {
        CellSliceRange {
            bits_start: 0,
            bits_end: 0,
            refs_start: 0,
            refs_end: 0,
        }
    }

    /// Returns a full range for the specified cell.
    #[inline]
    pub fn full<T>(cell: &T) -> Self
    where
        T: AsRef<DynCell> + ?Sized,
    {
        let cell = cell.as_ref();
        Self {
            bits_start: 0,
            bits_end: cell.bit_len(),
            refs_start: 0,
            refs_end: cell.reference_count(),
        }
    }

    /// Constructs a new cell slice from the specified cell using the current range.
    /// Returns an error if the cell is pruned.
    ///
    /// NOTE: the resulting range will be truncated to cell bounds.
    #[inline]
    pub fn apply<T>(self, cell: &T) -> Result<CellSlice<'_>, Error>
    where
        T: AsRef<DynCell> + ?Sized,
    {
        fn apply_impl(range: CellSliceRange, cell: &DynCell) -> Result<CellSlice<'_>, Error> {
            // Handle pruned branch access
            if unlikely(cell.is_exotic()) {
                Err(Error::UnexpectedExoticCell)
            } else {
                let bits_end = std::cmp::min(range.bits_end, cell.bit_len());
                let refs_end = std::cmp::min(range.refs_end, cell.reference_count());
                Ok(CellSlice {
                    range: CellSliceRange {
                        bits_start: std::cmp::min(range.bits_start, bits_end),
                        bits_end,
                        refs_start: std::cmp::min(range.refs_start, refs_end),
                        refs_end,
                    },
                    cell,
                })
            }
        }
        apply_impl(self, cell.as_ref())
    }

    /// Constructs a new cell slice from the specified cell using the current range.
    ///
    /// NOTE: the resulting range will be truncated to cell bounds.
    ///
    /// # Safety
    ///
    /// The following must be true:
    /// - cell is not pruned
    /// - range is in cell bounds
    pub fn apply_allow_exotic<T>(self, cell: &T) -> CellSlice<'_>
    where
        T: AsRef<DynCell> + ?Sized,
    {
        CellSlice {
            range: self,
            cell: cell.as_ref(),
        }
    }

    /// Returns the number of remaining bits and refs in the slice.
    pub const fn size(&self) -> Size {
        Size {
            bits: self.size_bits(),
            refs: self.size_refs(),
        }
    }

    /// Returns the number of remaining bits of data in the slice.
    pub const fn size_bits(&self) -> u16 {
        self.bits_end.saturating_sub(self.bits_start)
    }

    /// Returns the number of remaining references in the slice.
    pub const fn size_refs(&self) -> u8 {
        self.refs_end.saturating_sub(self.refs_start)
    }

    /// Returns whether there are no data bits and refs left.
    pub const fn is_empty(&self) -> bool {
        self.is_data_empty() && self.is_refs_empty()
    }

    /// Returns whether there are no bits of data left.
    pub const fn is_data_empty(&self) -> bool {
        self.bits_start >= self.bits_end
    }

    /// Returns whether there are no references left.
    pub const fn is_refs_empty(&self) -> bool {
        self.refs_start >= self.refs_end
    }

    /// Returns the start of data and reference windows.
    pub const fn offset(&self) -> Size {
        Size {
            bits: self.bits_start,
            refs: self.refs_start,
        }
    }

    /// Returns the start of the data window.
    pub const fn offset_bits(&self) -> u16 {
        self.bits_start
    }

    /// Returns the start of the references window.
    pub const fn offset_refs(&self) -> u8 {
        self.refs_start
    }

    /// Returns true if the slice contains at least `bits` and `refs`.
    pub const fn has_remaining(&self, bits: u16, refs: u8) -> bool {
        self.bits_start + bits <= self.bits_end && self.refs_start + refs <= self.refs_end
    }

    /// Tries to advance the start of data and refs windows.
    pub fn skip_first(&mut self, bits: u16, refs: u8) -> Result<(), Error> {
        if unlikely(
            self.bits_start + bits > self.bits_end || self.refs_start + refs > self.refs_end,
        ) {
            return Err(Error::CellUnderflow);
        }

        self.bits_start += bits;
        self.refs_start += refs;
        Ok(())
    }

    /// Leaves only the first `bits` and `refs` in the slice.
    pub fn only_first(&mut self, bits: u16, refs: u8) -> Result<(), Error> {
        if unlikely(
            self.bits_start + bits > self.bits_end || self.refs_start + refs > self.refs_end,
        ) {
            return Err(Error::CellUnderflow);
        }

        self.bits_end = self.bits_start + bits;
        self.refs_end = self.refs_start + refs;
        Ok(())
    }

    /// Removes the last `bits` and `refs` from the slice.
    pub fn skip_last(&mut self, bits: u16, refs: u8) -> Result<(), Error> {
        if unlikely(
            self.bits_start + bits > self.bits_end || self.refs_start + refs > self.refs_end,
        ) {
            return Err(Error::CellUnderflow);
        }

        self.bits_end -= bits;
        self.refs_end -= refs;
        Ok(())
    }

    /// Leaves only the last `bits` and `refs` in the slice.
    pub fn only_last(&mut self, bits: u16, refs: u8) -> Result<(), Error> {
        if unlikely(
            self.bits_start + bits > self.bits_end || self.refs_start + refs > self.refs_end,
        ) {
            return Err(Error::CellUnderflow);
        }

        self.bits_start = self.bits_end - bits;
        self.refs_start = self.refs_end - refs;
        Ok(())
    }

    /// Returns the first `bits` and `refs` of the slice and advances the start
    /// of data and refs windows.
    #[must_use = "use `skip_first` if you don't need the result"]
    pub fn split_prefix(&mut self, bits: u16, refs: u8) -> Result<Self, Error> {
        if unlikely(
            self.bits_start + bits > self.bits_end || self.refs_start + refs > self.refs_end,
        ) {
            return Err(Error::CellUnderflow);
        }

        let mut res = *self;
        self.bits_start += bits;
        self.refs_start += refs;
        res.bits_end = self.bits_start;
        res.refs_end = self.refs_start;
        Ok(res)
    }

    /// Returns the last `bits` and `refs` of the slice and shrinks the data and refs windows.
    #[must_use = "use `skip_last` if you don't need the result"]
    pub fn split_suffix(&mut self, bits: u16, refs: u8) -> Result<Self, Error> {
        if unlikely(
            self.bits_start + bits > self.bits_end || self.refs_start + refs > self.refs_end,
        ) {
            return Err(Error::CellUnderflow);
        }

        let mut res = *self;
        self.bits_end -= bits;
        self.refs_end -= refs;
        res.bits_start = self.bits_end;
        res.refs_start = self.refs_end;
        Ok(res)
    }

    /// Returns a slice range starting at the same bits and refs offsets,
    /// and containing no more than `bits` of data and `refs` of children.
    pub fn get_prefix(&self, bits: u16, refs: u8) -> Self {
        Self {
            bits_start: self.bits_start,
            bits_end: std::cmp::min(self.bits_start + bits, self.bits_end),
            refs_start: self.refs_start,
            refs_end: std::cmp::min(self.refs_start + refs, self.refs_end),
        }
    }

    /// Returns whether this range has the same size as the cell.
    #[inline]
    pub fn is_full<T>(&self, cell: &T) -> bool
    where
        T: AsRef<DynCell> + ?Sized,
    {
        let cell = cell.as_ref();
        self.bits_start == 0
            && self.refs_start == 0
            && self.bits_end == cell.bit_len()
            && self.refs_end == cell.reference_count()
    }
}

/// A read-only view for a subrange of a cell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CellSlice<'a> {
    cell: &'a DynCell,
    range: CellSliceRange,
}

impl Default for CellSlice<'_> {
    #[inline]
    fn default() -> Self {
        Cell::empty_cell_ref().as_slice_allow_exotic()
    }
}

impl<'a> AsRef<CellSlice<'a>> for CellSlice<'a> {
    #[inline]
    fn as_ref(&self) -> &CellSlice<'a> {
        self
    }
}

impl<'a> AsMut<CellSlice<'a>> for CellSlice<'a> {
    #[inline]
    fn as_mut(&mut self) -> &mut CellSlice<'a> {
        self
    }
}

impl<'a> CellSlice<'a> {
    /// Constructs a new cell slice from the specified cell.
    /// Returns an error if the cell is not ordinary.
    pub fn new(cell: &'a DynCell) -> Result<Self, Error> {
        // Handle pruned branch access
        if unlikely(cell.is_exotic()) {
            Err(Error::UnexpectedExoticCell)
        } else {
            Ok(Self {
                range: CellSliceRange::full(cell),
                cell,
            })
        }
    }

    /// Constructs a new cell slice from the specified cell.
    pub fn new_allow_exotic(cell: &'a DynCell) -> Self {
        Self {
            range: CellSliceRange::full(cell),
            cell,
        }
    }

    /// Alias for [`CellSliceRange::apply`].
    ///
    /// NOTE: the resulting range will be truncated to cell bounds.
    #[inline]
    pub fn apply((range, cell): &'a CellSliceParts) -> Result<Self, Error> {
        range.apply(cell)
    }

    /// Alias for [`CellSliceRange::apply_allow_exotic`].
    ///
    /// # Safety
    ///
    /// The following must be true:
    /// - cell is not pruned
    /// - range is in cell bounds
    #[inline]
    pub fn apply_allow_exotic((range, cell): &'a CellSliceParts) -> Self {
        range.apply_allow_exotic(cell)
    }

    /// Returns an underlying range indices.
    #[inline]
    pub const fn range(&self) -> CellSliceRange {
        self.range
    }

    /// Returns a reference to the underlying cell.
    #[inline]
    pub const fn cell(&self) -> &'a DynCell {
        self.cell
    }

    /// Computes cell type from descriptor bytes.
    #[inline]
    pub fn cell_type(&self) -> CellType {
        self.cell.cell_type()
    }

    /// Computes the cell level from the level mask.
    #[inline]
    pub fn level(&self) -> u8 {
        self.cell.level()
    }

    /// Computes the level mask from the descriptor bytes.
    #[inline]
    pub fn level_mask(&self) -> LevelMask {
        self.cell.level_mask()
    }

    /// Returns whether there are no data bits and refs left.
    pub const fn is_empty(&self) -> bool {
        self.range.is_empty()
    }

    /// Returns whether there are no bits of data left.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tycho_types::prelude::{Cell, CellFamily, CellBuilder};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// // Cell with empty data
    /// let empty_cell = Cell::empty_cell();
    /// assert!(empty_cell.as_slice()?.is_data_empty());
    ///
    /// // Cell with some bits in data
    /// let not_empty_cell = {
    ///     let mut builder = CellBuilder::new();
    ///     builder.store_bit_zero()?;
    ///     builder.build()?
    /// };
    /// assert!(!not_empty_cell.as_slice()?.is_data_empty());
    /// # Ok(()) }
    /// ```
    pub const fn is_data_empty(&self) -> bool {
        self.range.is_data_empty()
    }

    /// Returns whether there are no references left.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tycho_types::prelude::{Cell, CellFamily, CellBuilder};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// // Cell without references
    /// let empty_cell = Cell::empty_cell();
    /// assert!(empty_cell.as_slice()?.is_refs_empty());
    ///
    /// // Cell with some references
    /// let not_empty_cell = {
    ///     let mut builder = CellBuilder::new();
    ///     builder.store_reference(empty_cell)?;
    ///     builder.build()?
    /// };
    /// assert!(!not_empty_cell.as_slice()?.is_refs_empty());
    /// # Ok(()) }
    /// ```
    pub const fn is_refs_empty(&self) -> bool {
        self.range.is_refs_empty()
    }

    /// Returns the number of remaining bits and refs in the slice.
    pub const fn size(&self) -> Size {
        self.range.size()
    }

    /// Returns the number of remaining bits of data in the slice.
    pub const fn size_bits(&self) -> u16 {
        self.range.size_bits()
    }

    /// Returns the number of remaining references in the slice.
    pub const fn size_refs(&self) -> u8 {
        self.range.size_refs()
    }

    /// Returns the start of data and reference windows.
    pub const fn offset(&self) -> Size {
        self.range.offset()
    }

    /// Returns the start of the data window.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tycho_types::prelude::CellBuilder;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let cell = {
    ///     let mut builder = CellBuilder::new();
    ///     builder.store_zeros(100)?;
    ///     builder.build()?
    /// };
    /// let mut slice = cell.as_slice()?;
    /// slice.load_u8()?;
    /// assert_eq!(slice.offset_bits(), 8);
    /// # Ok(()) }
    /// ```
    #[inline]
    pub const fn offset_bits(&self) -> u16 {
        self.range.offset_bits()
    }

    /// Returns the start of the references window.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tycho_types::prelude::{Cell, CellFamily, CellBuilder};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let cell = {
    ///     let mut builder = CellBuilder::new();
    ///     builder.store_reference(Cell::empty_cell())?;
    ///     builder.build()?
    /// };
    /// let mut slice = cell.as_slice()?;
    ///
    /// slice.load_reference()?;
    /// assert_eq!(slice.offset_refs(), 1);
    /// # Ok(()) }
    /// ```
    #[inline]
    pub const fn offset_refs(&self) -> u8 {
        self.range.offset_refs()
    }

    /// Returns true if the slice contains at least `bits` and `refs`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tycho_types::prelude::{Cell, CellBuilder, CellFamily};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let cell = {
    ///     let mut builder = CellBuilder::new();
    ///     builder.store_zeros(100)?;
    ///     builder.store_reference(Cell::empty_cell())?;
    ///     builder.store_reference(Cell::empty_cell())?;
    ///     builder.build()?
    /// };
    /// let mut slice = cell.as_slice()?;
    ///
    /// assert!(slice.has_remaining(10, 2));
    /// assert!(!slice.has_remaining(500, 2)); // too many bits
    /// assert!(!slice.has_remaining(0, 4)); // too many refs
    /// //
    /// # Ok(()) }
    /// ```
    #[inline]
    pub const fn has_remaining(&self, bits: u16, refs: u8) -> bool {
        self.range.has_remaining(bits, refs)
    }

    /// Returns whether this slice is untouched.
    #[inline]
    pub fn is_full(&self) -> bool {
        self.range.is_full(self.cell)
    }

    /// Recursively computes the count of distinct cells returning
    /// the total storage used by this dag taking into account the
    /// identification of equal cells.
    ///
    /// Root slice does not count as cell. A slice subrange of
    /// cells is used during computation.
    pub fn compute_unique_stats(&self, limit: usize) -> Option<CellTreeStats> {
        StorageStat::compute_for_slice(self, limit)
    }

    /// Tries to advance the start of data and refs windows.
    pub fn skip_first(&mut self, bits: u16, refs: u8) -> Result<(), Error> {
        self.range.skip_first(bits, refs)
    }

    /// Leaves only the first `bits` and `refs` in the slice.
    pub fn only_first(&mut self, bits: u16, refs: u8) -> Result<(), Error> {
        self.range.only_first(bits, refs)
    }

    /// Removes the last `bits` and `refs` from the slice.
    pub fn skip_last(&mut self, bits: u16, refs: u8) -> Result<(), Error> {
        self.range.skip_last(bits, refs)
    }

    /// Leaves only the last `bits` and `refs` in the slice.
    pub fn only_last(&mut self, bits: u16, refs: u8) -> Result<(), Error> {
        self.range.only_last(bits, refs)
    }

    /// Lexicographically compares slice data.
    ///
    /// NOTE: this method is quite computationally heavy as it compares the content
    /// of two potentially unaligned slices. Use it with caution or check by cell.
    pub fn lex_cmp(&self, rhs: &CellSlice) -> Result<std::cmp::Ordering, Error> {
        use std::cmp::Ordering;

        let lhs = self;
        let lhs_bits = lhs.size_bits();
        let rhs_bits = rhs.size_bits();

        // Fast check

        // NOTE: We can ignore pointer metadata since we are comparing only bits,
        // and custom implementations of `CellImpl` do not alter data behavior.
        if std::ptr::addr_eq(lhs.cell, rhs.cell) && lhs.offset_bits() == rhs.offset_bits() {
            return Ok(lhs_bits.cmp(&rhs_bits));
        }

        // Full check
        let bits = std::cmp::min(lhs_bits, rhs_bits);
        let rem = bits % 32;
        for offset in (0..bits - rem).step_by(32) {
            match ok!(lhs.get_u32(offset)).cmp(&ok!(rhs.get_u32(offset))) {
                Ordering::Equal => {}
                ord => return Ok(ord),
            }
        }

        if rem > 0 {
            match ok!(lhs.get_uint(bits - rem, rem)).cmp(&ok!(rhs.get_uint(bits - rem, rem))) {
                Ordering::Equal => {}
                ord => return Ok(ord),
            }
        }

        Ok(lhs_bits.cmp(&rhs_bits))
    }

    /// Returns `true` if two slices have the same data bits and refs.
    ///
    /// NOTE: this method is quite computationally heavy as it compares the content
    /// of two potentially unaligned slices. Use it with caution or check by cell.
    pub fn contents_eq(&self, rhs: &CellSlice) -> Result<bool, Error> {
        let lhs = self;

        // Fast check
        if lhs.size_bits() != rhs.size_bits() || lhs.size_refs() != rhs.size_refs() {
            return Ok(false);
        }

        // Full check
        if ok!(self.lex_cmp(rhs)).is_ne() {
            return Ok(false);
        }

        for (lhs, rhs) in self.references().zip(rhs.references()) {
            if lhs.repr_hash() != rhs.repr_hash() {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Returns a slice starting at the same bits and refs offsets,
    /// and containing no more than `bits` of data and `refs` of children.
    pub fn get_prefix(&self, bits: u16, refs: u8) -> Self {
        Self {
            cell: self.cell,
            range: self.range.get_prefix(bits, refs),
        }
    }

    /// Returns `true` if this slice data is a prefix of the other slice data.
    pub fn is_data_prefix_of(&self, other: &Self) -> Result<bool, Error> {
        let bits = self.size_bits();
        if bits > other.size_bits() {
            return Ok(false);
        }

        let mut other = *other;
        let ok = other.only_first(bits, 0).is_ok();
        debug_assert!(ok);

        Ok(ok!(self.lex_cmp(&other)).is_eq())
    }

    /// Returns `true` if this slice data is a suffix of the other slice data.
    pub fn is_data_suffix_of(&self, other: &Self) -> Result<bool, Error> {
        let bits = self.size_bits();
        if bits > other.size_bits() {
            return Ok(false);
        }

        let mut other = *other;
        let ok = other.only_last(bits, 0).is_ok();
        debug_assert!(ok);

        Ok(ok!(self.lex_cmp(&other)).is_eq())
    }

    /// Returns a subslice with the data prefix removed.
    ///
    /// If the slice starts with `prefix`, returns the subslice after the prefix, wrapped in `Some`.
    /// If `prefix` is empty, simply returns the original slice.
    ///
    /// If the slice does not start with `prefix`, returns `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tycho_types::prelude::CellBuilder;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let cell = {
    ///     let mut builder = CellBuilder::new();
    ///     builder.store_u32(0xdeadbeaf)?;
    ///     builder.build()?
    /// };
    /// let slice = cell.as_slice()?;
    ///
    /// let prefix = {
    ///     let mut builder = CellBuilder::new();
    ///     builder.store_u16(0xdead)?;
    ///     builder.build()?
    /// };
    ///
    /// let without_prefix = slice.strip_data_prefix(&prefix.as_slice()?).unwrap();
    /// assert_eq!(without_prefix.get_u16(0)?, 0xbeaf);
    /// # Ok(()) }
    /// ```
    pub fn strip_data_prefix<'b>(&self, prefix: &CellSlice<'b>) -> Option<CellSlice<'a>> {
        let prefix_len = prefix.size_bits();
        if prefix_len == 0 {
            Some(*self)
        } else if self.size_bits() < prefix_len {
            None
        } else {
            let mut result = *self;
            let lcp = self.longest_common_data_prefix_impl(prefix, prefix_len);
            if prefix_len <= lcp && result.skip_first(prefix_len, 0).is_ok() {
                Some(result)
            } else {
                None
            }
        }
    }

    /// Returns the longest common data prefix.
    ///
    /// NOTE: The returned subslice will be a subslice of the current slice.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tycho_types::prelude::CellBuilder;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let cell = {
    ///     let mut builder = CellBuilder::new();
    ///     builder.store_u32(0xdeadbeaf)?;
    ///     builder.build()?
    /// };
    /// let slice = cell.as_slice()?;
    ///
    /// let prefix = {
    ///     let mut builder = CellBuilder::new();
    ///     builder.store_u16(0xdead)?;
    ///     builder.build()?
    /// };
    ///
    /// let lcp = slice.longest_common_data_prefix(&prefix.as_slice()?);
    /// assert_eq!(lcp.get_u16(0)?, 0xdead);
    /// assert_eq!(lcp.size_bits(), 16);
    /// # Ok(()) }
    /// ```
    pub fn longest_common_data_prefix(&self, other: &Self) -> Self {
        let prefix_len = self.longest_common_data_prefix_impl(other, u16::MAX);
        self.get_prefix(prefix_len, 0)
    }

    fn longest_common_data_prefix_impl(&self, other: &Self, max_hint: u16) -> u16 {
        if self.range.bits_start >= self.range.bits_end
            || other.range.bits_start >= other.range.bits_end
        {
            return 0;
        }
        let self_remaining_bits = self.range.bits_end - self.range.bits_start;
        let self_data = self.cell.data();
        let other_remaining_bits = other.range.bits_end - other.range.bits_start;
        let other_data = other.cell.data();

        // Compute max prefix length in bits
        let max_bit_len = std::cmp::min(self_remaining_bits, other_remaining_bits).min(max_hint);

        // Compute shifts and data offsets
        let self_r = self.range.bits_start % 8;
        let self_q = (self.range.bits_start / 8) as usize;
        let other_r = other.range.bits_start % 8;
        let other_q = (other.range.bits_start / 8) as usize;

        // Compute remaining bytes to check
        let self_bytes = (self_r + max_bit_len).div_ceil(8) as usize;
        debug_assert!((self_q + self_bytes) <= self_data.len());
        let other_bytes = (other_r + max_bit_len).div_ceil(8) as usize;
        debug_assert!((other_q + other_bytes) <= other_data.len());

        let aligned_bytes = std::cmp::min(self_bytes, other_bytes);

        let mut prefix_len: u16 = 0;

        unsafe {
            let self_data_ptr = self_data.as_ptr().add(self_q);
            let other_data_ptr = other_data.as_ptr().add(other_q);

            // Get first bytes aligned to the left
            let mut self_byte = *self_data_ptr << self_r;
            let mut other_byte = *other_data_ptr << other_r;

            // For all aligned bytes except the first
            for i in 1..aligned_bytes {
                // Concat previous bits with current bits
                // NOTE: shift as `u16` to allow overflow
                let next_self_byte = *self_data_ptr.add(i);
                self_byte |= ((next_self_byte as u16) >> (8 - self_r)) as u8;
                let next_other_byte = *other_data_ptr.add(i);
                other_byte |= ((next_other_byte as u16) >> (8 - other_r)) as u8;

                // XOR bytes to check equality
                match self_byte ^ other_byte {
                    // All bits are equal, update current bytes and move forward
                    0 => {
                        prefix_len += 8;
                        self_byte = next_self_byte << self_r;
                        other_byte = next_other_byte << other_r;
                    }
                    // Some bits are not equal
                    x => {
                        // Number of leading zeros is the number of equal bits
                        return std::cmp::min(prefix_len + x.leading_zeros() as u16, max_bit_len);
                    }
                }
            }

            // Concat remaining bits
            if self_r > 0 && aligned_bytes < self_bytes {
                self_byte |= *self_data_ptr.add(aligned_bytes) >> (8 - self_r);
            }
            if other_r > 0 && aligned_bytes < other_bytes {
                other_byte |= *other_data_ptr.add(aligned_bytes) >> (8 - other_r);
            }

            // Apply last byte mask
            let last_byte_mask = 0xff << ((8 - max_bit_len % 8) % 8);
            self_byte &= last_byte_mask;
            other_byte &= last_byte_mask;

            // Count the number of remaining equal bits
            prefix_len += (self_byte ^ other_byte).leading_zeros() as u16;
        }

        // Return the longest prefix (without equal bits from the last byte mask)
        std::cmp::min(prefix_len, max_bit_len)
    }

    /// Returns the number of leading bits of `self`.
    pub fn count_leading(&self, bit: bool) -> Result<u16, Error> {
        if self.range.bits_start >= self.range.bits_end {
            return Ok(0);
        }
        let data = self.cell.data();

        // Check if data is enough
        if self.range.bits_end.div_ceil(8) as usize > data.len() {
            return Err(Error::CellUnderflow);
        }

        let bit_count = self.range.bits_end - self.range.bits_start;

        // SAFETY: `bits_end` is in data range
        Ok(unsafe { bits_memscan(data, self.range.bits_start, bit_count, bit) })
    }

    /// Returns the number of trailing bits of `self`.
    pub fn count_trailing(&self, bit: bool) -> Result<u16, Error> {
        if self.range.bits_start >= self.range.bits_end {
            return Ok(0);
        }
        let data = self.cell.data();

        // Check if data is enough
        if self.range.bits_end.div_ceil(8) as usize > data.len() {
            return Err(Error::CellUnderflow);
        }

        let bit_count = self.range.bits_end - self.range.bits_start;

        // SAFETY: `bits_end` is in data range
        Ok(unsafe { bits_memscan_rev(data, self.range.bits_start, bit_count, bit) })
    }

    /// Checks whether the current slice consists of the same bits,
    /// returns `None` if there are 0s and 1s, returns `Some(bit)` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tycho_types::prelude::{Cell, CellFamily, CellBuilder};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// // Uniform cell consisting of only 0s
    /// let uniform_cell = {
    ///     let mut builder = CellBuilder::new();
    ///     builder.store_zeros(10)?;
    ///     builder.build()?
    /// };
    /// assert_eq!(uniform_cell.as_slice()?.test_uniform(), Some(false));
    ///
    /// // Non-uniform cell consisting of 0s and 1s
    /// let non_uniform_cell = {
    ///     let mut builder = CellBuilder::new();
    ///     builder.store_zeros(9)?;
    ///     builder.store_bit_one()?;
    ///     builder.build()?
    /// };
    /// assert_eq!(non_uniform_cell.as_slice()?.test_uniform(), None);
    ///
    /// // Empty cell is non-uniform
    /// let non_uniform_cell = Cell::empty_cell();
    /// assert_eq!(non_uniform_cell.as_slice()?.test_uniform(), None);
    /// # Ok(()) }
    /// ```
    pub fn test_uniform(&self) -> Option<bool> {
        if self.range.bits_start >= self.range.bits_end {
            return None;
        }
        let data = self.cell.data();

        // Check if data is enough
        if self.range.bits_end.div_ceil(8) as usize > data.len() {
            return None;
        }

        let mut bit_count = self.range.bits_end - self.range.bits_start;

        let r = self.range.bits_start & 0b111;
        let q = self.range.bits_start >> 3;

        // SAFETY: q is in data range
        let mut data_ptr = unsafe { data.as_ptr().add(q as usize) };
        let first_byte = unsafe { *data_ptr };
        let bit = (first_byte >> (7 - r)) & 1 != 0;

        if bit_count < 64 {
            let target = (bit as u8) * u8::MAX;
            let first_byte_mask: u8 = 0xff >> r;
            let last_byte_mask: u8 = 0xff << ((8 - (bit_count + r) % 8) % 8);

            if r + bit_count <= 8 {
                // Special case if all remaining_bits are in the first byte
                if ((first_byte ^ target) & first_byte_mask & last_byte_mask) != 0 {
                    return None;
                }
            } else {
                // Check the first byte
                if (first_byte ^ target) & first_byte_mask != 0 {
                    return None;
                }

                unsafe {
                    // Check all full bytes
                    bit_count -= 8 - r;
                    for _ in 0..(bit_count / 8) {
                        data_ptr = data_ptr.add(1);
                        if *data_ptr != target {
                            return None;
                        }
                    }

                    // Check the last byte (if not aligned)
                    if !bit_count.is_multiple_of(8) {
                        data_ptr = data_ptr.add(1);
                        if (*data_ptr ^ target) & last_byte_mask != 0 {
                            return None;
                        }
                    }
                }
            }

            Some(bit)
        } else {
            // SAFETY: `bits_end` is in data range
            let same_bits = unsafe { bits_memscan(data, self.range.bits_start, bit_count, bit) };
            (same_bits == bit_count).then_some(bit)
        }
    }

    /// Tries to read the bit at the specified offset (relative to the current bits window).
    pub fn get_bit(&self, offset: u16) -> Result<bool, Error> {
        if self.range.bits_start + offset < self.range.bits_end {
            let index = self.range.bits_start + offset;
            if let Some(byte) = self.cell.data().get((index / 8) as usize) {
                return Ok((byte >> (7 - index % 8)) & 1 != 0);
            }
        }
        Err(Error::CellUnderflow)
    }

    /// Tries to read the next bit, incrementing the bits window start.
    pub fn load_bit(&mut self) -> Result<bool, Error> {
        if self.range.bits_start < self.range.bits_end {
            let index = self.range.bits_start;
            if let Some(byte) = self.cell.data().get((index / 8) as usize) {
                self.range.bits_start += 1;
                return Ok((byte >> (7 - index % 8)) & 1 != 0);
            }
        }
        Err(Error::CellUnderflow)
    }

    /// Reads `u8` starting from the `offset`.
    #[inline]
    pub fn get_u8(&self, offset: u16) -> Result<u8, Error> {
        self.get_small_uint(offset, 8)
    }

    /// Tries to read the next `u8`, incrementing the bits window start.
    #[inline]
    pub fn load_u8(&mut self) -> Result<u8, Error> {
        self.load_small_uint(8)
    }

    /// Reads `u16` starting from the `offset`.
    pub fn get_u16(&self, offset: u16) -> Result<u16, Error> {
        if self.range.bits_start + offset + 16 <= self.range.bits_end {
            let index = self.range.bits_start + offset;
            let data = self.cell.data();
            let data_len = data.len();

            let r = index % 8;
            let q = (index / 8) as usize;

            if r == 0 && q + 2 <= data_len {
                // xxxxxxxx|yyyyyyyy -> xxxxxxxx|yyyyyyyy
                //^r

                // SAFETY: `q + 2 <= data_len`
                Ok(u16::from_be_bytes(unsafe {
                    *(data.as_ptr().add(q) as *const [u8; 2])
                }))
            } else if r != 0 && q + 3 <= data_len {
                // ___xxxxx|yyyyyyyy|zzz_____ -> xxxxxyyy|yyyyyzzz
                //  r^

                let mut bytes = [0u8; 4];

                // SAFETY: `q + 3 <= data_len`
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        data.as_ptr().add(q),
                        bytes.as_mut_ptr().add(1),
                        3,
                    );
                };

                let res = u32::from_be_bytes(bytes);
                Ok((res >> (8 - r)) as u16)
            } else {
                Err(Error::CellUnderflow)
            }
        } else {
            Err(Error::CellUnderflow)
        }
    }

    /// Tries to read the next `u16`, incrementing the bits window start.
    pub fn load_u16(&mut self) -> Result<u16, Error> {
        let res = self.get_u16(0);
        self.range.bits_start += 16 * res.is_ok() as u16;
        res
    }

    /// Reads `u32` starting from the `offset`.
    pub fn get_u32(&self, offset: u16) -> Result<u32, Error> {
        if self.range.bits_start + offset + 32 <= self.range.bits_end {
            let index = self.range.bits_start + offset;
            let data = self.cell.data();
            let data_len = data.len();

            let r = index % 8;
            let q = (index / 8) as usize;

            if r == 0 && q + 4 <= data_len {
                // xxxxxxxx|yyyyyyyy|zzzzzzzz|wwwwwwww -> xxxxxxxx|yyyyyyyy|zzzzzzzz|wwwwwwww
                //^r

                // SAFETY: `q + 4 <= data_len`
                Ok(u32::from_be_bytes(unsafe {
                    *(data.as_ptr().add(q) as *const [u8; 4])
                }))
            } else if r != 0 && q + 5 <= data_len {
                // ___xxxxx|yyyyyyyy|zzz_____ -> xxxxxyyy|yyyyyzzz
                //  r^

                let mut bytes = [0u8; 8];

                // SAFETY: `q + 5 <= data_len`
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        data.as_ptr().add(q),
                        bytes.as_mut_ptr().add(3),
                        5,
                    );
                };

                let res = u64::from_be_bytes(bytes);
                Ok((res >> (8 - r)) as u32)
            } else {
                Err(Error::CellUnderflow)
            }
        } else {
            Err(Error::CellUnderflow)
        }
    }

    /// Tries to read the next `u32`, incrementing the bits window start.
    pub fn load_u32(&mut self) -> Result<u32, Error> {
        let res = self.get_u32(0);
        self.range.bits_start += 32 * res.is_ok() as u16;
        res
    }

    /// Reads `u64` starting from the `offset`.
    pub fn get_u64(&self, offset: u16) -> Result<u64, Error> {
        if self.range.bits_start + offset + 64 <= self.range.bits_end {
            let index = self.range.bits_start + offset;
            let data = self.cell.data();
            let data_len = data.len();

            let r = index % 8;
            let q = (index / 8) as usize;

            if r == 0 && q + 8 <= data_len {
                // SAFETY: `q + 8 <= data_len`
                Ok(u64::from_be_bytes(unsafe {
                    *(data.as_ptr().add(q) as *const [u8; 8])
                }))
            } else if r != 0 && q + 9 <= data_len {
                // ___xxxxx|...|zzz_____ -> xxxxx...|...zzz
                //  r^

                let mut bytes = [0u8; 16];

                // SAFETY: `q + 9 <= data_len`
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        data.as_ptr().add(q),
                        bytes.as_mut_ptr().add(7),
                        9,
                    );
                };

                let res = u128::from_be_bytes(bytes);
                Ok((res >> (8 - r)) as u64)
            } else {
                Err(Error::CellUnderflow)
            }
        } else {
            Err(Error::CellUnderflow)
        }
    }

    /// Tries to read the next `u64`, incrementing the bits window start.
    pub fn load_u64(&mut self) -> Result<u64, Error> {
        let res = self.get_u64(0);
        self.range.bits_start += 64 * res.is_ok() as u16;
        res
    }

    /// Reads `u128` starting from the `offset`.
    pub fn get_u128(&self, offset: u16) -> Result<u128, Error> {
        if self.range.bits_start + offset + 128 <= self.range.bits_end {
            let index = self.range.bits_start + offset;
            let data = self.cell.data();
            let data_len = data.len();

            let r = index % 8;
            let q = (index / 8) as usize;

            if r == 0 && q + 16 <= data_len {
                // SAFETY: `q + 16 <= data_len`
                Ok(u128::from_be_bytes(unsafe {
                    *(data.as_ptr().add(q) as *const [u8; 16])
                }))
            } else if r != 0 && q + 17 <= data_len {
                // ___xxxxx|...|zzz_____ -> xxxxx...|...zzz
                //  r^

                let mut bytes = [0u8; 17];

                // SAFETY: `q + 17 <= data_len`
                unsafe {
                    std::ptr::copy_nonoverlapping(data.as_ptr().add(q), bytes.as_mut_ptr(), 17);
                };

                let res = u128::from_be_bytes(bytes[1..].try_into().unwrap());
                Ok(((bytes[0] as u128) << (120 + r)) | (res >> (8 - r)))
            } else {
                Err(Error::CellUnderflow)
            }
        } else {
            Err(Error::CellUnderflow)
        }
    }

    /// Tries to read the next `u128`, incrementing the bits window start.
    pub fn load_u128(&mut self) -> Result<u128, Error> {
        let res = self.get_u128(0);
        self.range.bits_start += 128 * res.is_ok() as u16;
        res
    }

    /// Reads 32 bytes starting from the `offset`.
    pub fn get_u256(&self, offset: u16) -> Result<HashBytes, Error> {
        if self.range.bits_start + offset + 256 <= self.range.bits_end {
            let index = self.range.bits_start + offset;
            let data = self.cell.data();
            let data_len = data.len();

            let r = index % 8;
            let q = (index / 8) as usize;

            if r == 0 && q + 32 <= data_len {
                // SAFETY: `q + 32 <= data_len`
                Ok(HashBytes(unsafe {
                    *(data.as_ptr().add(q) as *const [u8; 32])
                }))
            } else if r != 0 && q + 33 <= data_len {
                // ___xxxxx|...|zzz_____ -> xxxxx...|...zzz
                //  r^

                let shift = 8 - r;
                let rev_shift = 120 + r;

                // SAFETY: `q + 33 <= data_len`
                unsafe {
                    let mut bytes = [0u8; 33];
                    std::ptr::copy_nonoverlapping(data.as_ptr().add(q), bytes.as_mut_ptr(), 33);

                    // Interpret last 32 bytes as two u128
                    let [ovf, bytes @ ..] = bytes;
                    let [mut hi, mut lo]: [u128; 2] = std::mem::transmute(bytes);

                    // Numbers are in big endian order, swap bytes on little endian arch
                    #[cfg(target_endian = "little")]
                    {
                        hi = hi.swap_bytes();
                        lo = lo.swap_bytes();
                    }

                    // Shift right, putting `ovf` to the high bits
                    Ok(std::mem::transmute::<[[u8; 16]; 2], HashBytes>([
                        ((hi >> shift) | ((ovf as u128) << rev_shift)).to_be_bytes(),
                        ((lo >> shift) | (hi << rev_shift)).to_be_bytes(),
                    ]))
                }
            } else {
                Err(Error::CellUnderflow)
            }
        } else {
            Err(Error::CellUnderflow)
        }
    }

    /// Tries to read the next 32 bytes, incrementing the bits window start.
    pub fn load_u256(&mut self) -> Result<HashBytes, Error> {
        let res = self.get_u256(0);
        self.range.bits_start += 256 * res.is_ok() as u16;
        res
    }

    /// Returns a small subset of `bits` (0..=8) starting from the `offset`.
    ///
    /// NOTE: Reading zero bits always succeeds,
    /// and reading more than 8 bits always fails.
    pub fn get_small_uint(&self, offset: u16, bits: u16) -> Result<u8, Error> {
        if bits == 0 {
            return Ok(0);
        }

        if bits <= 8 && self.range.bits_start + offset + bits <= self.range.bits_end {
            let index = self.range.bits_start + offset;

            let r = index % 8;
            let q = (index / 8) as usize;
            let Some(&byte) = self.cell.data().get(q) else {
                return Err(Error::CellUnderflow);
            };

            if r == 0 {
                // xxx_____ -> _____xxx
                //^r
                Ok(byte >> (8 - bits))
            } else if bits <= (8 - r) {
                // __xxx___ -> _____xxx
                // r^
                Ok((byte >> (8 - r - bits)) & ((1 << bits) - 1))
            } else {
                // ______xx|y_______ -> _____xxy
                //     r^

                let mut res = (byte as u16) << 8;
                let Some(&next_byte) = self.cell.data().get(q + 1) else {
                    return Err(Error::CellUnderflow);
                };

                res |= next_byte as u16;
                Ok((res >> (8 - r)) as u8 >> (8 - bits))
            }
        } else {
            Err(Error::CellUnderflow)
        }
    }

    /// Tries to read the next small subset of `bits` (0..=8), incrementing the bits window start.
    ///
    /// NOTE: Reading zero bits always succeeds,
    /// and reading more than 8 bits always fails.
    pub fn load_small_uint(&mut self, bits: u16) -> Result<u8, Error> {
        let res = self.get_small_uint(0, bits);
        self.range.bits_start += bits * res.is_ok() as u16;
        res
    }

    /// Reads `u64` from the cell (but only the specified number of bits)
    /// starting from the `offset`.
    ///
    /// NOTE: Reading zero bits always succeeds,
    /// and reading more than 64 bits always fails.
    pub fn get_uint(&self, offset: u16, mut bits: u16) -> Result<u64, Error> {
        if bits == 0 {
            return Ok(0);
        }

        if bits <= 64 && self.range.bits_start + offset + bits <= self.range.bits_end {
            let index = self.range.bits_start + offset;
            let data = self.cell.data();
            let data_len = data.len();

            // Check if data is enough
            if self.range.bits_end.div_ceil(8) as usize > data_len {
                return Err(Error::CellUnderflow);
            }

            let r = index % 8;
            let q = (index / 8) as usize;

            // SAFETY: remaining bits are at least enough for `data_len`
            unsafe {
                let data_ptr = data.as_ptr().add(q);
                let first_byte = *data_ptr & (0xff >> r);

                let right_shift = (8 - (bits + r) % 8) % 8;

                if r + bits <= 8 {
                    // Special case if all remaining_bits are in the first byte
                    Ok((first_byte >> right_shift) as u64)
                } else {
                    let mut bytes = [0u8; 8];

                    // Copy remaining bytes
                    bits -= 8 - r;
                    std::ptr::copy_nonoverlapping(
                        data_ptr.add(1),
                        bytes.as_mut_ptr(),
                        bits.div_ceil(8) as usize,
                    );

                    let mut result = u64::from_be_bytes(bytes) >> (64 - bits);
                    result |= (first_byte as u64) << bits;
                    Ok(result)
                }
            }
        } else {
            Err(Error::CellUnderflow)
        }
    }

    /// Tries to read the next `u64` (but only the specified number of bits),
    /// incrementing the bits window start.
    ///
    /// NOTE: Reading zero bits always succeeds,
    /// and reading more than 64 bits always fails.
    pub fn load_uint(&mut self, bits: u16) -> Result<u64, Error> {
        let res = self.get_uint(0, bits);
        self.range.bits_start += bits * res.is_ok() as u16;
        res
    }

    /// Loads `bits`-width int from the slice.
    #[cfg(feature = "bigint")]
    pub fn load_bigint(&mut self, bits: u16, signed: bool) -> Result<num_bigint::BigInt, Error> {
        use num_bigint::{BigInt, Sign};

        match bits {
            0 => Ok(BigInt::ZERO),
            1..=64 if !signed => self.load_uint(bits).map(BigInt::from),
            1..=64 => self.load_uint(bits).map(|mut int| {
                if bits < 64 {
                    // Clone sign bit into all high bits
                    int |= ((int >> (bits - 1)) * u64::MAX) << (bits - 1);
                }
                BigInt::from(int as i64)
            }),
            65.. => {
                let rem = bits % 8;
                let mut buffer = [0u8; 128];
                self.load_raw(&mut buffer, bits).map(|buffer| {
                    let mut int = if signed {
                        BigInt::from_signed_bytes_be(buffer)
                    } else {
                        BigInt::from_bytes_be(Sign::Plus, buffer)
                    };
                    if !bits.is_multiple_of(8) {
                        int >>= 8 - rem;
                    }
                    int
                })
            }
        }
    }

    /// Loads `bits`-width int from the slice.
    #[cfg(feature = "bigint")]
    pub fn load_biguint(&mut self, bits: u16) -> Result<num_bigint::BigUint, Error> {
        use num_bigint::BigUint;

        match bits {
            0 => Ok(BigUint::ZERO),
            1..=64 => self.load_uint(bits).map(BigUint::from),
            65.. => {
                let rem = bits % 8;
                let mut buffer = [0u8; 128];
                self.load_raw(&mut buffer, bits).map(|buffer| {
                    let mut int = BigUint::from_bytes_be(buffer);
                    if !bits.is_multiple_of(8) {
                        int >>= 8 - rem;
                    }
                    int
                })
            }
        }
    }

    /// Loads variable length int from the slice.
    #[cfg(feature = "bigint")]
    pub fn load_var_bigint(
        &mut self,
        len_bits: u16,
        signed: bool,
    ) -> Result<num_bigint::BigInt, Error> {
        let len = self.load_uint(len_bits)? as u16;
        self.load_bigint(len * 8, signed)
    }

    /// Loads variable length unsigned int from the slice.
    #[cfg(feature = "bigint")]
    pub fn load_var_biguint(&mut self, len_bits: u16) -> Result<num_bigint::BigUint, Error> {
        let len = self.load_uint(len_bits)? as u16;
        self.load_biguint(len * 8)
    }

    /// Reads the specified number of bits to the target starting from the `offset`.
    pub fn get_raw<'b>(
        &'_ self,
        offset: u16,
        target: &'b mut [u8],
        bits: u16,
    ) -> Result<&'b mut [u8], Error> {
        if bits == 0 {
            return Ok(&mut target[..0]);
        }

        if self.range.bits_start + bits <= self.range.bits_end {
            let index = self.range.bits_start + offset;
            let data = self.cell.data();
            let data_len = data.len();

            let target_len = bits.div_ceil(8) as usize;
            let target = if target_len <= target.len() {
                &mut target[..target_len]
            } else {
                return Err(Error::CellUnderflow);
            };

            let r = index % 8;
            let q = (index / 8) as usize;

            // SAFETY: q will be checked to be in range 0..data_len,
            // r is in range 0..=7, target is guaranteed to be `target_len`
            unsafe {
                let mut data_ptr = data.as_ptr().add(q);
                let target_ptr = target.as_mut_ptr();

                if r == 0 && q + target_len <= data_len {
                    std::ptr::copy_nonoverlapping(data_ptr, target_ptr, target_len);
                } else if r != 0 {
                    let byte_len = (bits + r).div_ceil(8) as usize - 1;
                    if q + byte_len > data_len {
                        return Err(Error::CellUnderflow);
                    }

                    let shift = 8 - r;
                    for i in 0..byte_len {
                        let target = target_ptr.add(i);
                        *target = *data_ptr << r;
                        data_ptr = data_ptr.add(1);
                        *target |= *data_ptr >> shift;
                    }
                    if byte_len < target_len {
                        *target_ptr.add(byte_len) = *data_ptr << r;
                    }
                } else {
                    return Err(Error::CellUnderflow);
                }

                let bits_r = bits % 8;
                if bits_r != 0 {
                    *target_ptr.add(target_len - 1) &= 0xff << (8 - bits_r);
                }
                Ok(target)
            }
        } else {
            Err(Error::CellUnderflow)
        }
    }

    /// Tries to read the specified number of bits, incrementing the bits window start.
    /// Returns the minimum subslice containing all bits.
    pub fn load_raw<'b>(
        &'_ mut self,
        target: &'b mut [u8],
        bits: u16,
    ) -> Result<&'b mut [u8], Error> {
        let res = self.get_raw(0, target, bits);
        self.range.bits_start += bits * res.is_ok() as u16;
        res
    }

    /// Reads all remaining bits and refs into the new slice.
    pub fn load_remaining(&mut self) -> CellSlice<'a> {
        let result = *self;
        self.range.bits_start = self.range.bits_end;
        self.range.refs_start = self.range.refs_end;
        result
    }

    /// Returns the first `bits` and `refs` of the slice and advances the start
    /// of data and refs windows.
    #[must_use = "use `skip_first` if you don't need the result"]
    pub fn load_prefix(&mut self, bits: u16, refs: u8) -> Result<Self, Error> {
        let prefix_range = ok!(self.range.split_prefix(bits, refs));
        Ok(Self {
            cell: self.cell,
            range: prefix_range,
        })
    }

    /// Returns the last `bits` and `refs` of the slice and shrinks the data and refs windows.
    #[must_use = "use `skip_last` if you don't need the result"]
    pub fn load_suffix(&mut self, bits: u16, refs: u8) -> Result<Self, Error> {
        let suffix_range = ok!(self.range.split_suffix(bits, refs));
        Ok(Self {
            cell: self.cell,
            range: suffix_range,
        })
    }

    /// Returns a reference to the Nth child cell (relative to this slice's refs window).
    pub fn get_reference(&self, index: u8) -> Result<&'a DynCell, Error> {
        if self.range.refs_start + index < self.range.refs_end
            && let Some(cell) = self.cell.reference(self.range.refs_start + index)
        {
            return Ok(cell);
        }
        Err(Error::CellUnderflow)
    }

    /// Returns the Nth child cell (relative to this slice's refs window).
    pub fn get_reference_cloned(&self, index: u8) -> Result<Cell, Error> {
        if self.range.refs_start + index < self.range.refs_end
            && let Some(cell) = self.cell.reference_cloned(self.range.refs_start + index)
        {
            return Ok(cell);
        }
        Err(Error::CellUnderflow)
    }

    /// Tries to load the specified child cell as slice.
    /// Returns an error if the loaded cell is absent or is pruned.
    pub fn get_reference_as_slice(&self, index: u8) -> Result<CellSlice<'a>, Error> {
        if self.range.refs_start + index < self.range.refs_end {
            let Some(cell) = self.cell.reference(self.range.refs_start + index) else {
                return Err(Error::CellUnderflow);
            };

            CellSlice::new(cell)
        } else {
            Err(Error::CellUnderflow)
        }
    }

    /// Creates an iterator through child nodes.
    pub fn references(&self) -> RefsIter<'a> {
        RefsIter {
            cell: self.cell,
            max: self.range.refs_end,
            index: self.range.refs_start,
        }
    }

    /// Converts this slice into an iterator through child nodes.
    #[inline]
    pub fn into_references(self) -> RefsIter<'a> {
        self.references()
    }

    /// Returns this slice, but with references skipped.
    #[inline]
    pub fn without_references(mut self) -> Self {
        self.range.refs_start = self.range.refs_end;
        self
    }

    /// Returns a reference to the next child cell (relative to this slice's refs window),
    /// incrementing the refs window start.
    pub fn load_reference(&mut self) -> Result<&'a DynCell, Error> {
        if self.range.refs_start < self.range.refs_end {
            let res = match self.cell.reference(self.range.refs_start) {
                Some(cell) => Ok(cell),
                None => Err(Error::CellUnderflow),
            };
            self.range.refs_start += res.is_ok() as u8;
            res
        } else {
            Err(Error::CellUnderflow)
        }
    }

    /// Returns the next child cell (relative to this slice's refs window),
    /// incrementing the refs window start.
    pub fn load_reference_cloned(&mut self) -> Result<Cell, Error> {
        if self.range.refs_start < self.range.refs_end {
            let res = match self.cell.reference_cloned(self.range.refs_start) {
                Some(cell) => Ok(cell),
                None => Err(Error::CellUnderflow),
            };
            self.range.refs_start += res.is_ok() as u8;
            res
        } else {
            Err(Error::CellUnderflow)
        }
    }

    /// Tries to load the next child cell as slice.
    /// Returns an error if the loaded cell is absent or is pruned.
    ///
    /// NOTE: In case of pruned cell access the current slice remains unchanged.
    pub fn load_reference_as_slice(&mut self) -> Result<CellSlice<'a>, Error> {
        if self.range.refs_start < self.range.refs_end {
            let Some(cell) = self.cell.reference(self.range.refs_start) else {
                return Err(Error::CellUnderflow);
            };

            let res = CellSlice::new(cell);
            self.range.refs_start += res.is_ok() as u8;
            res
        } else {
            Err(Error::CellUnderflow)
        }
    }

    /// Returns an object which will display data as a bitstring
    /// with a termination bit.
    #[inline]
    pub fn display_data<'b: 'a>(&'b self) -> DisplayCellSliceData<'b, 'a> {
        DisplayCellSliceData(self)
    }
}

/// Helper struct to print the cell slice data.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct DisplayCellSliceData<'b, 'a>(&'b CellSlice<'a>);

impl<'b: 'a, 'a> std::fmt::Display for DisplayCellSliceData<'b, 'a> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::LowerHex::fmt(self, f)
    }
}

impl<'b: 'a, 'a> std::fmt::LowerHex for DisplayCellSliceData<'b, 'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut bytes = [0u8; 128];
        std::fmt::LowerHex::fmt(&ok!(make_bitstring(self.0, &mut bytes)), f)
    }
}

impl<'b: 'a, 'a> std::fmt::UpperHex for DisplayCellSliceData<'b, 'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut bytes = [0u8; 128];
        std::fmt::UpperHex::fmt(&ok!(make_bitstring(self.0, &mut bytes)), f)
    }
}

impl<'b: 'a, 'a> std::fmt::Binary for DisplayCellSliceData<'b, 'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut bytes = [0u8; 128];
        std::fmt::Binary::fmt(&ok!(make_bitstring(self.0, &mut bytes)), f)
    }
}

fn make_bitstring<'b: 'a, 'a>(
    s: &'b CellSlice<'a>,
    bytes: &'b mut [u8; 128],
) -> Result<Bitstring<'b>, std::fmt::Error> {
    let bit_len = s.size_bits();
    if s.get_raw(0, bytes, bit_len).is_err() {
        return Err(std::fmt::Error);
    }
    Ok(Bitstring { bytes, bit_len })
}

impl ExactSize for CellSlice<'_> {
    #[inline]
    fn exact_size(&self) -> Size {
        self.size()
    }
}

/// A type with a known size in bits and refs.
pub trait ExactSize {
    /// Exact size of the value when it is stored in a slice.
    fn exact_size(&self) -> Size;
}

impl<T: ExactSize> ExactSize for &T {
    #[inline]
    fn exact_size(&self) -> Size {
        T::exact_size(self)
    }
}

impl<T: ExactSize> ExactSize for &mut T {
    #[inline]
    fn exact_size(&self) -> Size {
        T::exact_size(self)
    }
}

impl<T: ExactSize> ExactSize for Box<T> {
    #[inline]
    fn exact_size(&self) -> Size {
        T::exact_size(self)
    }
}

impl<T: ExactSize> ExactSize for Arc<T> {
    #[inline]
    fn exact_size(&self) -> Size {
        T::exact_size(self)
    }
}

impl<T: ExactSize> ExactSize for Rc<T> {
    #[inline]
    fn exact_size(&self) -> Size {
        T::exact_size(self)
    }
}

impl ExactSize for () {
    #[inline]
    fn exact_size(&self) -> Size {
        Size::ZERO
    }
}

macro_rules! strip_plus {
    (+ $($rest: tt)*) => {
        $($rest)*
    }
}

macro_rules! impl_exact_size_for_tuples {
    ($( ($($tt:tt: $t:ident),+) ),*$(,)?) => {$(
        impl<$($t: ExactSize),+> ExactSize for ($($t),*,) {
            fn exact_size(&self) -> Size {
                strip_plus!($(+ ExactSize::exact_size(&self.$tt))+)
            }
        }
    )*};
}

impl_exact_size_for_tuples! {
    (0: T0),
    (0: T0, 1: T1),
    (0: T0, 1: T1, 2: T2),
    (0: T0, 1: T1, 2: T2, 3: T3),
    (0: T0, 1: T1, 2: T2, 3: T3, 4: T4),
    (0: T0, 1: T1, 2: T2, 3: T3, 4: T4, 5: T5),
}

/// # Safety
/// The following must be true:
/// - (offset + bit_count + 7) / 8 <= data.len()
unsafe fn bits_memscan(data: &[u8], mut offset: u16, bit_count: u16, cmp_to: bool) -> u16 {
    #[inline]
    fn is_aligned_to_u64(ptr: *const u8) -> bool {
        // SAFETY: Pointer-to-integer transmutes are valid (if you are okay with losing the
        // provenance).
        let addr = ptr.cast::<()>() as usize;
        addr & (std::mem::align_of::<u64>() - 1) == 0
    }

    unsafe {
        if bit_count == 0 {
            return 0;
        }
        debug_assert!((offset + bit_count + 7) as usize / 8 <= data.len());

        let xor_value = cmp_to as u8 * u8::MAX;

        // Apply offset to byte level
        let mut ptr = data.as_ptr().add(offset as usize >> 3);
        offset &= 0b111;

        let mut rem = bit_count;
        if offset > 0 {
            // NOTE: `offset` is in range 1..=7
            let v = (*ptr ^ xor_value) << offset;
            let c = v.leading_zeros() as u16;
            let l = 8 - offset;
            if c < l || bit_count <= l {
                return c.min(bit_count);
            }

            ptr = ptr.add(1);
            rem -= l;
        }

        while rem >= 8 && !is_aligned_to_u64(ptr) {
            let v = *ptr ^ xor_value;
            if v > 0 {
                return bit_count - rem + v.leading_zeros() as u16;
            }

            ptr = ptr.add(1);
            rem -= 8;
        }

        let xor_value_l = cmp_to as u64 * u64::MAX;
        while rem >= 64 {
            #[cfg(target_endian = "little")]
            let z = { (*ptr.cast::<u64>()).swap_bytes() ^ xor_value_l };
            #[cfg(not(target_endian = "little"))]
            let z = { *ptr.cast::<u64>() ^ xor_value_l };

            if z > 0 {
                return bit_count - rem + z.leading_zeros() as u16;
            }

            ptr = ptr.add(8);
            rem -= 64;
        }

        while rem >= 8 {
            let v = *ptr ^ xor_value;
            if v > 0 {
                return bit_count - rem + v.leading_zeros() as u16;
            }

            ptr = ptr.add(1);
            rem -= 8;
        }

        if rem > 0 {
            let v = *ptr ^ xor_value;
            let c = v.leading_zeros() as u16;
            if c < rem {
                return bit_count - rem + c;
            }
        }

        bit_count
    }
}

// # Safety
/// The following must be true:
/// - (offset + bit_count + 7) / 8 <= data.len()
unsafe fn bits_memscan_rev(data: &[u8], mut offset: u16, mut bit_count: u16, cmp_to: bool) -> u16 {
    unsafe {
        if bit_count == 0 {
            return 0;
        }
        debug_assert!((offset + bit_count + 7) as usize / 8 <= data.len());

        let xor_value = cmp_to as u8 * u8::MAX;
        let mut ptr = data.as_ptr().add((offset + bit_count) as usize >> 3);
        offset = (offset + bit_count) & 0b111;

        let mut res = offset;
        if offset > 0 {
            let v = (*ptr >> (8 - offset)) ^ xor_value;
            let c = v.trailing_zeros() as u16;
            if c < offset || res >= bit_count {
                return c.min(bit_count);
            }
            bit_count -= res;
        }

        let xor_value_l = cmp_to as u32 * u32::MAX;
        while bit_count >= 32 {
            ptr = ptr.sub(4);

            #[cfg(target_endian = "little")]
            let v = { ptr.cast::<u32>().read_unaligned().swap_bytes() ^ xor_value_l };
            #[cfg(not(target_endian = "little"))]
            let v = { ptr.cast::<u32>().read_unaligned() ^ xor_value_l };

            if v > 0 {
                return res + v.trailing_zeros() as u16;
            }

            res += 32;
            bit_count -= 32;
        }

        while bit_count >= 8 {
            ptr = ptr.sub(1);
            let v = *ptr ^ xor_value;
            if v > 0 {
                return res + v.trailing_zeros() as u16;
            }

            res += 8;
            bit_count -= 8;
        }

        if bit_count > 0 {
            ptr = ptr.sub(1);
            let v = *ptr ^ xor_value;
            res + std::cmp::min(v.trailing_zeros() as u16, bit_count)
        } else {
            res
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::error::Error;
    use crate::prelude::*;

    fn build_cell<F: FnOnce(&mut CellBuilder) -> Result<(), Error>>(f: F) -> Cell {
        let mut builder = CellBuilder::new();
        f(&mut builder).unwrap();
        builder.build().unwrap()
    }

    fn print_slice(name: &str, slice: CellSlice) {
        println!(
            "{name}: {}",
            build_cell(|b| b.store_slice(slice)).display_tree()
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)] // takes too long to execute on miri
    fn get_raw() -> anyhow::Result<()> {
        let cell = CellBuilder::from_raw_data(&[0xff; 128], 200).and_then(CellBuilder::build)?;
        let slice = cell.as_slice()?;

        let mut data = [0; 1];
        assert!(slice.get_raw(0, &mut data, 100).is_err());

        let mut data = [0; 64];
        assert!(slice.get_raw(0, &mut data, 500).is_err());

        let cell = CellBuilder::from_raw_data(&[0xff; 128], 1023).and_then(CellBuilder::build)?;
        let slice = cell.as_slice()?;

        let mut data = [0; 128];
        for offset in 0..=8 {
            for bits in 0..=(1023 - offset) {
                slice.get_raw(offset, &mut data, bits)?;
            }
        }

        Ok(())
    }

    #[test]
    fn strip_data_prefix() -> anyhow::Result<()> {
        let cell1 = build_cell(|b| {
            b.store_u16(0xabcd)?;
            b.store_bit_zero()?;
            b.store_u16(0xffff)
        });
        let mut slice1 = cell1.as_slice()?;
        slice1.skip_first(4, 0)?;

        let cell2 = build_cell(|b| {
            b.store_uint(0xbcd, 12)?;
            b.store_bit_zero()
        });

        print_slice("A", slice1);
        print_slice("B", cell2.as_slice()?);
        print_slice("LCP", slice1.longest_common_data_prefix(&cell2.as_slice()?));

        let mut without_prefix = slice1.strip_data_prefix(&cell2.as_slice()?).unwrap();
        print_slice("Result", without_prefix);

        assert_eq!(without_prefix.load_u16(), Ok(0xffff));
        assert!(without_prefix.is_data_empty());

        Ok(())
    }

    #[test]
    fn longest_common_data_prefix() -> anyhow::Result<()> {
        let cell1 = build_cell(|b| b.store_u64(0xffffffff00000000));
        let mut slice1 = cell1.as_slice()?;
        slice1.skip_first(1, 0)?;

        let cell2 = build_cell(|b| b.store_u64(0xfffffff000000000));
        let mut slice2 = cell2.as_slice()?;
        slice2.skip_first(6, 0)?;

        let prefix = slice1.longest_common_data_prefix(&slice2);

        let prefix = build_cell(|b| b.store_slice(prefix));
        println!("{}", prefix.display_root());
        assert_eq!(prefix.data(), [0xff, 0xff, 0xfe]);
        assert_eq!(prefix.bit_len(), 22);

        //
        let cell1 = build_cell(|b| b.store_u32(0));
        let cell2 = build_cell(|b| b.store_u32(1));
        let prefix = cell1
            .as_slice()?
            .longest_common_data_prefix(&cell2.as_slice()?);
        assert_eq!(prefix.size_bits(), 31);

        //
        let cell1 = build_cell(|b| b.store_raw(&[0, 0, 2, 2], 32));
        let mut slice1 = cell1.as_slice()?;
        slice1.skip_first(23, 0)?;

        let cell2 = build_cell(|b| b.store_raw(&[0; 128], 1023));
        let slice2 = cell2.as_slice()?.get_prefix(8, 0);

        let prefix = slice1.longest_common_data_prefix(&slice2);
        assert_eq!(prefix.size_bits(), 7);

        //
        let cell1 = build_cell(|b| b.store_u16(0));
        let mut slice1 = cell1.as_slice()?;
        slice1.skip_first(5, 0)?;

        let cell2 = build_cell(|b| b.store_u8(0));
        let mut slice2 = cell2.as_slice()?;
        slice2.skip_first(2, 0)?;

        let prefix = slice1
            .get_prefix(5, 0)
            .longest_common_data_prefix(&slice2.get_prefix(5, 0));
        assert_eq!(prefix.size_bits(), 5);

        Ok(())
    }

    #[test]
    fn unaligned_longest_common_data_prefix() -> anyhow::Result<()> {
        let raw_key =
            Boc::decode_base64("te6ccgEBAQEAJAAAQ0gBfkBkwI7RPE0/VFMj7yvbvFrpvOMEPDQHDlGWFIELM9s=")?;

        let unaligned = {
            let mut raw_key = raw_key.as_slice()?;
            raw_key.skip_first(4, 0)?;
            raw_key.get_prefix(267, 0)
        };
        let aligned = CellBuilder::build_from(unaligned)?;
        let aligned = aligned.as_slice()?;

        assert_eq!(unaligned.lex_cmp(&aligned)?, std::cmp::Ordering::Equal);

        let prefix = Boc::decode_base64(
            "te6ccgEBAwEAjgACB4HQAsACAQCBvwFNima9rQU2tAaEHK+4fSc/aaYcTkT20uyfZuGbZjVMAAAAAAAAAAAAAAAAAAAABAAAAAAAAAAAAAAAAAAAAAIAgb8RUsb7kteM/ARjNwkzPPYoytZRb4Ic9epNxxLMl/2h7AAAAAAAAAAAAAAAAAAAAADMzMzMzMzMzMzMzMzMzMzO",
        )?;
        let prefix = {
            let mut prefix = prefix.as_slice()?;
            prefix.skip_first(11, 0)?;
            prefix.get_prefix(14, 0)
        };

        let lcp_unaligned = unaligned.longest_common_data_prefix(&prefix);
        let lcp_aligned = aligned.longest_common_data_prefix(&prefix);

        assert_eq!(
            lcp_unaligned.lex_cmp(&lcp_aligned)?,
            std::cmp::Ordering::Equal
        );

        Ok(())
    }

    #[test]
    fn get_uint() -> anyhow::Result<()> {
        let cell = build_cell(|b| b.store_u64(0xfafafafafafafafa));

        let slice = cell.as_slice()?;
        assert_eq!(slice.get_uint(0, 3), Ok(0b111));
        assert_eq!(slice.get_uint(0, 11), Ok(0b11111010111));
        assert_eq!(slice.get_uint(1, 11), Ok(0b11110101111));
        assert_eq!(slice.get_uint(8, 3), Ok(0b111));
        assert_eq!(slice.get_uint(0, 16), Ok(0xfafa));

        Ok(())
    }

    #[test]
    fn test_uniform() -> anyhow::Result<()> {
        let cell = build_cell(|b| b.store_zeros(10));
        assert_eq!(cell.as_slice()?.test_uniform(), Some(false));

        let cell = build_cell(|b| b.store_u8(0xff));
        assert_eq!(cell.as_slice()?.test_uniform(), Some(true));

        let cell = build_cell(|b| b.store_u8(123));
        assert_eq!(cell.as_slice()?.test_uniform(), None);

        let cell = build_cell(|b| b.store_u16(123));
        assert_eq!(cell.as_slice()?.test_uniform(), None);

        let cell = build_cell(|b| {
            b.store_zeros(9)?;
            b.store_bit_one()
        });
        assert_eq!(cell.as_slice()?.test_uniform(), None);

        let cell = build_cell(|b| {
            b.store_zeros(20)?;
            b.store_bit_one()
        });
        assert_eq!(cell.as_slice()?.test_uniform(), None);

        let cell = build_cell(|b| {
            b.store_bit_zero()?;
            b.store_uint(u64::MAX, 29)
        });
        let mut slice = cell.as_slice()?;
        slice.skip_first(1, 0)?;
        assert_eq!(slice.test_uniform(), Some(true));

        Ok(())
    }

    #[test]
    fn compare_by_content() -> anyhow::Result<()> {
        fn cmp<L, R>(l: L, r: R) -> Result<std::cmp::Ordering, Error>
        where
            L: FnOnce(&mut CellBuilder) -> Result<(), Error>,
            R: FnOnce(&mut CellBuilder) -> Result<(), Error>,
        {
            let cell1 = build_cell(l);
            let cell2 = build_cell(r);
            cell1.as_slice()?.lex_cmp(&cell2.as_slice()?)
        }

        assert_eq!(
            cmp(
                |b| b.store_u64(0xffffffff0000000f),
                |b| b.store_u64(0xffffffff00000000)
            )?,
            std::cmp::Ordering::Greater
        );

        assert_eq!(
            cmp(
                |b| b.store_u64(0xfffffff00000000),
                |b| b.store_u64(0xffffffff00000000)
            )?,
            std::cmp::Ordering::Less
        );

        assert_eq!(
            cmp(
                |b| b.store_u64(0xffffffff00000000),
                |b| b.store_u64(0xffffffff00000000)
            )?,
            std::cmp::Ordering::Equal
        );

        assert_eq!(
            cmp(
                |b| b.store_uint(0xffffffff00000000, 60),
                |b| b.store_u64(0xffffffff00000000)
            )?,
            std::cmp::Ordering::Less
        );

        Ok(())
    }

    #[test]
    fn is_prefix_of() -> anyhow::Result<()> {
        let left = build_cell(|b| b.store_u64(0xabcdef1234567890));
        let right = build_cell(|b| b.store_u64(0xabcdef0000000000));

        // Not a prefix
        {
            let left = left.as_slice()?;
            let right = right.as_slice()?;
            assert!(!right.is_data_prefix_of(&left).unwrap());
        }

        // Aligned prefix
        {
            let left = left.as_slice()?;
            let mut right = right.as_slice()?;
            right.only_first(24, 0)?;
            assert!(right.is_data_prefix_of(&left).unwrap());
        }

        // Shifted prefix
        {
            let mut left = left.as_slice()?;
            left.skip_first(3, 0)?;

            let mut right = right.as_slice()?;
            right.skip_first(3, 0)?;
            right.only_first(21, 0)?;

            assert!(right.is_data_prefix_of(&left).unwrap());
        }

        // Empty prefix
        {
            let left = left.as_slice()?;
            let right = Cell::empty_cell_ref().as_slice()?;
            assert!(right.is_data_prefix_of(&left).unwrap());
        }

        // Not as prefix of an empty prefix
        {
            let left = Cell::empty_cell_ref().as_slice()?;
            let right = right.as_slice()?;
            assert!(!right.is_data_prefix_of(&left).unwrap());
        }

        Ok(())
    }

    #[test]
    fn is_suffix_of() -> anyhow::Result<()> {
        let left = build_cell(|b| b.store_u64(0xabcdef1234567890));
        let right = build_cell(|b| b.store_u64(0x0000001234567890));

        // Not a suffix
        {
            let left = left.as_slice()?;
            let right = right.as_slice()?;
            assert!(!right.is_data_suffix_of(&left).unwrap());
        }

        // Aligned suffix
        {
            let left = left.as_slice()?;
            let mut right = right.as_slice()?;
            right.only_last(40, 0)?;
            assert!(right.is_data_suffix_of(&left).unwrap());
        }

        // Shifted suffix
        {
            let mut left = left.as_slice()?;
            left.skip_last(3, 0)?;

            let mut right = right.as_slice()?;
            right.skip_last(3, 0)?;
            right.only_last(37, 0)?;

            assert!(right.is_data_suffix_of(&left).unwrap());
        }

        // Empty suffix
        {
            let left = left.as_slice()?;
            let right = Cell::empty_cell_ref().as_slice()?;
            assert!(right.is_data_suffix_of(&left).unwrap());
        }

        // Not as suffix of an empty suffix
        {
            let left = Cell::empty_cell_ref().as_slice()?;
            let right = right.as_slice()?;
            assert!(!right.is_data_suffix_of(&left).unwrap());
        }

        Ok(())
    }

    #[test]
    fn leading_bits() -> anyhow::Result<()> {
        // Empty slice has zero leading bits
        assert_eq!(Cell::empty_cell_ref().as_slice()?.count_leading(false)?, 0);
        assert_eq!(Cell::empty_cell_ref().as_slice()?.count_leading(true)?, 0);

        // Full slice has all bits set
        assert_eq!(
            Cell::all_zeros_ref().as_slice()?.count_leading(false)?,
            1023
        );
        assert_eq!(Cell::all_ones_ref().as_slice()?.count_leading(true)?, 1023);

        // Full slice has no leading other bits
        assert_eq!(Cell::all_zeros_ref().as_slice()?.count_leading(true)?, 0);
        assert_eq!(Cell::all_ones_ref().as_slice()?.count_leading(false)?, 0);

        // Test for different alignments
        for shift_before in [false, true] {
            for shift_after in [false, true] {
                for i in 0..128 {
                    let mut builder = CellBuilder::new();

                    if shift_before {
                        builder.store_ones(7)?;
                    };
                    builder.store_u128(1 << i)?;
                    if shift_after {
                        builder.store_ones(14)?;
                    }

                    let mut slice = builder.as_data_slice();

                    if shift_before {
                        slice.skip_first(7, 0)?;
                    }
                    if shift_after {
                        slice.only_first(128, 0)?;
                    }

                    assert_eq!(slice.count_leading(false)?, 127 - i);
                    assert_eq!(slice.count_leading(true)?, (i == 127) as u16);
                }
            }
        }

        Ok(())
    }

    #[test]
    fn trailing_bits() -> anyhow::Result<()> {
        // Empty slice has zero trailing bits
        assert_eq!(Cell::empty_cell_ref().as_slice()?.count_trailing(false)?, 0);
        assert_eq!(Cell::empty_cell_ref().as_slice()?.count_trailing(true)?, 0);

        // Full slice has all bits set
        assert_eq!(
            Cell::all_zeros_ref().as_slice()?.count_trailing(false)?,
            1023
        );
        assert_eq!(Cell::all_ones_ref().as_slice()?.count_trailing(true)?, 1023);

        // Full slice has no trailing other bits
        assert_eq!(Cell::all_zeros_ref().as_slice()?.count_trailing(true)?, 0);
        assert_eq!(Cell::all_ones_ref().as_slice()?.count_trailing(false)?, 0);

        // Test for different alignments
        for shift_before in [false, true] {
            for shift_after in [false, true] {
                for i in 0..128 {
                    let mut builder = CellBuilder::new();

                    if shift_before {
                        builder.store_ones(7)?;
                    };
                    builder.store_u128(1 << i)?;
                    if shift_after {
                        builder.store_ones(14)?;
                    }

                    let mut slice = builder.as_data_slice();

                    if shift_before {
                        slice.skip_first(7, 0)?;
                    }
                    if shift_after {
                        slice.only_first(128, 0)?;
                    }

                    assert_eq!(slice.count_trailing(false)?, i);
                    assert_eq!(slice.count_trailing(true)?, (i == 0) as u16);
                }
            }
        }

        Ok(())
    }

    #[test]
    fn split_slice() -> anyhow::Result<()> {
        let cell = CellBuilder::build_from((0xdeafbeafu32, 0xabbacafeu32))?;

        // Prefix
        {
            let mut cs = cell.as_slice()?;
            assert!(cs.load_prefix(0, 1).is_err());

            let mut prefix = cs.load_prefix(16, 0)?;
            assert_eq!(prefix.size_bits(), 16);
            assert_eq!(cs.size_bits(), 64 - 16);
            assert_eq!(prefix.load_u16()?, 0xdeaf);
            assert_eq!(cs.get_u16(0)?, 0xbeaf);

            let mut prefix = cs.load_prefix(32, 0)?;
            assert_eq!(prefix.size_bits(), 32);
            assert_eq!(cs.size_bits(), 64 - 16 - 32);
            assert_eq!(prefix.load_u32()?, 0xbeafabba);
            assert_eq!(cs.get_u16(0)?, 0xcafe);

            let mut prefix = cs.load_prefix(16, 0)?;
            assert_eq!(prefix.size_bits(), 16);
            assert_eq!(cs.size_bits(), 0);
            assert_eq!(prefix.load_u16()?, 0xcafe);

            assert!(cs.load_prefix(10, 0).is_err());
        }

        // Suffix
        {
            let mut cs = cell.as_slice()?;
            assert!(cs.load_suffix(0, 1).is_err());

            let mut suffix = cs.load_suffix(16, 0)?;
            assert_eq!(suffix.size_bits(), 16);
            assert_eq!(cs.size_bits(), 64 - 16);
            assert_eq!(suffix.load_u16()?, 0xcafe);
            assert_eq!(cs.get_u16(32)?, 0xabba);

            let mut suffix = cs.load_suffix(32, 0)?;
            assert_eq!(suffix.size_bits(), 32);
            assert_eq!(cs.size_bits(), 64 - 16 - 32);
            assert_eq!(suffix.load_u32()?, 0xbeafabba);
            assert_eq!(cs.get_u16(0)?, 0xdeaf);

            let mut suffix = cs.load_suffix(16, 0)?;
            assert_eq!(suffix.size_bits(), 16);
            assert_eq!(cs.size_bits(), 0);
            assert_eq!(suffix.load_u16()?, 0xdeaf);

            assert!(cs.load_suffix(10, 0).is_err());
        }

        Ok(())
    }
}
