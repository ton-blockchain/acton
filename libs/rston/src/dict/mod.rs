//! Dictionary implementation.

use std::ops::ControlFlow;

pub use self::aug::*;
pub use self::ops::*;
pub use self::raw::*;
pub use self::typed::*;
use crate::cell::*;
use crate::error::Error;

mod aug;
mod raw;
mod typed;

mod ops {
    pub(crate) use self::build::{
        AugMergeStackItemMode, MergeStackItem, MergeStackItemMode, SimpleMergeStackItemMode,
    };
    pub use self::build::{build_aug_dict_from_sorted_iter, build_dict_from_sorted_iter};
    pub use self::find::{
        aug_dict_find_by_extra, dict_find_bound, dict_find_bound_owned, dict_find_owned,
    };
    pub use self::get::{dict_get, dict_get_owned, dict_get_subdict};
    pub use self::insert::{aug_dict_insert, dict_insert, dict_insert_owned};
    pub use self::modify::{aug_dict_modify_from_sorted_iter, dict_modify_from_sorted_iter};
    pub use self::remove::{aug_dict_remove_owned, dict_remove_bound_owned, dict_remove_owned};
    pub use self::split_merge::{
        PartialSplitDict, aug_dict_merge_siblings, dict_merge, dict_merge_siblings,
        dict_split_by_prefix, dict_split_raw,
    };

    mod build;
    mod find;
    mod get;
    mod insert;
    mod modify;
    mod remove;
    mod split_merge;
}

/// Type which can be used as a dictionary key.
pub trait DictKey {
    /// Length in bits for a dictionary key.
    const BITS: u16;
}

impl<T: DictKey> DictKey for &T {
    const BITS: u16 = T::BITS;
}

impl<T: DictKey> DictKey for &mut T {
    const BITS: u16 = T::BITS;
}

/// Type which can be stored as a dict key.
pub trait StoreDictKey: DictKey {
    /// Stores key bits into a builder data.
    fn store_into_data(&self, data: &mut CellDataBuilder) -> Result<(), Error>;
}

impl<T: StoreDictKey> StoreDictKey for &T {
    #[inline]
    fn store_into_data(&self, data: &mut CellDataBuilder) -> Result<(), Error> {
        T::store_into_data(self, data)
    }
}

/// Type which can be loaded as a dict key.
pub trait LoadDictKey: DictKey + Sized {
    /// Creates a key from a raw builder data.
    fn load_from_data(data: &CellDataBuilder) -> Option<Self>;
}

macro_rules! impl_dict_key {
    ($($ty:ty => {
        bits: $bits:literal,
        store: |$this:ident, $builder:ident| $store_expr:expr,
        load: |$raw_data:ident| $load_expr:expr$(,)?
    }),*,) => {$(
        impl DictKey for $ty {
            const BITS: u16 = $bits;
        }

        impl StoreDictKey for $ty {
            #[inline]
            fn store_into_data(&self, $builder: &mut CellDataBuilder) -> Result<(), Error> {
                let $this = self;
                $store_expr
            }
        }

        impl LoadDictKey for $ty {
            #[inline]
            fn load_from_data(b: &CellDataBuilder) -> Option<Self> {
                let $raw_data = b.raw_data();
                Some($load_expr)
            }
        }
    )*};
}

impl_dict_key! {
    bool => {
        bits: 1,
        store: |x, b| b.store_bit(*x),
        load: |d| d[0] & 0x80 != 0,
    },
    u8 => {
        bits: 8,
        store: |x, b| b.store_u8(*x),
        load: |d| d[0],
    },
    i8 => {
        bits: 8,
        store: |x, b| b.store_u8(*x as u8),
        load: |d| d[0] as i8,
    },
    u16 => {
        bits: 16,
        store: |x, b| b.store_u16(*x),
        load: |d| u16::from_be_bytes([d[0], d[1]]),
    },
    i16 => {
        bits: 16,
        store: |x, b| b.store_u16(*x as u16),
        load: |d| i16::from_be_bytes([d[0], d[1]]),
    },
    u32 => {
        bits: 32,
        store: |x, b| b.store_u32(*x),
        load: |d| u32::from_be_bytes(d[..4].try_into().unwrap()),
    },
    i32 => {
        bits: 32,
        store: |x, b| b.store_u32(*x as u32),
        load: |d| i32::from_be_bytes(d[..4].try_into().unwrap()),
    },
    u64 => {
        bits: 64,
        store: |x, b| b.store_u64(*x),
        load: |d| u64::from_be_bytes(d[..8].try_into().unwrap()),
    },
    i64 => {
        bits: 64,
        store: |x, b| b.store_u64(*x as u64),
        load: |d| i64::from_be_bytes(d[..8].try_into().unwrap())
    },
    u128 => {
        bits: 128,
        store: |x, b| b.store_u128(*x),
        load: |d| u128::from_be_bytes(d[..16].try_into().unwrap()),
    },
    i128 => {
        bits: 128,
        store: |x, b| b.store_u128(*x as u128),
        load: |d| i128::from_be_bytes(d[..16].try_into().unwrap()),
    },
    [u8; 16] => {
        bits: 128,
        store: |x, b| b.store_raw(x, 128),
        load: |d| d[..16].try_into().unwrap(),
    },
    [u8; 20] => {
        bits: 160,
        store: |x, b| b.store_raw(x, 160),
        load: |d| d[..20].try_into().unwrap(),
    },
    [u8; 32] => {
        bits: 256,
        store: |x, b| b.store_raw(x, 256),
        load: |d| d[..32].try_into().unwrap(),
    },
    HashBytes => {
        bits: 256,
        store: |x, b| b.store_u256(x),
        load: |d| HashBytes(d[..32].try_into().unwrap()),
    },
     (u64, u32) => {
        bits: 96,
        store: |x, b| {
            ok!(b.store_u64(x.0));
            b.store_u32(x.1)
        },
        load: |d| {
            (u64::from_be_bytes(d[..8].try_into().unwrap()), u32::from_be_bytes(d[8..12].try_into().unwrap()))
        },
     },
}

/// `AugDict` search control flow.
pub trait SearchByExtra<A> {
    /// Returns if the leaf extra satisfies the condition.
    fn on_leaf(&mut self, leaf_extra: &A) -> bool {
        _ = leaf_extra;
        true
    }

    /// Returns which branch satisfies the condition.
    fn on_edge(&mut self, left_extra: &A, right_extra: &A) -> ControlFlow<(), Branch>;
}

impl<A, T: SearchByExtra<A>> SearchByExtra<A> for &mut T {
    #[inline]
    fn on_leaf(&mut self, leaf_extra: &A) -> bool {
        T::on_leaf(self, leaf_extra)
    }

    #[inline]
    fn on_edge(&mut self, left_extra: &A, right_extra: &A) -> ControlFlow<(), Branch> {
        T::on_edge(self, left_extra, right_extra)
    }
}

impl<A> SearchByExtra<A> for Branch {
    #[inline]
    fn on_edge(&mut self, _: &A, _: &A) -> ControlFlow<(), Branch> {
        ControlFlow::Continue(*self)
    }
}

impl<A: Ord> SearchByExtra<A> for std::cmp::Ordering {
    #[inline]
    fn on_edge(&mut self, left_extra: &A, right_extra: &A) -> ControlFlow<(), Branch> {
        ControlFlow::Continue(if *self == left_extra.cmp(right_extra) {
            Branch::Left
        } else {
            Branch::Right
        })
    }
}

/// Dictionary insertion mode.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum SetMode {
    /// Sets the value associated with the key in the dictionary.
    Set = 0b11,
    /// Sets the value associated with the key in the dictionary
    /// only if the key was already present in it.
    Replace = 0b01,
    /// Sets the value associated with key in dictionary,
    /// but only if it is not already present.
    Add = 0b10,
}

impl SetMode {
    /// Returns `true` if the new value can replace the old value for the same key.
    #[inline]
    pub const fn can_replace(self) -> bool {
        self as u8 & 0b01 != 0
    }

    /// Returns `true` if inserting a value can add a new key to the dictionary.
    #[inline]
    pub const fn can_add(self) -> bool {
        self as u8 & 0b10 != 0
    }
}

/// A type for a comparator function for [`AugDict`].
///
/// ## Args
/// - `left` - a left branch data.
/// - `right` - a right branch data.
/// - `builder` - a builder to write the result.
/// - `context` - a cell context.
pub type AugDictFn =
    fn(&mut CellSlice, &mut CellSlice, &mut CellBuilder, &dyn CellContext) -> Result<(), Error>;

/// Creates a leaf node
fn make_leaf(
    key: &CellSlice<'_>,
    key_bit_len: u16,
    value: &dyn Store,
    context: &dyn CellContext,
) -> Result<Cell, Error> {
    let mut builder = CellBuilder::new();
    ok!(write_label(key, key_bit_len, &mut builder));
    ok!(value.store_into(&mut builder, context));
    builder.build_ext(context)
}

/// Creates a leaf node with extra value
fn make_leaf_with_extra(
    key: &CellSlice<'_>,
    key_bit_len: u16,
    extra: &dyn Store,
    value: &dyn Store,
    context: &dyn CellContext,
) -> Result<Cell, Error> {
    let mut builder = CellBuilder::new();
    ok!(write_label(key, key_bit_len, &mut builder));
    ok!(extra.store_into(&mut builder, context));
    ok!(value.store_into(&mut builder, context));
    builder.build_ext(context)
}

/// Splits an edge or leaf
fn split_edge(
    data: &CellSlice,
    prefix: &mut CellSlice,
    lcp: &CellSlice,
    key: &mut CellSlice,
    value: &dyn Store,
    context: &dyn CellContext,
) -> Result<Cell, Error> {
    // Advance the key
    let prev_key_bit_len = key.size_bits();
    ok!(key.skip_first(lcp.size_bits() + 1, 0));

    // Read the next bit from the data
    prefix.skip_first(lcp.size_bits(), 0).ok();
    let old_to_right = ok!(prefix.load_bit());

    // Create a leaf for the old value
    let mut left = ok!(make_leaf(prefix, key.size_bits(), data, context));
    // Create a leaf for the right value
    let mut right = ok!(make_leaf(key, key.size_bits(), value, context));

    // The part that starts with 1 goes to the right cell
    if old_to_right {
        std::mem::swap(&mut left, &mut right);
    }

    // Create fork
    let mut builder = CellBuilder::new();
    ok!(write_label(lcp, prev_key_bit_len, &mut builder));
    ok!(builder.store_reference(left));
    ok!(builder.store_reference(right));
    builder.build_ext(context)
}

#[allow(clippy::too_many_arguments)]
fn split_aug_edge(
    data: &mut CellSlice,
    prefix: &mut CellSlice,
    lcp: &CellSlice,
    key: &mut CellSlice,
    extra: &dyn Store,
    value: &dyn Store,
    comparator: AugDictFn,
    context: &dyn CellContext,
) -> Result<Cell, Error> {
    // Advance the key
    let prev_key_bit_len = key.size_bits();
    ok!(key.skip_first(lcp.size_bits() + 1, 0));

    // Read the next bit from the data
    prefix.skip_first(lcp.size_bits(), 0).ok();
    let old_to_right = ok!(prefix.load_bit());

    // Create a leaf for the old value
    let mut left = ok!(make_leaf(prefix, key.size_bits(), data, context));
    // Create a leaf for the new value
    let mut right = ok!(make_leaf_with_extra(
        key,
        key.size_bits(),
        extra,
        value,
        context
    ));
    // The part that starts with 1 goes to the right cell
    if old_to_right {
        std::mem::swap(&mut left, &mut right);
    }

    let left_slice = &mut ok!(left.as_slice());
    let right_slice = &mut ok!(right.as_slice());
    ok!(skip_label_full(left_slice, key.size_bits()));
    ok!(skip_label_full(right_slice, key.size_bits()));

    // Create fork edge
    let mut builder = CellBuilder::new();
    ok!(write_label(lcp, prev_key_bit_len, &mut builder));
    ok!(builder.store_reference(left.clone()));
    ok!(builder.store_reference(right.clone()));
    ok!(comparator(left_slice, right_slice, &mut builder, context));
    builder.build_ext(context)
}

/// Type alias for a pair of key and value as cell slice parts.
pub type DictOwnedEntry = (CellDataBuilder, CellSliceParts);

/// Dictionary bound.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum DictBound {
    /// The lowest dictionary key.
    Min,
    /// The largest dictionary key.
    Max,
}

impl DictBound {
    fn update_direction(
        self,
        prefix: &CellSlice<'_>,
        signed: bool,
        direction: &mut Option<Branch>,
    ) -> Branch {
        match direction {
            // Compute direction by the first part
            None => {
                let mut branch = *direction.insert(self.into_branch());
                // Invert first bit for signed keys if starting from the empty part
                if signed && prefix.is_data_empty() {
                    branch = branch.reversed();
                }
                branch
            }
            // Use the same direction for all remaining parts
            Some(direction) => *direction,
        }
    }

    fn into_branch(self) -> Branch {
        match self {
            Self::Min => Branch::Left,
            Self::Max => Branch::Right,
        }
    }
}

/// Loads a non-empty dictionary from the root cell.
pub fn dict_load_from_root(
    slice: &mut CellSlice<'_>,
    key_bit_len: u16,
    context: &dyn CellContext,
) -> Result<Cell, Error> {
    let mut root = *slice;

    let label = ok!(read_label(slice, key_bit_len));
    if label.size_bits() != key_bit_len {
        ok!(slice.skip_first(0, 2));
        let root_bits = root.size_bits() - slice.size_bits();
        let root_refs = root.size_refs() - slice.size_refs();
        root = root.get_prefix(root_bits, root_refs)
    } else {
        slice.load_remaining();
    }

    let mut builder = CellBuilder::new();
    ok!(builder.store_slice(root));
    builder.build_ext(context)
}

fn rebuild_dict_from_stack(
    mut segments: Vec<Segment<'_>>,
    mut leaf: Cell,
    context: &dyn CellContext,
) -> Result<Cell, Error> {
    // Rebuild the tree starting from leaves
    while let Some(last) = segments.pop() {
        // Load the opposite branch
        let (left, right) = match last.next_branch {
            Branch::Left => match last.data.reference_cloned(1) {
                Some(cell) => (leaf, cell),
                None => return Err(Error::CellUnderflow),
            },
            Branch::Right => match last.data.reference_cloned(0) {
                Some(cell) => (cell, leaf),
                None => return Err(Error::CellUnderflow),
            },
        };

        let mut builder = CellBuilder::new();
        ok!(builder.store_cell_data(last.data));
        ok!(builder.store_reference(left));
        ok!(builder.store_reference(right));
        leaf = ok!(builder.build_ext(context));
    }

    Ok(leaf)
}

fn rebuild_aug_dict_from_stack(
    mut segments: Vec<Segment<'_>>,
    mut leaf: Cell,
    comparator: AugDictFn,
    context: &dyn CellContext,
) -> Result<Cell, Error> {
    // Rebuild the tree starting from leaves
    while let Some(last) = segments.pop() {
        // Load the opposite branch
        let (left, right) = match last.next_branch {
            Branch::Left => match last.data.reference_cloned(1) {
                Some(cell) => (leaf, cell),
                None => return Err(Error::CellUnderflow),
            },
            Branch::Right => match last.data.reference_cloned(0) {
                Some(cell) => (cell, leaf),
                None => return Err(Error::CellUnderflow),
            },
        };

        let last_data_slice = ok!(last.data.as_slice());
        let last_label = ok!(read_label(&mut last_data_slice.clone(), last.key_bit_len));

        let child_key_bit_len = last.key_bit_len - last_label.size_bits() - 1;

        let left_slice = &mut left.as_slice()?;
        let right_slice = &mut right.as_slice()?;
        ok!(skip_label_full(left_slice, child_key_bit_len));
        ok!(skip_label_full(right_slice, child_key_bit_len));

        let mut builder = CellBuilder::new();
        ok!(write_label(&last_label, last.key_bit_len, &mut builder));
        ok!(builder.store_reference(left.clone()));
        ok!(builder.store_reference(right.clone()));
        ok!(comparator(left_slice, right_slice, &mut builder, context));
        leaf = ok!(builder.build_ext(context));
    }

    Ok(leaf)
}

fn skip_label_full(slice: &mut CellSlice<'_>, key_bit_len: u16) -> Result<(), Error> {
    let label = ok!(read_label(slice, key_bit_len));
    if label.size_bits() < key_bit_len {
        // Cell is not a leaf so we need to skip 2 refs.
        slice.skip_first(0, 2)
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct Segment<'a> {
    data: &'a DynCell,
    next_branch: Branch,
    key_bit_len: u16,
}

impl Segment<'_> {
    // Returns the new leaf and the removed leaf
    fn rebuild_as_removed(
        self,
        key: &CellSlice<'_>,
        prev_key_bit_len: u16,
        context: &dyn CellContext,
    ) -> Result<(Cell, Cell), Error> {
        let index = self.next_branch as u8;

        // Load value branch
        let Some(value) = self.data.reference_cloned(index) else {
            return Err(Error::CellUnderflow);
        };
        // NOTE: do not use gas here as it was accounted while loading `child` in previous block.
        // TODO: change mode to `LoadMode::Noop` if copy-on-write for libraries is not ok.
        let value = ok!(context.load_cell(value, LoadMode::Resolve));

        // Load parent label
        let pfx = ok!(read_label(
            &mut self.data.as_slice_allow_exotic(),
            prev_key_bit_len
        ));

        // Load the opposite branch
        let mut opposite = match self.data.reference(1 - index) {
            // TODO: change mode to `LoadMode::UseGas` if copy-on-write for libraries is not ok.
            Some(cell) => ok!(context
                .load_dyn_cell(cell, LoadMode::Full)
                .and_then(CellSlice::new)),
            None => return Err(Error::CellUnderflow),
        };
        let rem = ok!(read_label(&mut opposite, key.size_bits()));

        // Build an edge cell
        let mut builder = CellBuilder::new();
        ok!(write_label_parts(
            &pfx,
            index == 0,
            &rem,
            prev_key_bit_len,
            &mut builder
        ));
        ok!(builder.store_slice(opposite));
        let leaf = ok!(builder.build_ext(context));

        Ok((leaf, value))
    }
}

fn write_label(key: &CellSlice, key_bit_len: u16, label: &mut CellBuilder) -> Result<(), Error> {
    if key_bit_len == 0 || key.is_data_empty() {
        return write_hml_empty(label);
    }

    let bits_for_len = (16 - key_bit_len.leading_zeros()) as u16;

    let remaining_bits = key.size_bits();

    let hml_short_len = 2 + 2 * remaining_bits;
    let hml_long_len = 2 + bits_for_len + remaining_bits;
    let hml_same_len = 3 + bits_for_len;

    if hml_same_len < hml_long_len
        && hml_same_len < hml_short_len
        && let Some(bit) = key.test_uniform()
    {
        ok!(write_hml_same(bit, remaining_bits, bits_for_len, label));
        return Ok(());
    }

    if hml_short_len <= MAX_BIT_LEN && hml_short_len <= hml_long_len {
        ok!(write_hml_short_tag(remaining_bits, label));
    } else if hml_long_len <= MAX_BIT_LEN {
        ok!(write_hml_long_tag(remaining_bits, bits_for_len, label));
    } else {
        return Err(Error::InvalidData);
    }

    ok!(label.store_slice_data(key));
    Ok(())
}

fn write_label_parts(
    pfx: &CellSlice,
    bit: bool,
    rem: &CellSlice,
    key_bit_len: u16,
    label: &mut CellBuilder,
) -> Result<(), Error> {
    if key_bit_len == 0 {
        return write_hml_empty(label);
    }

    let bits_for_len = (16 - key_bit_len.leading_zeros()) as u16;

    let remaining_bits = pfx.size_bits() + 1 + rem.size_bits();

    let hml_short_len = 2 + 2 * remaining_bits;
    let hml_long_len = 2 + bits_for_len + remaining_bits;
    let hml_same_len = 3 + bits_for_len;

    if hml_same_len < hml_long_len
        && hml_same_len < hml_short_len
        && (pfx.is_data_empty() || matches!(pfx.test_uniform(), Some(p) if p == bit))
        && (rem.is_data_empty() || matches!(rem.test_uniform(), Some(r) if r == bit))
    {
        return write_hml_same(bit, remaining_bits, bits_for_len, label);
    }

    if hml_short_len <= MAX_BIT_LEN && hml_short_len <= hml_long_len {
        ok!(write_hml_short_tag(remaining_bits, label));
    } else if hml_long_len <= MAX_BIT_LEN {
        ok!(write_hml_long_tag(remaining_bits, bits_for_len, label));
    } else {
        return Err(Error::InvalidData);
    }
    ok!(label.store_slice_data(pfx));
    ok!(label.store_bit(bit));
    label.store_slice_data(rem)
}

fn read_label<'a>(label: &mut CellSlice<'a>, key_bit_len: u16) -> Result<CellSlice<'a>, Error> {
    let bits_for_len = (16 - key_bit_len.leading_zeros()) as u16;

    if bits_for_len == 0 && label.is_data_empty() {
        Ok(label.get_prefix(0, 0))
    } else if !ok!(label.load_bit()) {
        read_hml_short(label)
    } else if !ok!(label.load_bit()) {
        read_hml_long(label, bits_for_len)
    } else {
        read_hml_same(label, bits_for_len)
    }
}

fn write_hml_empty(label: &mut CellBuilder) -> Result<(), Error> {
    label.store_zeros(2)
}

fn write_hml_short_tag(len: u16, label: &mut CellBuilder) -> Result<(), Error> {
    ok!(label.store_bit_zero());

    for _ in 0..len / 32 {
        ok!(label.store_u32(u32::MAX));
    }

    let rem = len % 32;
    if rem != 0 {
        ok!(label.store_uint(u64::MAX, rem));
    }
    label.store_bit_zero()
}

fn read_hml_short<'a>(label: &mut CellSlice<'a>) -> Result<CellSlice<'a>, Error> {
    let mut len = 0;
    while ok!(label.load_bit()) {
        len += 1;
    }
    let result = *label;
    ok!(label.skip_first(len, 0));
    Ok(result.get_prefix(len, 0))
}

fn write_hml_long_tag(len: u16, bits_for_len: u16, label: &mut CellBuilder) -> Result<(), Error> {
    ok!(label.store_bit_one());
    ok!(label.store_bit_zero());
    label.store_uint(len as u64, bits_for_len)
}

fn read_hml_long<'a>(label: &mut CellSlice<'a>, bits_for_len: u16) -> Result<CellSlice<'a>, Error> {
    let len = ok!(label.load_uint(bits_for_len)) as u16;
    let result = *label;
    ok!(label.skip_first(len, 0));
    Ok(result.get_prefix(len, 0))
}

fn write_hml_same(
    bit: bool,
    len: u16,
    bits_for_len: u16,
    label: &mut CellBuilder,
) -> Result<(), Error> {
    ok!(label.store_small_uint(0b110 | bit as u8, 3));
    label.store_uint(len as u64, bits_for_len)
}

fn read_hml_same<'a>(label: &mut CellSlice<'a>, bits_for_len: u16) -> Result<CellSlice<'a>, Error> {
    let cell = match ok!(label.load_bit()) {
        false => Cell::all_zeros_ref(),
        true => Cell::all_ones_ref(),
    };
    let len = ok!(label.load_uint(bits_for_len)) as u16;

    Ok(cell.as_slice_allow_exotic().get_prefix(len, 0))
}

/// Which branch to take when traversing the tree.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Branch {
    /// Branch for a key part that starts with bit 0
    Left = 0,
    /// Branch for a key part that starts with bit 1
    Right = 1,
}

impl Branch {
    /// Converts the branch to a boolean value.
    pub fn into_bit(self) -> bool {
        self == Self::Right
    }

    /// Returns the opposite branch.
    pub fn reversed(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
}

impl From<bool> for Branch {
    fn from(value: bool) -> Self {
        if value { Self::Right } else { Self::Left }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_cell<F: FnOnce(&mut CellBuilder) -> Result<(), Error>>(f: F) -> Cell {
        let mut builder = CellBuilder::new();
        f(&mut builder).unwrap();
        builder.build().unwrap()
    }

    #[test]
    fn labels() -> anyhow::Result<()> {
        let key_bit_len = 6;

        // Build key
        let key = {
            let mut builder = CellBuilder::new();
            builder.store_zeros(5)?;
            builder.store_bit_one()?;
            builder.build()?
        };

        // Build label
        let label = {
            let mut builder = CellBuilder::new();
            write_label(&key.as_slice()?, key_bit_len, &mut builder)?;
            builder.build().unwrap()
        };

        // Parse label
        let parsed_key = read_label(&mut label.as_slice()?, key_bit_len)?;
        let parsed_key = {
            let mut builder = CellBuilder::new();
            builder.store_slice(parsed_key)?;
            builder.build()?
        };

        // Parsed key should be equal to the original
        assert_eq!(key.as_ref(), parsed_key.as_ref());

        let label = CellBuilder::from_raw_data(&[0xcc, 0x40], 9)
            .unwrap()
            .build()
            .unwrap();
        let prefix = read_label(&mut label.as_slice()?, 32).unwrap();

        println!("{}", build_cell(|b| b.store_slice(prefix)).display_tree());
        assert_eq!(prefix.test_uniform(), Some(false));

        Ok(())
    }

    #[test]
    fn build_from_array() {
        for entries in [
            &[(0u32, 1u32), (1, 2), (2, 4), (2, 3), (3, 4), (4, 5)][..],
            &[
                (534837844, 3117028142),
                (1421713188, 3155891450),
                (1526242096, 2789399854),
                (1971086295, 1228713494),
                (4258889371, 3256452222),
            ],
        ] {
            let result =
                build_dict_from_sorted_iter(entries.iter().copied(), Cell::empty_context())
                    .unwrap();

            let mut dict = Dict::<u32, u32>::new();
            for (k, v) in entries {
                dict.add(k, v).unwrap();
            }

            println!("{}", result.as_ref().unwrap().display_tree());
            println!(
                "BOC: {}",
                crate::boc::BocRepr::encode_base64(&result).unwrap()
            );

            assert_eq!(result, dict.root);
        }
    }

    #[test]
    fn build_from_any_array() {
        for _ in 0..100 {
            let n = 1 + rand9::random::<u32>() % 1000;
            let mut entries = (0..n)
                .map(|_| (rand9::random::<u32>(), rand9::random::<u32>()))
                .collect::<Vec<_>>();
            entries.sort_by_key(|(k, _)| *k);

            let built_from_dict =
                build_dict_from_sorted_iter(entries.iter().copied(), Cell::empty_context())
                    .unwrap();

            let mut dict = Dict::<u32, u32>::new();
            for (k, v) in entries {
                dict.add(k, v).unwrap();
            }

            // println!("{}", built_from_dict.as_ref().unwrap().display_tree());

            assert_eq!(built_from_dict, dict.root);
        }
    }
}
