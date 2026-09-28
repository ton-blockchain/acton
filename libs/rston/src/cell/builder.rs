use std::cmp::Ordering;
use std::mem::MaybeUninit;
use std::rc::Rc;
use std::sync::Arc;

use super::CellFamily;
use crate::cell::cell_context::{CellContext, CellParts};
use crate::cell::{
    Cell, CellDescriptor, CellImpl, CellInner, CellSlice, CellType, DynCell, HashBytes, LevelMask,
    MAX_BIT_LEN, MAX_REF_COUNT, Size,
};
use crate::error::Error;
use crate::util::{ArrayVec, ArrayVecIntoIter, Bitstring};

/// A data structure that can be serialized into cells.
pub trait Store {
    /// Tries to store itself into the cell builder.
    fn store_into(&self, builder: &mut CellBuilder, context: &dyn CellContext)
    -> Result<(), Error>;
}

impl<T: Store + ?Sized> Store for &T {
    #[inline]
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        <T as Store>::store_into(self, builder, context)
    }
}

impl<T: Store + ?Sized> Store for &mut T {
    #[inline]
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        <T as Store>::store_into(self, builder, context)
    }
}

impl<T: Store + ?Sized> Store for Box<T> {
    #[inline]
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        <T as Store>::store_into(self.as_ref(), builder, context)
    }
}

impl<T: Store + ?Sized> Store for Arc<T> {
    #[inline]
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        <T as Store>::store_into(self.as_ref(), builder, context)
    }
}

impl<T: Store + ?Sized> Store for Rc<T> {
    #[inline]
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        <T as Store>::store_into(self.as_ref(), builder, context)
    }
}

impl Store for () {
    #[inline]
    fn store_into(&self, _: &mut CellBuilder, _: &dyn CellContext) -> Result<(), Error> {
        Ok(())
    }
}

macro_rules! impl_store_for_tuples {
    ($( ($($field:ident: $t:ident),+) ),*$(,)?) => {$(
        impl<$($t: Store),+> Store for ($($t),*) {
            fn store_into(
                &self,
                builder: &mut CellBuilder,
                context: &dyn CellContext
            ) -> Result<(), Error> {
                let ($($field),+) = self;
                $(ok!($field.store_into(builder, context)));*;
                Ok(())
            }
        }
    )*};
}

impl_store_for_tuples! {
    (t1: T1, t2: T2),
    (t1: T1, t2: T2, t3: T3),
    (t1: T1, t2: T2, t3: T3, t4: T4),
    (t1: T1, t2: T2, t3: T3, t4: T4, t5: T5),
    (t1: T1, t2: T2, t3: T3, t4: T4, t5: T5, t6: T6),
}

impl<T: Store> Store for Option<T> {
    #[inline]
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        match self {
            Some(data) => {
                ok!(builder.store_bit_one());
                data.store_into(builder, context)
            }
            None => builder.store_bit_zero(),
        }
    }
}

impl Store for CellBuilder {
    #[inline]
    fn store_into(&self, builder: &mut CellBuilder, _: &dyn CellContext) -> Result<(), Error> {
        builder.store_builder(self)
    }
}

impl Store for CellSlice<'_> {
    #[inline]
    fn store_into(&self, builder: &mut CellBuilder, _: &dyn CellContext) -> Result<(), Error> {
        builder.store_slice(self)
    }
}

impl Store for Cell {
    #[inline]
    fn store_into(&self, builder: &mut CellBuilder, _: &dyn CellContext) -> Result<(), Error> {
        builder.store_reference(self.clone())
    }
}

macro_rules! impl_primitive_store {
    ($($type:ty => |$b:ident, $v:ident| $expr:expr),*$(,)?) => {
        $(impl Store for $type {
            #[inline]
            fn store_into(&self,
                $b: &mut CellBuilder,
                _: &dyn CellContext
            ) -> Result<(), Error> {
                let $v = self;
                $expr
            }
        })*
    };
}

impl_primitive_store! {
    bool => |b, v| b.store_bit(*v),
    u8 => |b, v| b.store_u8(*v),
    i8 => |b, v| b.store_u8(*v as u8),
    u16 => |b, v| b.store_u16(*v),
    i16 => |b, v| b.store_u16(*v as u16),
    u32 => |b, v| b.store_u32(*v),
    i32 => |b, v| b.store_u32(*v as u32),
    u64 => |b, v| b.store_u64(*v),
    i64 => |b, v| b.store_u64(*v as u64),
    u128 => |b, v| b.store_u128(*v),
    i128 => |b, v| b.store_u128(*v as u128),
    HashBytes => |b, v| b.store_u256(v),
}

/// Builder for constructing cells with densely packed data.
#[derive(Default, Clone)]
pub struct CellBuilder {
    inner: CellDataBuilder,
    is_exotic: bool,
    references: ArrayVec<Cell, MAX_REF_COUNT>,
}

impl Eq for CellBuilder {}

impl PartialEq for CellBuilder {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.is_exotic == other.is_exotic
            && self.inner == other.inner
            && self.references.as_ref() == other.references.as_ref()
    }
}

impl Ord for CellBuilder {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.inner.bit_len, self.references.len())
            .cmp(&(other.inner.bit_len, other.references.len()))
        {
            Ordering::Equal => {}
            ord => return ord,
        }

        // TODO: compare subslices of len {(bits + 7) / 8} ?
        match self.inner.data.cmp(&other.inner.data) {
            Ordering::Equal => {}
            ord => return ord,
        }

        for (a, b) in self.references().iter().zip(other.references().iter()) {
            match a.repr_hash().cmp(b.repr_hash()) {
                Ordering::Equal => {}
                ord => return ord,
            }
        }

        Ordering::Equal
    }
}

impl PartialOrd for CellBuilder {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl std::fmt::Debug for CellBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[repr(transparent)]
        struct Data<'a, T>(&'a T);

        impl<T: std::fmt::Display> std::fmt::Debug for Data<'_, T> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Display::fmt(self.0, f)
            }
        }

        f.debug_struct("CellBuilder")
            .field("data", &Data(&self.inner.display_data()))
            .field("bit_len", &self.inner.bit_len)
            .field("is_exotic", &self.is_exotic)
            .field("references", &self.references.as_ref())
            .finish()
    }
}

impl std::ops::Deref for CellBuilder {
    type Target = CellDataBuilder;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl std::ops::DerefMut for CellBuilder {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl AsRef<CellDataBuilder> for CellBuilder {
    #[inline]
    fn as_ref(&self) -> &CellDataBuilder {
        &self.inner
    }
}

impl AsMut<CellDataBuilder> for CellBuilder {
    #[inline]
    fn as_mut(&mut self) -> &mut CellDataBuilder {
        &mut self.inner
    }
}

impl From<CellDataBuilder> for CellBuilder {
    #[inline]
    fn from(inner: CellDataBuilder) -> Self {
        Self {
            inner,
            is_exotic: false,
            references: ArrayVec::new(),
        }
    }
}

impl CellBuilder {
    /// Builds a new cell from the specified data using the default cell context.
    #[inline]
    pub fn build_from<T>(data: T) -> Result<Cell, Error>
    where
        T: Store,
    {
        Self::build_from_ext(data, Cell::empty_context())
    }

    /// Builds a new cell from the specified data using the provided cell context.
    #[inline]
    pub fn build_from_ext<T>(data: T, context: &dyn CellContext) -> Result<Cell, Error>
    where
        T: Store,
    {
        fn build_from_ext_impl(data: &dyn Store, context: &dyn CellContext) -> Result<Cell, Error> {
            let mut builder = CellBuilder::new();
            ok!(data.store_into(&mut builder, context));
            builder.build_ext(context)
        }
        build_from_ext_impl(&data, context)
    }

    /// Builds a new library cell referencing the specified library.
    ///
    /// Uses the default cell context.
    #[inline]
    pub fn build_library<T: AsRef<[u8; 32]>>(hash: &T) -> Cell {
        Self::build_library_ext(hash, Cell::empty_context())
    }

    /// Builds a new library cell referencing the specified library.
    ///
    /// Uses the specified cell context.
    #[inline]
    pub fn build_library_ext<T: AsRef<[u8; 32]>>(hash: &T, context: &dyn CellContext) -> Cell {
        fn build_library_ext_impl(
            hash: &[u8; 32],
            context: &dyn CellContext,
        ) -> Result<Cell, Error> {
            let mut b = CellBuilder::new();
            b.set_exotic(true);
            ok!(b.store_u8(CellType::LibraryReference.to_byte()));
            ok!(b.store_u256(HashBytes::wrap(hash)));
            b.build_ext(context)
        }
        build_library_ext_impl(hash.as_ref(), context).unwrap()
    }

    /// Creates an empty cell builder.
    pub const fn new() -> Self {
        Self {
            inner: CellDataBuilder::new(),
            is_exotic: false,
            references: ArrayVec::new(),
        }
    }

    /// Tries to create a cell builder with the specified data.
    ///
    /// NOTE: if `bits` is greater than `bytes * 8`, pads the value with zeros (as high bits).
    pub fn from_raw_data(value: &[u8], bits: u16) -> Result<Self, Error> {
        Ok(Self {
            inner: ok!(CellDataBuilder::from_raw_data(value, bits)),
            is_exotic: false,
            references: ArrayVec::new(),
        })
    }

    /// Creates a cell builder from its inner parts.
    #[inline]
    pub fn from_parts(is_exotic: bool, data: CellDataBuilder, refs: CellRefsBuilder) -> Self {
        Self {
            inner: data,
            is_exotic,
            references: refs.0,
        }
    }

    /// Splits cell builder into its inner parts.
    #[inline]
    pub fn into_parts(self) -> (bool, CellDataBuilder, CellRefsBuilder) {
        (self.is_exotic, self.inner, CellRefsBuilder(self.references))
    }

    /// Returns a slice which contains builder data and references.
    ///
    /// NOTE: intermediate cell hash is undefined.
    pub fn as_full_slice(&self) -> CellSlice<'_> {
        CellSlice::new_allow_exotic(IntermediateFullCell::wrap(self))
    }

    /// Returns the stored cell size.
    pub const fn size(&self) -> Size {
        Size {
            bits: self.inner.bit_len,
            refs: self.references.len() as u8,
        }
    }

    /// Returns child cell count.
    #[inline(always)]
    pub const fn size_refs(&self) -> u8 {
        self.references.len() as u8
    }

    /// Returns the remaining capacity in bits and references.
    pub const fn spare_capacity(&self) -> Size {
        Size {
            bits: self.inner.spare_capacity_bits(),
            refs: self.spare_capacity_refs(),
        }
    }

    /// Returns remaining references capacity.
    #[inline]
    pub const fn spare_capacity_refs(&self) -> u8 {
        (MAX_REF_COUNT - self.references.len()) as u8
    }

    /// Returns true if there is enough remaining capacity to fit `bits` and `refs`.
    #[inline]
    pub const fn has_capacity(&self, bits: u16, refs: u8) -> bool {
        self.inner.bit_len + bits <= MAX_BIT_LEN
            && self.references.len() + refs as usize <= MAX_REF_COUNT
    }

    /// Returns whether this cell will be built as an exotic.
    #[inline]
    pub const fn is_exotic(&self) -> bool {
        self.is_exotic
    }

    /// Marks this cell as exotic.
    #[inline]
    pub fn set_exotic(&mut self, is_exotic: bool) {
        self.is_exotic = is_exotic;
    }

    /// Returns a slice of the child cells stored in the builder.
    #[inline]
    pub fn references(&self) -> &[Cell] {
        self.references.as_ref()
    }

    /// Tries to store a child in the cell,
    /// returning `false` if there is not enough remaining capacity.
    pub fn store_reference(&mut self, cell: Cell) -> Result<(), Error> {
        if self.references.len() < MAX_REF_COUNT {
            // SAFETY: reference count is in the valid range
            unsafe { self.references.push(cell) }
            Ok(())
        } else {
            Err(Error::CellOverflow)
        }
    }

    /// Sets children of the cell.
    pub fn set_references(&mut self, refs: CellRefsBuilder) {
        self.references = refs.0;
    }

    /// Tries to append a builder (its data and references),
    /// returning `false` if there is not enough remaining capacity.
    pub fn store_builder(&mut self, builder: &Self) -> Result<(), Error> {
        if self.inner.bit_len + builder.inner.bit_len <= MAX_BIT_LEN
            && self.references.len() + builder.references.len() <= MAX_REF_COUNT
        {
            ok!(self.store_raw(&builder.inner.data, builder.inner.bit_len));
            for cell in builder.references.as_ref() {
                ok!(self.store_reference(cell.clone()));
            }
            Ok(())
        } else {
            Err(Error::CellOverflow)
        }
    }

    /// Tries to append a cell slice (its data and references),
    /// returning `false` if there is not enough remaining capacity.
    #[inline]
    pub fn store_slice<'a, T>(&mut self, value: T) -> Result<(), Error>
    where
        T: AsRef<CellSlice<'a>>,
    {
        fn store_slice_impl(builder: &mut CellBuilder, value: &CellSlice<'_>) -> Result<(), Error> {
            if builder.inner.bit_len + value.size_bits() <= MAX_BIT_LEN
                && builder.references.len() + value.size_refs() as usize <= MAX_REF_COUNT
            {
                ok!(builder.store_slice_data(value));
                for cell in value.references().cloned() {
                    ok!(builder.store_reference(cell));
                }
                Ok(())
            } else {
                Err(Error::CellOverflow)
            }
        }
        store_slice_impl(self, value.as_ref())
    }

    /// Tries to build a new cell using the specified cell context.
    pub fn build_ext(mut self, context: &dyn CellContext) -> Result<Cell, Error> {
        debug_assert!(self.inner.bit_len <= MAX_BIT_LEN);
        debug_assert!(self.references.len() <= MAX_REF_COUNT);

        let mut children_mask = LevelMask::EMPTY;
        for child in self.references.as_ref() {
            let child = child.as_ref();
            children_mask |= child.descriptor().level_mask();
        }

        let is_exotic = self.is_exotic;

        let level_mask = 'mask: {
            // NOTE: make only a brief check here, as it will raise a proper error in finalizer
            if is_exotic
                && self.inner.bit_len >= 8
                && let Some(ty) = CellType::from_byte_exotic(self.data[0])
            {
                match ty {
                    CellType::PrunedBranch => break 'mask LevelMask::new(self.data[1]),
                    CellType::MerkleProof | CellType::MerkleUpdate => {
                        break 'mask children_mask.virtualize(1);
                    }
                    CellType::LibraryReference => break 'mask LevelMask::EMPTY,
                    _ => {}
                };
            }

            children_mask
        };

        let d1 = CellDescriptor::compute_d1(level_mask, is_exotic, self.references.len() as u8);
        let d2 = CellDescriptor::compute_d2(self.inner.bit_len);

        let rem = self.inner.bit_len % 8;
        let last_byte = (self.inner.bit_len / 8) as usize;
        if rem > 0 {
            // SAFETY: `last_byte` is in the valid range
            let last_byte = unsafe { self.inner.data.get_unchecked_mut(last_byte) };

            // x0000000 - rem=1, tag_mask=01000000, data_mask=11000000
            // xx000000 - rem=2, tag_mask=00100000, data_mask=11100000
            // xxx00000 - rem=3, tag_mask=00010000, data_mask=11110000
            // xxxx0000 - rem=4, tag_mask=00001000, data_mask=11111000
            // xxxxx000 - rem=5, tag_mask=00000100, data_mask=11111100
            // xxxxxx00 - rem=6, tag_mask=00000010, data_mask=11111110
            // xxxxxxx0 - rem=7, tag_mask=00000001, data_mask=11111111
            let tag_mask: u8 = 1 << (7 - rem);
            let data_mask = !(tag_mask - 1);

            // xxxxyyyy & data_mask -> xxxxy000 | tag_mask -> xxxx1000
            *last_byte = (*last_byte & data_mask) | tag_mask;
        }

        let byte_len = self.inner.bit_len.div_ceil(8);
        let data = &self.inner.data[..std::cmp::min(byte_len as usize, 128)];

        let cell_parts = CellParts {
            bit_len: self.inner.bit_len,
            descriptor: CellDescriptor { d1, d2 },
            children_mask,
            references: self.references,
            data,
        };
        context.finalize_cell(cell_parts)
    }

    /// Tries to build a new cell using the default cell context.
    ///
    /// See [`empty_context`]
    ///
    /// [`empty_context`]: fn@CellFamily::empty_context
    pub fn build(self) -> Result<Cell, Error> {
        self.build_ext(Cell::empty_context())
    }
}

/// Helper struct to print the cell builder data.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct DisplayCellBuilderData<'a>(&'a CellDataBuilder);

impl std::fmt::Display for DisplayCellBuilderData<'_> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::LowerHex::fmt(self, f)
    }
}

impl std::fmt::LowerHex for DisplayCellBuilderData<'_> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::LowerHex::fmt(&self.0.as_bitstring(), f)
    }
}

impl std::fmt::UpperHex for DisplayCellBuilderData<'_> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::UpperHex::fmt(&self.0.as_bitstring(), f)
    }
}

impl std::fmt::Binary for DisplayCellBuilderData<'_> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Binary::fmt(&self.0.as_bitstring(), f)
    }
}

/// Generates a fully random builder with any type of child cells.
#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for CellBuilder {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        match u.arbitrary::<CellType>()? {
            CellType::Ordinary => {
                let bit_len = u.int_in_range(0..=MAX_BIT_LEN)?;
                let refs = u.int_in_range(0..=4)?;

                let mut b = CellBuilder::new();
                b.store_raw(u.bytes(bit_len.div_ceil(8) as _)?, bit_len)
                    .unwrap();

                let mut children = ArrayVec::<Cell, 4>::new();
                for i in 0..refs as u8 {
                    let cell = 'cell: {
                        if i > 0 {
                            // Allow to reuse cells.
                            if let Some(i) = u.int_in_range(0..=i)?.checked_sub(1) {
                                break 'cell children.get(i).cloned().unwrap();
                            }
                        }

                        u.arbitrary::<Cell>()
                            .and_then(crate::arbitrary::check_max_depth)?
                    };

                    b.store_reference(cell.clone()).unwrap();

                    // SAFETY: `refs` is at most 4.
                    unsafe { children.push(cell) };
                }

                Ok(b)
            }
            CellType::PrunedBranch => {
                let level_mask = LevelMask::new(u.int_in_range(0b001..=0b111)?);

                let mut b = CellBuilder::new();
                b.set_exotic(true);
                b.store_u16(u16::from_be_bytes([
                    CellType::PrunedBranch.to_byte(),
                    level_mask.to_byte(),
                ]))
                .unwrap();

                let level_count = level_mask.level() as usize;

                let hashes = 32 * level_count;
                b.store_raw(u.bytes(hashes)?, hashes as u16 * 8).unwrap();

                for _ in 0..level_count {
                    b.store_u16(u.int_in_range(0..=(u16::MAX - 1))?).unwrap();
                }

                Ok(b)
            }
            CellType::LibraryReference => {
                let hash = u.bytes(32)?;

                let mut b = CellBuilder::new();
                b.set_exotic(true);
                b.store_u8(CellType::LibraryReference.to_byte()).unwrap();
                b.store_raw(hash, 256).unwrap();
                Ok(b)
            }
            CellType::MerkleProof => {
                let mut b = CellBuilder::new();
                u.arbitrary::<crate::merkle::MerkleProof>()?
                    .store_into(&mut b, Cell::empty_context())
                    .unwrap();
                Ok(b)
            }
            CellType::MerkleUpdate => {
                let mut b = CellBuilder::new();
                u.arbitrary::<crate::merkle::MerkleUpdate>()?
                    .store_into(&mut b, Cell::empty_context())
                    .unwrap();
                Ok(b)
            }
        }
    }

    fn size_hint(_: usize) -> (usize, Option<usize>) {
        (3, None)
    }
}

/// Builder for constructing cell data.
#[derive(Clone)]
pub struct CellDataBuilder {
    data: [u8; 128],
    bit_len: u16,
}

impl Default for CellDataBuilder {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Eq for CellDataBuilder {}
impl PartialEq for CellDataBuilder {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.bit_len == other.bit_len && self.data == other.data
    }
}

impl Ord for CellDataBuilder {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.bit_len.cmp(&other.bit_len) {
            Ordering::Equal => {}
            ord => return ord,
        }

        // TODO: compare subslices of len {(bits + 7) / 8} ?
        self.data.cmp(&other.data)
    }
}

impl PartialOrd for CellDataBuilder {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl std::fmt::Debug for CellDataBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[repr(transparent)]
        struct Data<'a, T>(&'a T);

        impl<T: std::fmt::Display> std::fmt::Debug for Data<'_, T> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Display::fmt(self.0, f)
            }
        }

        f.debug_struct("CellBuilder")
            .field("data", &Data(&self.display_data()))
            .field("bit_len", &self.bit_len)
            .finish()
    }
}

macro_rules! impl_store_uint {
    ($self:ident, $value:ident, bytes: $bytes:literal, bits: $bits:literal) => {
        if $self.bit_len + $bits <= MAX_BIT_LEN {
            let q = ($self.bit_len / 8) as usize;
            let r = $self.bit_len % 8;
            // SAFETY: q is in range 0..=127, r is in range 0..=7
            unsafe {
                let data_ptr = $self.data.as_mut_ptr().add(q);
                debug_assert!(q + $bytes + usize::from(r > 0) <= 128);
                if r == 0 {
                    // Just append data
                    let value = $value.to_be_bytes();
                    std::ptr::copy_nonoverlapping(value.as_ptr(), data_ptr, $bytes);
                } else {
                    // Append high bits to the last byte
                    *data_ptr |= ($value >> ($bits - 8 + r)) as u8;
                    // Make shifted bytes
                    let value: [u8; $bytes] = ($value << (8 - r)).to_be_bytes();
                    // Write shifted bytes
                    std::ptr::copy_nonoverlapping(value.as_ptr(), data_ptr.add(1), $bytes);
                }
            };
            $self.bit_len += $bits;
            Ok(())
        } else {
            Err(Error::CellOverflow)
        }
    };
}

impl CellDataBuilder {
    /// Creates an empty cell data builder.
    pub const fn new() -> Self {
        Self {
            data: [0; 128],
            bit_len: 0,
        }
    }

    /// Tries to create a cell builder with the specified data.
    ///
    /// NOTE: if `bits` is greater than `bytes * 8`, pads the value with zeros (as high bits).
    pub fn from_raw_data(value: &[u8], bits: u16) -> Result<Self, Error> {
        let mut res = Self::new();
        ok!(res.store_raw(value, bits));
        Ok(res)
    }

    /// Returns a slice which contains only builder data bits and no references.
    ///
    /// NOTE: intermediate cell hash is undefined.
    pub fn as_data_slice(&self) -> CellSlice<'_> {
        CellSlice::new_allow_exotic(IntermediateDataCell::wrap(self))
    }

    /// Returns an object which will display data as a bitstring
    /// with a termination bit.
    #[inline]
    pub fn display_data(&self) -> DisplayCellBuilderData<'_> {
        DisplayCellBuilderData(self)
    }

    /// Returns cell data as a [`Bitstring`].
    #[inline]
    pub fn as_bitstring(&self) -> Bitstring<'_> {
        Bitstring {
            bytes: &self.data,
            bit_len: self.bit_len,
        }
    }

    /// Returns an underlying cell data.
    #[inline]
    pub const fn raw_data(&self) -> &[u8; 128] {
        &self.data
    }

    /// Returns the data size of this cell in bits.
    #[inline]
    pub const fn size_bits(&self) -> u16 {
        self.bit_len
    }

    /// Returns remaining data capacity in bits.
    #[inline]
    pub const fn spare_capacity_bits(&self) -> u16 {
        MAX_BIT_LEN - self.bit_len
    }

    /// Returns true if there is enough remaining capacity to fit `bits`.
    #[inline]
    pub const fn has_capacity_bits(&self, bits: u16) -> bool {
        self.bit_len + bits <= MAX_BIT_LEN
    }

    /// Clears all data bits and sets the size in bits to 0.
    pub fn clear_bits(&mut self) {
        self.data = [0; 128];
        self.bit_len = 0;
    }

    /// Removes the specified amount of bits from the end of the data.
    pub fn rewind_bits(&mut self, mut bits: u16) -> Result<(), Error> {
        if bits == 0 {
            return Ok(());
        }
        let Some(new_bit_len) = self.bit_len.checked_sub(bits) else {
            return Err(Error::CellUnderflow);
        };

        let q = (new_bit_len / 8) as usize;
        let r = new_bit_len % 8;

        // SAFETY: q is in range 0..=127, r is in range 0..=7
        unsafe {
            let mut data_ptr = self.data.as_mut_ptr().add(q);

            if r != 0 {
                let shift = 8 - r;
                *data_ptr &= 0xff << shift;
                bits = bits.saturating_sub(shift);
                data_ptr = data_ptr.add(1);
            }

            std::ptr::write_bytes(data_ptr, 0, bits.div_ceil(8) as usize);
        }

        self.bit_len = new_bit_len;
        Ok(())
    }

    /// Tries to store the specified number of zero bits in the cell,
    /// returning `false` if there is not enough remaining capacity.
    pub fn store_zeros(&mut self, bits: u16) -> Result<(), Error> {
        if self.bit_len + bits <= MAX_BIT_LEN {
            self.bit_len += bits;
            Ok(())
        } else {
            Err(Error::CellOverflow)
        }
    }

    /// Tries to store the specified number of set bits in the cell,
    /// returning `false` if there is not enough remaining capacity.
    pub fn store_ones(&mut self, bits: u16) -> Result<(), Error> {
        self.store_raw(crate::cell::cell_impl::ALL_ONES_CELL.data(), bits)
    }

    /// Tries to store one zero bit in the cell,
    /// returning `false` if there is not enough remaining capacity.
    pub fn store_bit_zero(&mut self) -> Result<(), Error> {
        let fits = self.bit_len < MAX_BIT_LEN;
        self.bit_len += fits as u16;
        if fits {
            Ok(())
        } else {
            Err(Error::CellOverflow)
        }
    }

    /// Tries to store one non-zero bit in the cell,
    /// returning `false` if there is not enough remaining capacity.
    pub fn store_bit_one(&mut self) -> Result<(), Error> {
        if self.bit_len < MAX_BIT_LEN {
            let q = (self.bit_len / 8) as usize;
            let r = self.bit_len % 8;
            unsafe { *self.data.get_unchecked_mut(q) |= 1 << (7 - r) };
            self.bit_len += 1;
            Ok(())
        } else {
            Err(Error::CellOverflow)
        }
    }

    /// Tries to store one bit in the cell,
    /// returning `false` if there is not enough remaining capacity.
    pub fn store_bit(&mut self, value: bool) -> Result<(), Error> {
        if value {
            self.store_bit_one()
        } else {
            self.store_bit_zero()
        }
    }

    /// Tries to store `u8` in the cell,
    /// returning `false` if there is not enough remaining capacity.
    pub fn store_u8(&mut self, value: u8) -> Result<(), Error> {
        if self.bit_len + 8 <= MAX_BIT_LEN {
            let q = (self.bit_len / 8) as usize;
            let r = self.bit_len % 8;
            unsafe {
                if r == 0 {
                    debug_assert!(q < 128);
                    // xxxxxxxx
                    *self.data.get_unchecked_mut(q) = value;
                } else {
                    debug_assert!(q + 1 < 128);
                    // yyyxxxxx|xxx00000
                    *self.data.get_unchecked_mut(q) |= value >> r;
                    *self.data.get_unchecked_mut(q + 1) = value << (8 - r);
                }
            };
            self.bit_len += 8;
            Ok(())
        } else {
            Err(Error::CellOverflow)
        }
    }

    /// Tries to store `u16` in the cell,
    /// returning `false` if there is not enough remaining capacity.
    pub fn store_u16(&mut self, value: u16) -> Result<(), Error> {
        impl_store_uint!(self, value, bytes: 2, bits: 16)
    }

    /// Tries to store `u32` in the cell,
    /// returning `false` if there is not enough remaining capacity.
    pub fn store_u32(&mut self, value: u32) -> Result<(), Error> {
        impl_store_uint!(self, value, bytes: 4, bits: 32)
    }

    /// Tries to store `u64` in the cell,
    /// returning `false` if there is not enough remaining capacity.
    pub fn store_u64(&mut self, value: u64) -> Result<(), Error> {
        impl_store_uint!(self, value, bytes: 8, bits: 64)
    }

    /// Tries to store `u128` in the cell,
    /// returning `false` if there is not enough remaining capacity.
    pub fn store_u128(&mut self, value: u128) -> Result<(), Error> {
        impl_store_uint!(self, value, bytes: 16, bits: 128)
    }

    /// Tries to store 32 bytes in the cell,
    /// returning `false` if there is not enough remaining capacity.
    #[inline]
    pub fn store_u256<T>(&mut self, value: &T) -> Result<(), Error>
    where
        T: AsRef<[u8; 32]>,
    {
        fn store_u256_impl(builder: &mut CellDataBuilder, value: &[u8; 32]) -> Result<(), Error> {
            if builder.bit_len + 256 <= MAX_BIT_LEN {
                let q = (builder.bit_len / 8) as usize;
                let r = builder.bit_len % 8;
                unsafe {
                    let data_ptr = builder.data.as_mut_ptr().add(q);
                    debug_assert!(q + 32 + usize::from(r > 0) <= 128);
                    if r == 0 {
                        // Just append data
                        std::ptr::copy_nonoverlapping(value.as_ptr(), data_ptr, 32);
                    } else {
                        // Interpret 32 bytes as two u128
                        let [mut hi, mut lo]: [u128; 2] = std::mem::transmute_copy(value);

                        // Numbers are in big endian order, swap bytes on little endian arch
                        #[cfg(target_endian = "little")]
                        {
                            hi = hi.swap_bytes();
                            lo = lo.swap_bytes();
                        }

                        let shift = 8 - r;

                        // Append high bits to the last byte
                        *data_ptr |= (hi >> (128 - shift)) as u8;
                        // Make shifted bytes
                        let hi: [u8; 16] = ((hi << shift) | (lo >> (128 - shift))).to_be_bytes();
                        let lo: [u8; 16] = (lo << shift).to_be_bytes();
                        // Write shifted bytes
                        std::ptr::copy_nonoverlapping(hi.as_ptr(), data_ptr.add(1), 16);
                        std::ptr::copy_nonoverlapping(lo.as_ptr(), data_ptr.add(17), 16);
                    }
                };
                builder.bit_len += 256;
                Ok(())
            } else {
                Err(Error::CellOverflow)
            }
        }

        store_u256_impl(self, value.as_ref())
    }

    /// Tries to store `u8` in the cell (but only the specified number of bits),
    /// returning `false` if there is not enough remaining capacity.
    ///
    /// NOTE: if `bits` is greater than **8**, pads the value with zeros (as high bits).
    pub fn store_small_uint(&mut self, mut value: u8, mut bits: u16) -> Result<(), Error> {
        if bits == 0 {
            return Ok(());
        }

        if self.bit_len + bits <= MAX_BIT_LEN {
            bits = if let Some(offset) = bits.checked_sub(8) {
                self.bit_len += offset;
                8
            } else {
                bits
            };

            // Ensure that value starts with significant bits
            value <<= 8 - bits;

            let q = (self.bit_len / 8) as usize;
            let r = self.bit_len % 8;
            unsafe {
                debug_assert!(q < 128);
                if r == 0 {
                    // xxxxxxxx
                    *self.data.get_unchecked_mut(q) = value;
                } else {
                    // yyyxxxxx|xxx00000
                    *self.data.get_unchecked_mut(q) |= value >> r;
                    if bits + r > 8 {
                        debug_assert!(q + 1 < 128);
                        *self.data.get_unchecked_mut(q + 1) = value << (8 - r);
                    }
                }
            };
            self.bit_len += bits;
            Ok(())
        } else {
            Err(Error::CellOverflow)
        }
    }

    /// Tries to store `u64` in the cell (but only the specified number of bits),
    /// returning `false` if there is not enough remaining capacity.
    ///
    /// NOTE: if `bits` is greater than **64**, pads the value with zeros (as high bits).
    pub fn store_uint(&mut self, mut value: u64, mut bits: u16) -> Result<(), Error> {
        if bits == 0 {
            return Ok(());
        }

        if self.bit_len + bits <= MAX_BIT_LEN {
            // Store zeros if bits is greater than 64
            bits = if let Some(offset) = bits.checked_sub(64) {
                self.bit_len += offset;
                64
            } else {
                bits
            };

            // Ensure that value starts with significant bits
            value <<= 64 - bits;

            let q = (self.bit_len / 8) as usize;
            let r = self.bit_len % 8;
            // SAFETY: q is in range 0..=127, r is in range 0..=7
            unsafe {
                let data_ptr = self.data.as_mut_ptr().add(q);
                if r == 0 {
                    let byte_len = bits.div_ceil(8) as usize;
                    debug_assert!(q + byte_len <= 128);

                    // Just append data
                    let value = value.to_be_bytes();
                    std::ptr::copy_nonoverlapping(value.as_ptr(), data_ptr, byte_len);
                } else {
                    debug_assert!(q < 128);

                    // Append high bits to the last byte
                    let shift = 8 - r;
                    *data_ptr |= (value >> (64 - shift)) as u8;

                    // If there are some bits left
                    if let Some(bits) = bits.checked_sub(shift)
                        && bits > 0
                    {
                        let byte_len = bits.div_ceil(8) as usize;
                        debug_assert!(q + 1 + byte_len <= 128);

                        // Make shifted bytes
                        let value: [u8; 8] = (value << shift).to_be_bytes();
                        // Write shifted bytes
                        std::ptr::copy_nonoverlapping(value.as_ptr(), data_ptr.add(1), byte_len);
                    }
                }
            }
            self.bit_len += bits;
            Ok(())
        } else {
            Err(Error::CellOverflow)
        }
    }

    /// Stores `bits`-width [`BigInt`] to the builder.
    #[cfg(feature = "bigint")]
    pub fn store_bigint(
        &mut self,
        x: &num_bigint::BigInt,
        bits: u16,
        signed: bool,
    ) -> Result<(), Error> {
        use crate::util::BigIntExt;

        let int_bits = x.bitsize(signed);
        if int_bits > bits {
            return Err(Error::IntOverflow);
        }
        self.store_bigint_truncate(x, bits, signed)
    }

    /// Stores `bits`-width [`BigInt`] to the builder.
    /// `x` bit size is ignored and only low `bits` of it are written.
    #[cfg(feature = "bigint")]
    pub fn store_bigint_truncate(
        &mut self,
        x: &num_bigint::BigInt,
        bits: u16,
        signed: bool,
    ) -> Result<(), Error> {
        use std::borrow::Cow;

        use num_traits::ToPrimitive;

        match x.to_u64() {
            Some(value) => self.store_uint(value, bits),
            None => {
                let int = if !bits.is_multiple_of(8) {
                    let align = 8 - bits % 8;
                    Cow::Owned(x.clone() << align)
                } else {
                    Cow::Borrowed(x)
                };

                let minimal_bytes = bits.div_ceil(8) as usize;

                let (prefix, mut bytes) = if signed {
                    let bytes = int.to_signed_bytes_le();
                    (
                        bytes
                            .last()
                            .map(|first| (first >> 7) * 255)
                            .unwrap_or_default(),
                        bytes,
                    )
                } else {
                    (0, int.to_bytes_le().1)
                };
                bytes.resize(minimal_bytes, prefix);
                bytes.reverse();

                self.store_raw(&bytes, bits)
            }
        }
    }

    /// Stores `bits`-width [`BigUint`] to the builder.
    #[cfg(feature = "bigint")]
    pub fn store_biguint(
        &mut self,
        x: &num_bigint::BigUint,
        bits: u16,
        signed: bool,
    ) -> Result<(), Error> {
        use crate::util::BigIntExt;

        let int_bits = x.bitsize(signed);
        if int_bits > bits {
            return Err(Error::IntOverflow);
        }
        self.store_biguint_truncate(x, bits)
    }

    /// Stores `bits`-width [`BigUint`] to the builder.
    /// `x` bit size is ignored and only low `bits` of it are written.
    #[cfg(feature = "bigint")]
    pub fn store_biguint_truncate(
        &mut self,
        x: &num_bigint::BigUint,
        bits: u16,
    ) -> Result<(), Error> {
        use std::borrow::Cow;

        use num_traits::ToPrimitive;

        match x.to_u64() {
            Some(value) => self.store_uint(value, bits),
            None => {
                let int = if !bits.is_multiple_of(8) {
                    let align = 8 - bits % 8;
                    Cow::Owned(x.clone() << align)
                } else {
                    Cow::Borrowed(x)
                };

                let minimal_bytes = bits.div_ceil(8) as usize;

                let mut bytes = int.to_bytes_le();
                bytes.resize(minimal_bytes, 0);
                bytes.reverse();

                self.store_raw(&bytes, bits)
            }
        }
    }

    /// Loads variable length [`BigInt`] to the builder.
    #[cfg(feature = "bigint")]
    pub fn store_var_bigint(
        &mut self,
        x: &num_bigint::BigInt,
        len_bits: u16,
        signed: bool,
    ) -> Result<(), Error> {
        use crate::util::BigIntExt;

        let bits = x.bitsize(signed);
        let bytes = bits.div_ceil(8);
        if bytes >= (1 << len_bits) {
            return Err(Error::IntOverflow);
        }

        ok!(self.store_small_uint(bytes as u8, len_bits));
        self.store_bigint_truncate(x, bytes * 8, signed)
    }

    /// Loads variable length [`BigInt`] to the builder.
    #[cfg(feature = "bigint")]
    pub fn store_var_biguint(
        &mut self,
        x: &num_bigint::BigUint,
        len_bits: u16,
    ) -> Result<(), Error> {
        let bits = x.bits() as u16;
        let bytes = bits.div_ceil(8);
        if bytes >= (1 << len_bits) {
            return Err(Error::IntOverflow);
        }

        ok!(self.store_small_uint(bytes as u8, len_bits));
        self.store_biguint_truncate(x, bytes * 8)
    }

    /// Tries to store bytes in the cell (but only the specified number of bits),
    /// returning `false` if there is not enough remaining capacity.
    ///
    /// NOTE: if `bits` is greater than `bytes * 8`, pads the value with zeros (as high bits).
    pub fn store_raw(&mut self, value: &[u8], bits: u16) -> Result<(), Error> {
        store_raw(&mut self.data, &mut self.bit_len, value, bits)
    }

    /// Tries to store all data bits of the specified cell in the current cell,
    /// returning `false` if there is not enough remaining capacity.
    #[inline]
    pub fn store_cell_data<T>(&mut self, value: T) -> Result<(), Error>
    where
        T: AsRef<DynCell>,
    {
        fn store_cell_data_impl(
            builder: &mut CellDataBuilder,
            value: &DynCell,
        ) -> Result<(), Error> {
            store_raw(
                &mut builder.data,
                &mut builder.bit_len,
                value.data(),
                value.bit_len(),
            )
        }
        store_cell_data_impl(self, value.as_ref())
    }

    /// Tries to store the remaining slice data in the cell,
    /// returning `false` if there is not enough remaining capacity.
    #[inline]
    pub fn store_slice_data<'a, T>(&mut self, value: T) -> Result<(), Error>
    where
        T: AsRef<CellSlice<'a>>,
    {
        fn store_slice_data_impl(
            builder: &mut CellDataBuilder,
            value: &CellSlice<'_>,
        ) -> Result<(), Error> {
            let bits = value.size_bits();
            if builder.bit_len + bits <= MAX_BIT_LEN {
                // SAFETY: An uninitialized `[MaybeUninit<_>; LEN]` is valid.
                let mut slice_data =
                    unsafe { MaybeUninit::<[MaybeUninit<u8>; 128]>::uninit().assume_init() };

                // SAFETY: casting `slice_data` to a `*const [u8]` is safe since `MaybeUninit`
                // is guaranteed to have the same layout as `u8`.
                // The pointer obtained is valid since it refers to memory owned by `slice_data`
                // which is a reference and thus guaranteed to be valid for reads.
                let slice_data = unsafe {
                    &mut *(&mut slice_data as *mut [MaybeUninit<u8>; 128] as *mut [u8; 128])
                };

                let slice_data = ok!(value.get_raw(0, slice_data, bits));
                builder.store_raw(slice_data, bits)
            } else {
                Err(Error::CellOverflow)
            }
        }
        store_slice_data_impl(self, value.as_ref())
    }

    /// Tries to prepend bytes to the cell data (but only the specified number of bits),
    /// returning `false` if there is not enough capacity.
    ///
    /// NOTE: if `bits` is greater than `bytes * 8`, pads the value with zeros (as high bits).
    pub fn prepend_raw(&mut self, value: &[u8], bits: u16) -> Result<(), Error> {
        if bits == 0 {
            return Ok(());
        }

        // Prevent asm code bloat
        fn store_raw_impl(
            data: &mut [u8; 128],
            bit_len: &mut u16,
            value: &[u8],
            bits: u16,
        ) -> Result<(), Error> {
            store_raw(data, bit_len, value, bits)
        }

        if self.bit_len + bits <= MAX_BIT_LEN {
            let mut data = [0; 128];
            let mut bit_len = 0;
            ok!(store_raw_impl(&mut data, &mut bit_len, value, bits));
            ok!(store_raw_impl(
                &mut data,
                &mut bit_len,
                &self.data,
                self.bit_len
            ));
            self.data = data;
            self.bit_len = bit_len;
            Ok(())
        } else {
            Err(Error::CellOverflow)
        }
    }
}

#[inline]
fn store_raw(
    data: &mut [u8; 128],
    bit_len: &mut u16,
    value: &[u8],
    mut bits: u16,
) -> Result<(), Error> {
    if *bit_len + bits <= MAX_BIT_LEN {
        let max_bit_len = value.len().saturating_mul(8) as u16;
        bits = if let Some(offset) = bits.checked_sub(max_bit_len) {
            *bit_len += offset;
            max_bit_len
        } else {
            bits
        };

        // Do nothing for empty slices or noop store
        if bits == 0 {
            return Ok(());
        }

        let q = (*bit_len / 8) as usize;
        let r = *bit_len % 8;
        // SAFETY: q is in range 0..=127, r is in range 0..=7
        unsafe {
            let mut data_ptr = data.as_mut_ptr().add(q);
            let mut value_ptr = value.as_ptr();

            if r == 0 {
                let byte_len = bits.div_ceil(8) as usize;
                debug_assert!(q + byte_len <= 128);
                debug_assert!(byte_len <= value.len());

                std::ptr::copy_nonoverlapping(value_ptr, data_ptr, byte_len);

                let bits_r = bits % 8;
                if bits_r != 0 {
                    *data_ptr.add(byte_len - 1) &= 0xff << (8 - bits_r);
                }
            } else {
                let byte_len = (bits + r).div_ceil(8) as usize - 1;
                let value_len = bits.div_ceil(8) as usize;
                debug_assert!(q + byte_len <= 128);
                debug_assert!(byte_len <= value_len && value_len <= value.len());

                let shift = 8 - r;
                for _ in 0..byte_len {
                    *data_ptr |= *value_ptr >> r;
                    data_ptr = data_ptr.add(1);
                    *data_ptr = *value_ptr << shift;
                    value_ptr = value_ptr.add(1);
                }
                if byte_len < value_len {
                    *data_ptr |= *value_ptr >> r;
                }

                let bits_r = (r + bits) % 8;
                if bits_r != 0 {
                    *data_ptr &= 0xff << (8 - bits_r);
                }
            }
        }
        *bit_len += bits;
        Ok(())
    } else {
        Err(Error::CellOverflow)
    }
}

/// Builder for constructing cell references array.
///
/// Can be used later for [`CellBuilder::set_references`].
#[derive(Default)]
#[repr(transparent)]
pub struct CellRefsBuilder(ArrayVec<Cell, MAX_REF_COUNT>);

impl CellRefsBuilder {
    /// Tries to store a child in the cell,
    /// returning `false` if there is not enough remaining capacity.
    pub fn store_reference(&mut self, cell: Cell) -> Result<(), Error> {
        if self.0.len() < MAX_REF_COUNT {
            // SAFETY: reference count is in the valid range
            unsafe { self.0.push(cell) }
            Ok(())
        } else {
            Err(Error::CellOverflow)
        }
    }

    /// Computes children level mask as a combination of all level masks.
    pub fn compute_level_mask(&self) -> LevelMask {
        let mut result = LevelMask::EMPTY;
        for child in self.0.as_ref() {
            result |= child.as_ref().level_mask();
        }
        result
    }

    /// Returns the number of references in the builder, also referred to as its ‘length’.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if the builder contains no references.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns an iterator over stored reverences.
    pub fn iter(&self) -> std::slice::Iter<'_, Cell> {
        self.0.as_ref().iter()
    }
}

impl IntoIterator for CellRefsBuilder {
    type Item = Cell;
    type IntoIter = ArrayVecIntoIter<Cell, MAX_REF_COUNT>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a CellRefsBuilder {
    type Item = &'a Cell;
    type IntoIter = std::slice::Iter<'a, Cell>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.0.as_ref().iter()
    }
}

#[repr(transparent)]
struct IntermediateDataCell(CellDataBuilder);

impl IntermediateDataCell {
    #[inline(always)]
    const fn wrap(value: &CellDataBuilder) -> &Self {
        // SAFETY: IntermediateDataCell is #[repr(transparent)]
        unsafe { &*(value as *const CellDataBuilder as *const Self) }
    }
}

impl CellImpl for IntermediateDataCell {
    fn untrack(self: CellInner<Self>) -> Cell {
        Cell(self)
    }

    fn descriptor(&self) -> CellDescriptor {
        CellDescriptor {
            d1: 0,
            d2: CellDescriptor::compute_d2(self.0.bit_len),
        }
    }

    fn data(&self) -> &[u8] {
        self.0.raw_data()
    }

    fn bit_len(&self) -> u16 {
        self.0.bit_len
    }

    fn reference(&self, _: u8) -> Option<&DynCell> {
        None
    }

    fn reference_cloned(&self, _: u8) -> Option<Cell> {
        None
    }

    fn reference_repr_hash(&self, _: u8) -> Option<HashBytes> {
        None
    }

    fn virtualize(&self) -> &DynCell {
        self
    }

    fn hash(&self, _: u8) -> &HashBytes {
        panic!("Hash for an intermediate data cell is not defined");
    }

    fn depth(&self, _: u8) -> u16 {
        0
    }
}

#[repr(transparent)]
struct IntermediateFullCell(CellBuilder);

impl IntermediateFullCell {
    #[inline(always)]
    const fn wrap(value: &CellBuilder) -> &Self {
        // SAFETY: IntermediateFullCell is #[repr(transparent)]
        unsafe { &*(value as *const CellBuilder as *const Self) }
    }
}

impl CellImpl for IntermediateFullCell {
    fn untrack(self: CellInner<Self>) -> Cell {
        Cell(self)
    }

    fn descriptor(&self) -> CellDescriptor {
        CellDescriptor {
            d1: CellDescriptor::compute_d1(LevelMask::EMPTY, false, self.0.references.len() as u8),
            d2: CellDescriptor::compute_d2(self.0.bit_len),
        }
    }

    fn data(&self) -> &[u8] {
        self.0.raw_data()
    }

    fn bit_len(&self) -> u16 {
        self.0.bit_len
    }

    fn reference(&self, index: u8) -> Option<&DynCell> {
        match self.0.references.get(index) {
            Some(cell) => Some(cell.as_ref()),
            None => None,
        }
    }

    fn reference_cloned(&self, index: u8) -> Option<Cell> {
        self.0.references.get(index).cloned()
    }

    fn reference_repr_hash(&self, index: u8) -> Option<HashBytes> {
        Some(*self.0.references.get(index)?.repr_hash())
    }

    fn virtualize(&self) -> &DynCell {
        self
    }

    fn hash(&self, _: u8) -> &HashBytes {
        panic!("Hash for an intermediate data cell is not defined");
    }

    fn depth(&self, _: u8) -> u16 {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clone_builder() {
        let mut builder = CellBuilder::new();
        builder.store_u32(0xdeafbeaf).unwrap();
        let cell1 = builder.clone().build().unwrap();
        let cell2 = builder.clone().build().unwrap();
        assert_eq!(cell1.as_ref(), cell2.as_ref());

        builder.store_u32(0xb00b5).unwrap();
        let cell3 = builder.build().unwrap();
        assert_ne!(cell1.as_ref(), cell3.as_ref());
    }

    #[test]
    fn compare_builders() {
        let mut a = CellBuilder::new();
        a.store_u32(0xdeafbeaf).unwrap();

        let mut b = CellBuilder::new();
        b.store_u32(0xdeafbeaf).unwrap();

        assert_eq!(a, b);

        b.store_u8(1).unwrap();
        assert!(a < b);
        a.store_u8(2).unwrap();
        assert!(a > b);

        a.rewind_bits(8).unwrap();
        a.store_u8(1).unwrap();
        assert_eq!(a, b);

        let child_a = a.clone().build().unwrap();
        a.store_reference(child_a.clone()).unwrap();
        assert!(a > b);

        let child_b = b.clone().build().unwrap();
        b.store_reference(child_b).unwrap();
        assert_eq!(a, b);

        let child_b2 = b.clone().build().unwrap();
        a.store_reference(child_a).unwrap();
        b.store_reference(child_b2).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn rewind_builder() {
        let mut builder = CellBuilder::new();
        builder.store_u32(0xdeafbeaf).unwrap();
        assert_eq!(builder.size_bits(), 32);
        assert_eq!(builder.data[..4], 0xdeafbeaf_u32.to_be_bytes());

        builder.rewind_bits(5).unwrap();
        assert_eq!(builder.size_bits(), 27);
        assert_eq!(builder.data[..4], 0xdeafbea0_u32.to_be_bytes());

        builder.store_u32(0xdeafbeaf).unwrap();
        assert_eq!(builder.size_bits(), 32 + 27);
        assert_eq!(builder.data[..8], [
            0xde, 0xaf, 0xbe, 0xbb, 0xd5, 0xf7, 0xd5, 0xe0
        ]);
        builder.rewind_bits(32).unwrap();
        assert_eq!(builder.data[..8], [
            0xde, 0xaf, 0xbe, 0xa0, 0x00, 0x00, 0x00, 0x00
        ]);

        assert_eq!(builder.rewind_bits(32), Err(Error::CellUnderflow));

        builder.rewind_bits(27).unwrap();
        assert_eq!(builder.size_bits(), 0);
        assert_eq!(builder.data, [0u8; 128]);

        builder.store_raw(&[0xff; 128], MAX_BIT_LEN).unwrap();
        assert_eq!(builder.size_bits(), MAX_BIT_LEN);

        let mut target = [0xff; 128];
        target[127] = 0xfe;
        assert_eq!(builder.data, target);

        builder.rewind_bits(3).unwrap();
        assert_eq!(builder.size_bits(), MAX_BIT_LEN - 3);
        target[127] = 0xf0;
        assert_eq!(builder.data, target);

        builder.rewind_bits(8).unwrap();
        assert_eq!(builder.size_bits(), MAX_BIT_LEN - 3 - 8);
        target[126] = 0xf0;
        target[127] = 0x00;
        assert_eq!(builder.data, target);
    }

    #[test]
    #[cfg_attr(miri, ignore)] // takes too long to execute on miri
    fn store_raw() {
        const ONES: &[u8] = &[0xff; 128];
        for offset in 0..8 {
            for bits in 0..=1016 {
                let mut builder = CellBuilder::new();
                builder.store_zeros(offset).unwrap();
                builder.store_raw(ONES, bits).unwrap();
                builder.build().unwrap();
            }
        }
    }

    #[test]
    fn prepend_raw() {
        let mut builder = CellBuilder::new();
        builder.store_raw(&[0xde, 0xaf, 0xbe, 0xaf], 20).unwrap();
        builder.prepend_raw(&[0xaa, 0x55], 5).unwrap();
        let cell = builder.build().unwrap();
        println!("{}", cell.display_tree());
    }

    #[test]
    fn store_slice() -> anyhow::Result<()> {
        const SOME_HASH: &HashBytes = HashBytes::wrap(&[
            0xdf, 0x86, 0xce, 0xbc, 0xe8, 0xd5, 0xab, 0x0c, 0x69, 0xb4, 0xce, 0x33, 0xfe, 0x9b,
            0x0e, 0x2c, 0xdf, 0x69, 0xa3, 0xe1, 0x13, 0x7e, 0x64, 0x85, 0x6b, 0xbc, 0xfd, 0x39,
            0xe7, 0x9b, 0xc1, 0x6f,
        ]);

        let mut builder = CellBuilder::new();
        builder.store_zeros(3)?;
        builder.store_u256(SOME_HASH)?;
        let cell = builder.build()?;
        println!("{}", cell.display_tree());

        let mut builder = CellBuilder::new();
        let mut slice = cell.as_slice()?;
        slice.skip_first(3, 0)?;
        builder.store_slice(slice)?;
        let cell = builder.build()?;
        println!("{}", cell.display_tree());
        assert_eq!(cell.data(), SOME_HASH);

        Ok(())
    }
}
