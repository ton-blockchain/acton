use std::marker::PhantomData;

use crate::cell::*;
use crate::dict::{AugDictFn, StoreDictKey, make_leaf, read_label, write_label};
use crate::error::Error;

/// Builds a new dictionary from an iterator of sorted unique entries.
///
/// It is a preferred way to build a large `Dict` as doing lots of
/// insertions is too slow.
// TODO: Better handle signed keys since their `Ord` implementation
// has a different ordering than their bit representation.
pub fn build_dict_from_sorted_iter<K, V, I>(
    entries: I,
    context: &dyn CellContext,
) -> Result<Option<Cell>, Error>
where
    I: IntoIterator<Item = (K, V)>,
    K: StoreDictKey + Ord,
    V: Store,
{
    let mut stack = Vec::<MergeStackItem<V, SimpleMergeStackItemMode>>::new();

    let mut prev_key = None::<K>;
    for (key, value) in entries {
        if let Some(prev_key) = &prev_key {
            match key.cmp(prev_key) {
                // Skip duplicates
                std::cmp::Ordering::Equal => continue,
                // Allow only sorted entries
                std::cmp::Ordering::Greater => {}
                // Invalid data. TODO: Panic here?
                std::cmp::Ordering::Less => return Err(Error::InvalidData),
            }
        }

        let mut prefix = CellDataBuilder::new();
        ok!(key.store_into_data(&mut prefix));
        debug_assert_eq!(prefix.size_bits(), K::BITS);
        prev_key = Some(key);

        let lcp_len = ok!(MergeStackItem::reduce(
            &mut stack,
            prefix.as_data_slice(),
            K::BITS,
            (),
            context,
        ));

        stack.push(MergeStackItem::Leaf {
            prefix,
            value,
            extra: (),
            prev_lcp_len: lcp_len,
            mode: PhantomData,
        });
    }

    MergeStackItem::finalize(stack, K::BITS, (), context)
}

/// Builds a new augmented dictionary from an iterator of sorted unique entries.
///
/// It is a preferred way to build a large `Dict` as doing lots of
/// insertions is too slow.
// TODO: Better handle signed keys since their `Ord` implementation
// has a different ordering than their bit representation.
pub fn build_aug_dict_from_sorted_iter<K, A, V, I>(
    entries: I,
    comparator: AugDictFn,
    context: &dyn CellContext,
) -> Result<Option<Cell>, Error>
where
    I: IntoIterator<Item = (K, A, V)>,
    K: StoreDictKey + Ord,
    A: Store,
    V: Store,
{
    let mut stack = Vec::<MergeStackItem<V, AugMergeStackItemMode<A>>>::new();

    let mut prev_key = None::<K>;
    for (key, extra, value) in entries {
        if let Some(prev_key) = &prev_key {
            match key.cmp(prev_key) {
                // Skip duplicates
                std::cmp::Ordering::Equal => continue,
                // Allow only sorted entries
                std::cmp::Ordering::Greater => {}
                // Invalid data. TODO: Panic here?
                std::cmp::Ordering::Less => return Err(Error::InvalidData),
            }
        }

        let mut prefix = CellDataBuilder::new();
        ok!(key.store_into_data(&mut prefix));
        debug_assert_eq!(prefix.size_bits(), K::BITS);
        prev_key = Some(key);

        let lcp_len = ok!(MergeStackItem::reduce(
            &mut stack,
            prefix.as_data_slice(),
            K::BITS,
            comparator,
            context
        ));

        stack.push(MergeStackItem::Leaf {
            prefix,
            extra,
            value,
            prev_lcp_len: lcp_len,
            mode: PhantomData,
        });
    }

    MergeStackItem::finalize(stack, K::BITS, comparator, context)
}

pub(crate) trait MergeStackItemMode {
    type ExtraComparator: Copy;
    type ExtraOffset;

    type LeafExtra;
    type NodeExtra;

    fn write_leaf_extra(
        leaf_extra: &Self::LeafExtra,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error>;

    fn read_node_extra(cell: &Cell, key_bit_len: u16) -> Result<Self::NodeExtra, Error>;

    fn leaf_extra_offset(builder: &CellBuilder) -> Self::ExtraOffset;
    fn node_extra_offset(node_extra: Self::NodeExtra) -> Self::ExtraOffset;
    fn denorm_node_extra_offset(builder: &CellBuilder, is_leaf: bool) -> Self::ExtraOffset;

    fn merge_node(
        left_cell: Cell,
        right_cell: Cell,
        left_extra_offset: Self::ExtraOffset,
        right_extra_offset: Self::ExtraOffset,
        comparator: Self::ExtraComparator,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<Self::NodeExtra, Error>;
}

pub(crate) struct SimpleMergeStackItemMode;

impl MergeStackItemMode for SimpleMergeStackItemMode {
    type ExtraComparator = ();
    type ExtraOffset = ();

    type LeafExtra = ();
    type NodeExtra = ();

    #[inline]
    fn write_leaf_extra(
        _: &Self::LeafExtra,
        _: &mut CellBuilder,
        _: &dyn CellContext,
    ) -> Result<(), Error> {
        Ok(())
    }

    #[inline]
    fn read_node_extra(_: &Cell, _: u16) -> Result<Self::NodeExtra, Error> {
        Ok(())
    }

    #[inline]
    fn leaf_extra_offset(_: &CellBuilder) -> Self::ExtraOffset {}
    #[inline]
    fn node_extra_offset(_: Self::NodeExtra) -> Self::ExtraOffset {}
    #[inline]
    fn denorm_node_extra_offset(_: &CellBuilder, _: bool) -> Self::ExtraOffset {}

    #[inline]
    fn merge_node(
        left_cell: Cell,
        right_cell: Cell,
        _: Self::ExtraOffset,
        _: Self::ExtraOffset,
        _: Self::ExtraComparator,
        builder: &mut CellBuilder,
        _: &dyn CellContext,
    ) -> Result<(), Error> {
        ok!(builder.store_reference(left_cell));
        builder.store_reference(right_cell)
    }
}

pub(crate) struct AugMergeStackItemMode<A>(PhantomData<A>);

impl<A> MergeStackItemMode for AugMergeStackItemMode<A>
where
    A: Store,
{
    type ExtraComparator = AugDictFn;
    type ExtraOffset = (u16, u8);

    type LeafExtra = A;
    type NodeExtra = (u16, bool);

    #[inline]
    fn write_leaf_extra(
        leaf_extra: &Self::LeafExtra,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        leaf_extra.store_into(builder, context)
    }

    #[inline]
    fn read_node_extra(cell: &Cell, key_bit_len: u16) -> Result<Self::NodeExtra, Error> {
        let mut slice = ok!(cell.as_slice());
        let label = ok!(read_label(&mut slice, key_bit_len));
        let is_leaf = label.size_bits() == key_bit_len;
        Ok((slice.offset_bits(), is_leaf))
    }

    #[inline]
    fn leaf_extra_offset(builder: &CellBuilder) -> Self::ExtraOffset {
        (builder.size_bits(), 0)
    }

    #[inline]
    fn node_extra_offset((extra_bits_offset, is_leaf): Self::NodeExtra) -> Self::ExtraOffset {
        (extra_bits_offset, if is_leaf { 0 } else { 2 })
    }

    #[inline]
    fn denorm_node_extra_offset(builder: &CellBuilder, is_leaf: bool) -> Self::ExtraOffset {
        (builder.size_bits(), if is_leaf { 0 } else { 2 })
    }

    #[inline]
    fn merge_node(
        left_cell: Cell,
        right_cell: Cell,
        left_extra_offset: Self::ExtraOffset,
        right_extra_offset: Self::ExtraOffset,
        comparator: Self::ExtraComparator,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<Self::NodeExtra, Error> {
        ok!(builder.store_reference(left_cell.clone()));
        ok!(builder.store_reference(right_cell.clone()));

        let extra_offset = builder.size_bits();

        let mut left_extra_slice = ok!(left_cell.as_slice());
        ok!(left_extra_slice.skip_first(left_extra_offset.0, left_extra_offset.1));

        let mut right_extra_slice = ok!(right_cell.as_slice());
        ok!(right_extra_slice.skip_first(right_extra_offset.0, right_extra_offset.1));

        ok!(comparator(
            &mut left_extra_slice,
            &mut right_extra_slice,
            builder,
            context
        ));

        Ok((extra_offset, false))
    }
}

pub(crate) enum MergeStackItem<V, M: MergeStackItemMode> {
    /// Leaf value to be serialized.
    Leaf {
        prefix: CellDataBuilder,
        value: V,
        extra: M::LeafExtra,
        prev_lcp_len: u16,
        mode: PhantomData<M>,
    },
    /// Complete node which can be used as is.
    Node {
        prefix: CellDataBuilder,
        value: Cell,
        extra: M::NodeExtra,
        prev_lcp_len: u16,
        mode: PhantomData<M>,
    },
    /// A subtree with a prefix.
    /// Used when we need to use the sibling of the removed branch.
    DenormNode {
        prefix: CellDataBuilder,
        cell: Cell,
        after_label_offset: u16,
        is_leaf: bool,
        prev_lcp_len: u16,
        mode: PhantomData<M>,
    },
}

impl<V: Store, M: MergeStackItemMode> MergeStackItem<V, M> {
    pub fn reduce(
        res_stack: &mut Vec<Self>,
        key: CellSlice<'_>,
        key_bit_len: u16,
        comparator: M::ExtraComparator,
        context: &dyn CellContext,
    ) -> Result<u16, Error> {
        let mut lcp_len = 0;
        if let Some(last) = res_stack.last() {
            let left_prefix = last.prefix_slice();
            let right_prefix = key;

            let lcp = left_prefix.longest_common_data_prefix(&right_prefix);
            lcp_len = lcp.size_bits();

            if lcp_len <= last.prev_lcp_len() {
                // LCP decreased, we are not able to group the current item to the last one.
                // At this point we can safely reduce the stack.
                let mut right = res_stack.pop().unwrap();
                while !res_stack.is_empty() {
                    if right.prev_lcp_len() <= lcp_len {
                        break;
                    }
                    let left = res_stack.pop().unwrap();
                    let key_offset = if res_stack.is_empty() {
                        1
                    } else {
                        left.prev_lcp_len() + 1
                    }
                    .max(lcp_len + 1);

                    right = ok!(left.merge(right, key_offset, key_bit_len, comparator, context));
                }
                res_stack.push(right);
            }
        }

        Ok(lcp_len)
    }

    pub fn prefix_slice(&self) -> CellSlice<'_> {
        match self {
            Self::Leaf { prefix, .. } => prefix.as_data_slice(),
            Self::Node { prefix, .. } => prefix.as_data_slice(),
            Self::DenormNode { prefix, .. } => prefix.as_data_slice(),
        }
    }

    pub fn prev_lcp_len(&self) -> u16 {
        match self {
            Self::Leaf { prev_lcp_len, .. } => *prev_lcp_len,
            Self::Node { prev_lcp_len, .. } => *prev_lcp_len,
            Self::DenormNode { prev_lcp_len, .. } => *prev_lcp_len,
        }
    }

    pub fn build(self, key_bit_len: u16, context: &dyn CellContext) -> Result<Cell, Error> {
        match self {
            Self::Leaf {
                prefix,
                value,
                extra,
                ..
            } => {
                let mut b = CellBuilder::new();
                ok!(write_label(&prefix.as_data_slice(), key_bit_len, &mut b));
                ok!(M::write_leaf_extra(&extra, &mut b, context));
                ok!(value.store_into(&mut b, context));
                b.build_ext(context)
            }
            Self::Node { value, .. } => Ok(value),
            Self::DenormNode {
                prefix,
                cell,
                after_label_offset,
                ..
            } => {
                let mut data = ok!(cell.as_slice());
                ok!(data.skip_first(after_label_offset, 0));
                make_leaf(&prefix.as_data_slice(), key_bit_len, &data, context)
            }
        }
    }

    pub fn merge(
        self,
        right: Self,
        key_offset: u16,
        key_bit_len: u16,
        comparator: M::ExtraComparator,
        context: &dyn CellContext,
    ) -> Result<Self, Error> {
        let left_offset = self.prev_lcp_len();
        let split_at = right.prev_lcp_len();

        let label_len = split_at - key_offset;
        let leaf_key_bit_len = key_bit_len - split_at - 1;
        let key_bit_len = key_bit_len - key_offset;

        let mut builder = CellBuilder::new();

        // Build the right cell and write the common prefix
        let right_extra_offset;
        let result_prefix;
        let right_cell = match right {
            Self::Leaf {
                prefix,
                value,
                extra,
                ..
            } => {
                let mut prefix_slice = prefix.as_data_slice();

                // Write the common prefix as a label
                let mut common_prefix = prefix_slice;
                ok!(common_prefix.skip_first(key_offset, 0)); // TODO: Unwrap
                ok!(write_label(
                    &common_prefix.get_prefix(label_len, 0),
                    key_bit_len,
                    &mut builder,
                ));

                // Build leaf from value
                ok!(prefix_slice.skip_first(split_at + 1, 0)); // TODO: Unwrap
                let value = {
                    let mut builder = CellBuilder::new();
                    ok!(write_label(&prefix_slice, leaf_key_bit_len, &mut builder));
                    right_extra_offset = M::leaf_extra_offset(&builder);
                    ok!(M::write_leaf_extra(&extra, &mut builder, context));
                    ok!(value.store_into(&mut builder, context));
                    ok!(builder.build_ext(context))
                };

                result_prefix = prefix;
                value
            }
            Self::Node {
                prefix,
                value,
                extra,
                ..
            } => {
                // Write the common prefix as a label
                let mut common_prefix = prefix.as_data_slice();
                ok!(common_prefix.skip_first(key_offset, 0)); // TODO: Unwrap
                ok!(write_label(
                    &common_prefix.get_prefix(label_len, 0),
                    key_bit_len,
                    &mut builder,
                ));

                right_extra_offset = M::node_extra_offset(extra);

                // Use value as is
                result_prefix = prefix;
                value
            }
            Self::DenormNode {
                prefix,
                cell,
                after_label_offset,
                is_leaf,
                ..
            } => {
                let mut prefix_slice = prefix.as_data_slice();

                // Write the common prefix as a label
                let mut common_prefix = prefix_slice;
                ok!(common_prefix.skip_first(key_offset, 0)); // TODO: Unwrap
                ok!(write_label(
                    &common_prefix.get_prefix(label_len, 0),
                    key_bit_len,
                    &mut builder,
                ));

                // Build leaf from value
                ok!(prefix_slice.skip_first(split_at + 1, 0)); // TODO: Unwrap
                let value = {
                    let mut builder = CellBuilder::new();
                    ok!(write_label(&prefix_slice, leaf_key_bit_len, &mut builder));
                    right_extra_offset = M::denorm_node_extra_offset(&builder, is_leaf);

                    let mut data = ok!(cell.as_slice());
                    ok!(data.skip_first(after_label_offset, 0));
                    ok!(builder.store_slice(data));
                    ok!(builder.build_ext(context))
                };

                result_prefix = prefix;
                value
            }
        };

        // Build the left cell
        let left_extra_offset;
        let left_cell = match self {
            Self::Leaf {
                prefix,
                value,
                extra,
                ..
            } => {
                let mut prefix_slice = prefix.as_data_slice();
                ok!(prefix_slice.skip_first(split_at + 1, 0)); // TODO: Unwrap

                let mut builder = CellBuilder::new();
                ok!(write_label(&prefix_slice, leaf_key_bit_len, &mut builder));
                left_extra_offset = M::leaf_extra_offset(&builder);
                ok!(M::write_leaf_extra(&extra, &mut builder, context));
                ok!(value.store_into(&mut builder, context));
                ok!(builder.build_ext(context))
            }
            Self::Node { value, extra, .. } => {
                left_extra_offset = M::node_extra_offset(extra);
                value
            }
            Self::DenormNode {
                prefix,
                cell,
                after_label_offset,
                is_leaf,
                ..
            } => {
                let mut prefix_slice = prefix.as_data_slice();
                ok!(prefix_slice.skip_first(split_at + 1, 0)); // TODO: Unwrap

                let mut builder = CellBuilder::new();
                ok!(write_label(&prefix_slice, leaf_key_bit_len, &mut builder));
                left_extra_offset = M::denorm_node_extra_offset(&builder, is_leaf);

                let mut data = ok!(cell.as_slice());
                ok!(data.skip_first(after_label_offset, 0));
                ok!(builder.store_slice(data));
                ok!(builder.build_ext(context))
            }
        };

        // Complete the node
        let extra = ok!(M::merge_node(
            left_cell,
            right_cell,
            left_extra_offset,
            right_extra_offset,
            comparator,
            &mut builder,
            context
        ));

        // Wrap into a new stack item
        Ok(MergeStackItem::Node {
            prefix: result_prefix,
            value: ok!(builder.build_ext(context)),
            extra,
            prev_lcp_len: left_offset,
            mode: PhantomData,
        })
    }

    pub fn finalize(
        mut stack: Vec<Self>,
        key_bit_len: u16,
        comparator: M::ExtraComparator,
        context: &dyn CellContext,
    ) -> Result<Option<Cell>, Error> {
        let Some(mut result) = stack.pop() else {
            return Ok(None);
        };

        while let Some(left) = stack.pop() {
            let key_offset = if stack.is_empty() {
                0
            } else {
                left.prev_lcp_len() + 1
            };

            result = ok!(left.merge(result, key_offset, key_bit_len, comparator, context));
        }

        result.build(key_bit_len, context).map(Some)
    }
}
