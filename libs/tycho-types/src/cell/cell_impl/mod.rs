use std::marker::PhantomData;
use std::mem::{ManuallyDrop, MaybeUninit};

use super::{
    Cell, CellDescriptor, CellFamily, CellImpl, CellInner, DynCell, EMPTY_CELL_HASH, HashBytes,
    MAX_REF_COUNT,
};
use crate::util::TryAsMut;

macro_rules! define_gen_vtable_ptr {
    (($($param:tt)*) => $($type:tt)*) => {
        const fn gen_vtable_ptr<$($param)*>() -> *const () {
            // SAFETY: "fat" pointer consists of two "slim" pointers
            let [_, vtable] = unsafe {
                std::mem::transmute::<*const dyn crate::cell::CellImpl, [*const (); 2]>(
                    std::ptr::null::<$($type)*>(),
                )
            };
            vtable
        }
    };
}

/// Single-threaded cell implementation.
#[cfg(not(feature = "sync"))]
pub mod rc;
/// Thread-safe cell implementation.
#[cfg(feature = "sync")]
pub mod sync;

/// Helper struct for tightly packed cell data.
#[repr(C)]
struct HeaderWithData<H, const N: usize> {
    header: H,
    data: [u8; N],
}

struct EmptyOrdinaryCell;

impl CellImpl for EmptyOrdinaryCell {
    fn untrack(self: CellInner<Self>) -> Cell {
        Cell(self)
    }

    fn descriptor(&self) -> CellDescriptor {
        CellDescriptor::new([0, 0])
    }

    fn data(&self) -> &[u8] {
        &[]
    }

    fn bit_len(&self) -> u16 {
        0
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
        EMPTY_CELL_HASH
    }

    fn depth(&self, _: u8) -> u16 {
        0
    }
}

/// Static cell which can be used to create cell references in const context.
pub struct StaticCell {
    descriptor: CellDescriptor,
    data: &'static [u8],
    bit_len: u16,
    hash: &'static [u8; 32],
}

impl StaticCell {
    /// Creates a new plain ordinary cell from parts.
    ///
    /// # Safety
    ///
    /// The following must be true:
    /// - Data must be well-formed and normalized (contain bit tag if needed and
    ///   be enough to store `bit_len` of bits).
    /// - `bit_len` must be in range 0..=1023
    /// - `hash` must be a correct hash for the current cell.
    pub const unsafe fn new(data: &'static [u8], bit_len: u16, hash: &'static [u8; 32]) -> Self {
        Self {
            descriptor: CellDescriptor::new([0, CellDescriptor::compute_d2(bit_len)]),
            data,
            bit_len,
            hash,
        }
    }
}

impl CellImpl for StaticCell {
    #[inline]
    fn untrack(self: CellInner<Self>) -> Cell {
        Cell(self)
    }

    fn descriptor(&self) -> CellDescriptor {
        self.descriptor
    }

    fn data(&self) -> &[u8] {
        self.data
    }

    fn bit_len(&self) -> u16 {
        self.bit_len
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
        HashBytes::wrap(self.hash)
    }

    fn depth(&self, _: u8) -> u16 {
        0
    }
}

pub(crate) static ALL_ZEROS_CELL: StaticCell = StaticCell {
    descriptor: CellDescriptor::new([0, 0xff]),
    data: &ALL_ZEROS_CELL_DATA,
    bit_len: 1023,
    hash: &ALL_ZEROS_CELL_HASH,
};

const ALL_ZEROS_CELL_DATA: [u8; 128] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
];

const ALL_ZEROS_CELL_HASH: [u8; 32] = [
    0xba, 0x03, 0x8d, 0x92, 0x4d, 0xa0, 0xb4, 0x2c, 0x44, 0x76, 0x62, 0xe6, 0xb8, 0xa5, 0x3f, 0x15,
    0x88, 0x9e, 0xbd, 0xf9, 0xd3, 0xb2, 0xf0, 0x1d, 0xbf, 0x94, 0x2c, 0x29, 0xbc, 0x48, 0x98, 0x71,
];

pub(crate) static ALL_ONES_CELL: StaticCell = StaticCell {
    descriptor: CellDescriptor::new([0, 0xff]),
    data: &ALL_ONES_CELL_DATA,
    bit_len: 1023,
    hash: &ALL_ONES_CELL_HASH,
};

const ALL_ONES_CELL_DATA: [u8; 128] = [0xff; 128];

const ALL_ONES_CELL_HASH: [u8; 32] = [
    0x82, 0x97, 0x0d, 0x46, 0x64, 0xb7, 0x68, 0x3c, 0x3d, 0x14, 0xd4, 0x9b, 0x1f, 0x9f, 0xf3, 0x49,
    0x66, 0x12, 0x81, 0x70, 0x30, 0x1a, 0x7b, 0xec, 0xc2, 0x7a, 0xf1, 0xad, 0xbe, 0x6a, 0x31, 0xc9,
];

type OrdinaryCell<const N: usize> = HeaderWithData<OrdinaryCellHeader, N>;

struct OrdinaryCellHeader {
    bit_len: u16,
    hashes: Box<[(HashBytes, u16)]>,
    descriptor: CellDescriptor,
    references: [MaybeUninit<Cell>; MAX_REF_COUNT],
}

impl OrdinaryCellHeader {
    fn level_descr(&self, level: u8) -> &(HashBytes, u16) {
        let hash_index = hash_index(self.descriptor, level);
        debug_assert!((hash_index as usize) < self.hashes.len());

        // SAFETY: hash index is in range 0..=3
        unsafe { self.hashes.get_unchecked(hash_index as usize) }
    }

    fn reference(&self, i: u8) -> Option<&Cell> {
        if i < self.descriptor.reference_count() {
            // SAFETY: Item is initialized
            let child = unsafe { self.references.get_unchecked(i as usize).assume_init_ref() };
            Some(child)
        } else {
            None
        }
    }
}

impl Drop for OrdinaryCellHeader {
    fn drop(&mut self) {
        let references_ptr = self.references.as_mut_ptr() as *mut Cell;
        debug_assert!(self.descriptor.reference_count() <= MAX_REF_COUNT as u8);

        for i in 0..self.descriptor.reference_count() {
            // SAFETY: references were initialized
            let child = unsafe { std::ptr::read(references_ptr.add(i as usize)) };
            if CellInner::strong_count(&child.0) == 1 {
                SafeDeleter::retire(child.0);
            }
        }
    }
}

// TODO: merge VTables for different data array sizes

impl<const N: usize> CellImpl for OrdinaryCell<N> {
    #[inline]
    fn untrack(self: CellInner<Self>) -> Cell {
        Cell(self)
    }

    fn descriptor(&self) -> CellDescriptor {
        self.header.descriptor
    }

    fn data(&self) -> &[u8] {
        let data_ptr = std::ptr::addr_of!(self.data) as *const u8;
        let data_len = self.header.descriptor.byte_len() as usize;
        // SAFETY: header is initialized
        unsafe { std::slice::from_raw_parts(data_ptr, data_len) }
    }

    fn bit_len(&self) -> u16 {
        self.header.bit_len
    }

    fn reference(&self, index: u8) -> Option<&DynCell> {
        Some(self.header.reference(index)?.as_ref())
    }

    fn reference_cloned(&self, index: u8) -> Option<Cell> {
        Some(self.header.reference(index)?.clone())
    }

    fn reference_repr_hash(&self, index: u8) -> Option<HashBytes> {
        Some(*self.header.reference(index)?.repr_hash())
    }

    fn virtualize(&self) -> &DynCell {
        if self.header.descriptor.level_mask().is_empty() {
            self
        } else {
            VirtualCellWrapper::wrap(self)
        }
    }

    fn hash(&self, level: u8) -> &HashBytes {
        &self.header.level_descr(level).0
    }

    fn depth(&self, level: u8) -> u16 {
        self.header.level_descr(level).1
    }
}

struct LibraryReference {
    repr_hash: HashBytes,
    descriptor: CellDescriptor,
    data: [u8; 33],
}

impl LibraryReference {
    const BIT_LEN: u16 = 8 + 256;
}

impl CellImpl for LibraryReference {
    #[inline]
    fn untrack(self: CellInner<Self>) -> Cell {
        Cell(self)
    }

    fn descriptor(&self) -> CellDescriptor {
        self.descriptor
    }

    fn data(&self) -> &[u8] {
        self.data.as_ref()
    }

    fn bit_len(&self) -> u16 {
        LibraryReference::BIT_LEN
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
        &self.repr_hash
    }

    fn depth(&self, _: u8) -> u16 {
        0
    }
}

type PrunedBranch<const N: usize> = HeaderWithData<PrunedBranchHeader, N>;

struct PrunedBranchHeader {
    repr_hash: HashBytes,
    level: u8,
    descriptor: CellDescriptor,
}

impl PrunedBranchHeader {
    #[inline]
    pub const fn cell_data_len(level: usize) -> usize {
        2 + level * (32 + 2)
    }
}

impl<const N: usize> CellImpl for PrunedBranch<N> {
    #[inline]
    fn untrack(self: CellInner<Self>) -> Cell {
        Cell(self)
    }

    fn descriptor(&self) -> CellDescriptor {
        self.header.descriptor
    }

    fn data(&self) -> &[u8] {
        let data_ptr = std::ptr::addr_of!(self.data) as *const u8;
        let data_len = self.header.descriptor.byte_len() as usize;
        // SAFETY: header is initialized
        unsafe { std::slice::from_raw_parts(data_ptr, data_len) }
    }

    fn bit_len(&self) -> u16 {
        8 + 8 + (self.header.level as u16) * (256 + 16)
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
        VirtualCellWrapper::wrap(self)
    }

    fn hash(&self, level: u8) -> &HashBytes {
        let hash_index = hash_index(self.header.descriptor, level);
        if hash_index == self.header.level {
            &self.header.repr_hash
        } else {
            let offset = 2 + hash_index as usize * 32;
            debug_assert!(offset + 32 <= self.header.descriptor.byte_len() as usize);

            let data_ptr = std::ptr::addr_of!(self.data) as *const u8;

            // SAFETY: Cell was created from a well-formed parts, so data is big enough
            HashBytes::wrap(unsafe { &*(data_ptr.add(offset) as *const [u8; 32]) })
        }
    }

    fn depth(&self, level: u8) -> u16 {
        let hash_index = hash_index(self.header.descriptor, level);
        if hash_index == self.header.level {
            0
        } else {
            let offset = 2 + self.header.level as usize * 32 + hash_index as usize * 2;
            debug_assert!(offset + 2 <= self.header.descriptor.byte_len() as usize);

            let data_ptr = std::ptr::addr_of!(self.data) as *const u8;

            // SAFETY: Cell was created from a well-formed parts, so data is big enough
            u16::from_be_bytes(unsafe { *(data_ptr.add(offset) as *const [u8; 2]) })
        }
    }
}

/// A cell that is missing from a tree.
///
/// Somewhat similar to pruned cell, but doesn't affect hashes
/// of parent cells and panics on data access.
///
/// Can only be constructed either via [`AbsentCell::new`] or
/// via BOC deserialization.
///
/// [`CellBuilder`]: crate::cell::CellBuilder
pub struct AbsentCell {
    d1: u8,
    hashes: Box<[(HashBytes, u16)]>,
}

impl AbsentCell {
    /// Creates `Self` from the provided cell.
    ///
    /// Creating [`AbsentCell`] from an [`AbsentCell`] gives the same cell.
    #[allow(clippy::new_ret_no_self)]
    pub fn new<T: AsRef<DynCell>>(cell: T) -> Cell {
        let cell = cell.as_ref();
        let desc = cell.descriptor();
        // Copy only level mask bits and fill the rest with absent mask.
        let d1 = (desc.d1 & CellDescriptor::LEVEL_MASK) | CellDescriptor::ABSENT_MASK;

        let hash_count = 1 + desc.level_mask().level() as usize;
        let mut hashes = Vec::with_capacity(hash_count);
        for level in desc.level_mask() {
            hashes.push((*cell.hash(level), cell.depth(level)));
        }
        debug_assert_eq!(hash_count, hashes.len());

        Cell(CellInner::new(Self {
            d1,
            hashes: hashes.into_boxed_slice(),
        }))
    }
}

impl CellImpl for AbsentCell {
    #[inline]
    fn untrack(self: CellInner<Self>) -> Cell {
        Cell(self)
    }

    fn descriptor(&self) -> CellDescriptor {
        CellDescriptor { d1: self.d1, d2: 0 }
    }

    fn data(&self) -> &[u8] {
        panic!("absent cell has no data")
    }

    fn bit_len(&self) -> u16 {
        panic!("absent cell has no data (no bit_len either)")
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

    fn hash(&self, level: u8) -> &HashBytes {
        let descriptor = CellDescriptor { d1: self.d1, d2: 0 };
        let hash_index = hash_index(descriptor, level);
        let (hash, _) = &self
            .hashes
            .get(hash_index as usize)
            .expect("absent cell must be well-formed");
        hash
    }

    fn depth(&self, level: u8) -> u16 {
        let descriptor = CellDescriptor { d1: self.d1, d2: 0 };
        let hash_index = hash_index(descriptor, level);
        let (_, depth) = &self
            .hashes
            .get(hash_index as usize)
            .expect("absent cell must be well-formed");
        *depth
    }
}

#[repr(transparent)]
pub struct VirtualCell<T>(T);

impl<#[cfg(not(feature = "sync"))] T, #[cfg(feature = "sync")] T: Send + Sync> CellImpl
    for VirtualCell<T>
where
    T: AsRef<DynCell> + TryAsMut<DynCell> + 'static,
{
    #[inline]
    fn untrack(self: CellInner<Self>) -> Cell {
        Cell(self)
    }

    fn descriptor(&self) -> CellDescriptor {
        self.0.as_ref().descriptor()
    }

    fn data(&self) -> &[u8] {
        self.0.as_ref().data()
    }

    fn bit_len(&self) -> u16 {
        self.0.as_ref().bit_len()
    }

    fn reference(&self, index: u8) -> Option<&DynCell> {
        Some(self.0.as_ref().reference(index)?.virtualize())
    }

    fn reference_cloned(&self, index: u8) -> Option<Cell> {
        Some(Cell::virtualize(self.0.as_ref().reference_cloned(index)?))
    }

    fn reference_repr_hash(&self, index: u8) -> Option<HashBytes> {
        self.0.as_ref().reference_repr_hash(index)
    }

    fn virtualize(&self) -> &DynCell {
        self.0.as_ref().virtualize()
    }

    fn hash(&self, level: u8) -> &HashBytes {
        let cell = self.0.as_ref();
        cell.hash(virtual_hash_index(cell.descriptor(), level, 0))
    }

    fn depth(&self, level: u8) -> u16 {
        let cell = self.0.as_ref();
        cell.depth(virtual_hash_index(cell.descriptor(), level, 0))
    }
}

/// A wrapper type which implements a virtualized cell interface.
#[repr(transparent)]
pub struct VirtualCellWrapper<T, const L: u8 = 0>(T);

impl<T> VirtualCellWrapper<T, 0> {
    /// Wraps a cell reference.
    pub fn wrap(value: &T) -> &Self {
        // SAFETY: VirtualCellWrapper<T> is #[repr(transparent)]
        unsafe { &*(value as *const T as *const Self) }
    }
}

impl<#[cfg(not(feature = "sync"))] T, #[cfg(feature = "sync")] T: Send + Sync, const L: u8> CellImpl
    for VirtualCellWrapper<T, L>
where
    T: CellImpl + 'static,
{
    #[inline]
    fn untrack(self: CellInner<Self>) -> Cell {
        Cell(self)
    }

    fn descriptor(&self) -> CellDescriptor {
        self.0.descriptor()
    }

    fn data(&self) -> &[u8] {
        self.0.data()
    }

    fn bit_len(&self) -> u16 {
        self.0.bit_len()
    }

    fn reference(&self, index: u8) -> Option<&DynCell> {
        dyn_reference_virtualize(&self.0, index)
    }

    fn reference_cloned(&self, index: u8) -> Option<Cell> {
        dyn_reference_virtualize_cloned(&self.0, index)
    }

    fn reference_repr_hash(&self, index: u8) -> Option<HashBytes> {
        self.0.reference_repr_hash(index)
    }

    fn virtualize(&self) -> &DynCell {
        unsafe { virtualize_into_next_wrapper(L, &self.0) }
    }

    fn hash(&self, level: u8) -> &HashBytes {
        self.0
            .hash(virtual_hash_index(self.0.descriptor(), level, L))
    }

    fn depth(&self, level: u8) -> u16 {
        self.0
            .depth(virtual_hash_index(self.0.descriptor(), level, L))
    }
}

#[inline(never)]
fn dyn_reference_virtualize(cell: &DynCell, index: u8) -> Option<&DynCell> {
    Some(cell.reference(index)?.virtualize())
}

#[inline(never)]
fn dyn_reference_virtualize_cloned(cell: &DynCell, index: u8) -> Option<Cell> {
    Some(Cell::virtualize(cell.reference_cloned(index)?))
}

const unsafe fn virtualize_into_next_wrapper<
    #[cfg(not(feature = "sync"))] T: CellImpl + Sized + 'static,
    #[cfg(feature = "sync")] T: CellImpl + Sized + Send + Sync + 'static,
>(
    level: u8,
    cell: &T,
) -> &DynCell {
    const fn gen_vtable_ptr<
        #[cfg(not(feature = "sync"))] T: CellImpl + 'static,
        #[cfg(feature = "sync")] T: CellImpl + Send + Sync + 'static,
        const L: u8,
    >() -> *const () {
        // SAFETY: "fat" pointer consists of two "slim" pointers
        let [_, vtable] = unsafe {
            std::mem::transmute::<*const dyn CellImpl, [*const (); 2]>(std::ptr::null::<
                VirtualCellWrapper<T, L>,
            >())
        };
        vtable
    }

    struct Vtable<T>(PhantomData<T>);

    impl<#[cfg(not(feature = "sync"))] T, #[cfg(feature = "sync")] T: Send + Sync> Vtable<T>
    where
        T: CellImpl + 'static,
    {
        const LEVELS: [*const (); 2] = [gen_vtable_ptr::<T, 1>(), gen_vtable_ptr::<T, 2>()];
    }

    let vtable = Vtable::<T>::LEVELS[(level != 0) as usize];

    // Construct fat pointer with vtable info
    let data = cell as *const T as *const ();
    let ptr: *const DynCell = unsafe { std::mem::transmute([data, vtable]) };

    unsafe { &*ptr }
}

fn virtual_hash_index(descriptor: CellDescriptor, level: u8, offset: u8) -> u8 {
    descriptor
        .level_mask()
        .virtualize(1 + offset)
        .hash_index(level)
}

fn hash_index(descriptor: CellDescriptor, level: u8) -> u8 {
    descriptor.level_mask().hash_index(level)
}

// === Linear deleter ===

/// Linear deleter for cell trees.
pub struct SafeDeleter {
    // TODO: Drop too large vector after completion?
    to_delete: std::cell::RefCell<Vec<CellInner>>,
    is_active: std::cell::Cell<bool>,
}

impl SafeDeleter {
    /// Add cell into thread-local deleter queue.
    pub fn retire(value: CellInner) {
        thread_local! {
            static DELETER: SafeDeleter = const {
                SafeDeleter {
                    to_delete: std::cell::RefCell::new(Vec::new()),
                    is_active: std::cell::Cell::new(false),
                }
            };
        }

        let mut value = ManuallyDrop::new(value);

        match DELETER.try_with(|deleter| {
            // SAFETY: Value is going to be taken only once, because this
            // closure will run only if `LocalKey` still exists.
            let value = unsafe { ManuallyDrop::take(&mut value) };

            if deleter.is_active.get() {
                // Delay child value drop if we are already dropping some parent value.
                deleter.to_delete.borrow_mut().push(value);
                return;
            }

            // Activate delayed drop of children.
            deleter.is_active.set(true);

            // Drop the parent value. This will fill the `to_delete` vector
            // with children values.
            drop(value);

            // Iterate and "drop" children.
            loop {
                // Take one child value.
                let mut to_delete = deleter.to_delete.borrow_mut();
                match to_delete.pop() {
                    Some(value) => {
                        // Make sure to drop the `RefMut` guard first.
                        drop(to_delete);
                        // "Drop" the child.
                        drop(value);
                    }
                    None => break,
                }
            }

            // Disable delayed delete.
            deleter.is_active.set(false);
        }) {
            Ok(()) => {}
            // SAFETY: `LocalKey` does not exist anymore (we are removing some other
            // thread local SafeRc). So the closure above will not run and
            // we are dropping the value like always.
            Err(_) => drop(unsafe { ManuallyDrop::take(&mut value) }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AbsentCell;
    use crate::boc::Boc;
    use crate::cell::{Cell, CellBuilder, CellFamily};
    use crate::merkle::make_pruned_branch;

    #[test]
    fn static_cells() {
        let mut builder = CellBuilder::new();
        builder.store_zeros(1023).unwrap();
        let cell = builder.build().unwrap();
        let all_zeros = Cell::all_zeros_ref();
        assert_eq!(cell.as_ref().repr_hash(), all_zeros.repr_hash());
        assert_eq!(cell.as_ref().data(), all_zeros.data());
        assert_eq!(Boc::encode(cell.as_ref()), Boc::encode(all_zeros));

        let builder = CellBuilder::from_raw_data(&[0xff; 128], 1023).unwrap();
        let cell = builder.build().unwrap();
        let all_ones = Cell::all_ones_ref();
        assert_eq!(cell.as_ref().repr_hash(), all_ones.repr_hash());
        assert_eq!(cell.as_ref().data(), all_ones.data());
        assert_eq!(Boc::encode(cell.as_ref()), Boc::encode(all_ones));
    }

    #[test]
    fn absent_cell_works() {
        let simple_cell = CellBuilder::build_from(123u32).unwrap();
        let pruned_level1 =
            make_pruned_branch(simple_cell.as_ref(), 0, Cell::empty_context()).unwrap();
        let pruned_level2 =
            make_pruned_branch(pruned_level1.as_ref(), 1, Cell::empty_context()).unwrap();
        let pruned_level3 =
            make_pruned_branch(pruned_level2.as_ref(), 2, Cell::empty_context()).unwrap();

        for cell in [simple_cell, pruned_level1, pruned_level2, pruned_level3] {
            let absent_direct = AbsentCell::new(&cell);
            assert_eq!(absent_direct.level_mask(), cell.level_mask());
            assert_eq!(absent_direct.repr_hash(), cell.repr_hash());
            assert_eq!(absent_direct.repr_depth(), cell.repr_depth());

            for level in cell.level_mask() {
                assert_eq!(absent_direct.hash(level), cell.hash(level));
                assert_eq!(absent_direct.depth(level), cell.depth(level));
            }

            let absent_indirect = AbsentCell::new(&absent_direct);
            assert_eq!(
                absent_indirect.descriptor().level_mask(),
                absent_indirect.descriptor().level_mask()
            );
            assert_eq!(absent_direct.repr_hash(), absent_indirect.repr_hash());
            assert_eq!(absent_direct.repr_depth(), absent_indirect.repr_depth());
        }
    }
}
