use std::marker::PhantomData;

use crate::cell::*;
use crate::dict::{
    AugDictFn, AugMergeStackItemMode, MergeStackItem, MergeStackItemMode, SimpleMergeStackItemMode,
    StoreDictKey, build_aug_dict_from_sorted_iter, build_dict_from_sorted_iter, read_label,
};
use crate::error::Error;

/// Modify a dict with a sorted list of inserts/removes.
// TODO: Better handle signed keys since their `Ord` implementation
// has a different ordering than their bit representation.
pub fn dict_modify_from_sorted_iter<K, V, T, I, FK, FV>(
    dict: &mut Option<Cell>,
    entries: I,
    mut extract_key: FK,
    mut extract_value: FV,
    context: &dyn CellContext,
) -> Result<bool, Error>
where
    I: IntoIterator<Item = T>,
    K: StoreDictKey + Ord,
    V: Store,
    for<'a> FK: FnMut(&'a T) -> K,
    FV: FnMut(T) -> Result<Option<V>, Error>,
{
    let Some(root) = &*dict else {
        // The simplest case when we just need to create a new dict from the sorted iter.
        let mut err = None;
        let res = build_dict_from_sorted_iter(
            entries.into_iter().flat_map(|entry| {
                if err.is_some() {
                    return None;
                }
                let key = extract_key(&entry);
                match extract_value(entry) {
                    Ok(Some(value)) => Some((key, value)),
                    Ok(None) => None,
                    Err(e) => {
                        err = Some(e);
                        None
                    }
                }
            }),
            context,
        );
        if let Some(e) = err {
            return Err(e);
        }
        *dict = ok!(res);
        return Ok(dict.is_some());
    };

    // Fallback to the full merge.
    let mut prev_key_builder = CellDataBuilder::new();

    let mut iter_stack = Vec::<IterStackItem>::new();
    let mut res_stack = Vec::<MergeStackItem<V, SimpleMergeStackItemMode>>::new();

    let mut prev_key = None::<K>;
    for item in entries {
        let is_first = prev_key.is_none();
        let key = extract_key(&item);
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

        // Build key.
        let mut key_builder = CellDataBuilder::new();
        ok!(key.store_into_data(&mut key_builder));
        debug_assert_eq!(key_builder.size_bits(), K::BITS);
        prev_key = Some(key);

        let key = key_builder.as_data_slice();

        let value = ok!(extract_value(item));
        let before_remove = value.is_none();

        // Update stack to contain a path to the `key`.
        let seek_state = if is_first {
            ok!(IterStackItem::seek_first(
                &mut iter_stack,
                root.clone(),
                key,
                before_remove,
                &mut res_stack,
                (),
                context,
            ))
        } else {
            ok!(IterStackItem::seek_next(
                &mut iter_stack,
                prev_key_builder.as_data_slice(),
                key,
                before_remove,
                &mut res_stack,
                (),
                context,
            ))
        };

        // Update the previous key builder.
        prev_key_builder.clear_bits();
        prev_key_builder.store_slice_data(key).unwrap();

        //
        let iter = iter_stack.last_mut().unwrap();
        match (seek_state, value) {
            // Add or remove value at fork.
            (SeekState::NotFound { remaining_key }, value) => {
                // Check if the new key is greater than the label in the current fork.
                if ok!(remaining_key.lex_cmp(&iter.label())).is_gt()
                    && iter.state != IterStackItem::STATE_USED_FULL
                {
                    ok!(iter.add_subtrees(1, false, &mut res_stack, (), context));
                    iter.state = IterStackItem::STATE_USED_FULL;
                }

                if let Some(value) = value {
                    ok!(MergeStackItem::add_value(
                        &mut res_stack,
                        key_builder,
                        value,
                        (),
                        (),
                        context,
                    ));
                }
            }
            // Add or remove value at existing.
            (SeekState::Found, value) => {
                debug_assert_eq!(iter.remaining_key_bits, 0);

                iter.state = IterStackItem::STATE_USED_FULL;
                if let Some(value) = value {
                    ok!(MergeStackItem::add_value(
                        &mut res_stack,
                        key_builder,
                        value,
                        (),
                        (),
                        context,
                    ));
                } else {
                    iter.flatten = true;
                }
            }
        }
    }

    if prev_key.is_none() {
        // Do nothing when the iterator was empty.
        return Ok(false);
    }

    ok!(IterStackItem::seek_end(
        &mut iter_stack,
        &mut res_stack,
        (),
        context,
    ));

    *dict = ok!(MergeStackItem::finalize(res_stack, K::BITS, (), context));
    Ok(true)
}

/// Modify a dict with a sorted list of inserts/removes.
// TODO: Better handle signed keys since their `Ord` implementation
// has a different ordering than their bit representation.
pub fn aug_dict_modify_from_sorted_iter<K, A, V, T, I, FK, FV>(
    dict: &mut Option<Cell>,
    entries: I,
    mut extract_key: FK,
    mut extract_value: FV,
    comparator: AugDictFn,
    context: &dyn CellContext,
) -> Result<bool, Error>
where
    I: IntoIterator<Item = T>,
    K: StoreDictKey + Ord,
    A: Store,
    V: Store,
    for<'a> FK: FnMut(&'a T) -> K,
    FV: FnMut(T) -> Result<Option<(A, V)>, Error>,
{
    let Some(root) = &*dict else {
        // The simplest case when we just need to create a new dict from the sorted iter.
        let mut err = None;
        let res = build_aug_dict_from_sorted_iter(
            entries.into_iter().flat_map(|entry| {
                if err.is_some() {
                    return None;
                }
                let key = extract_key(&entry);
                match extract_value(entry) {
                    Ok(Some((extra, value))) => Some((key, extra, value)),
                    Ok(None) => None,
                    Err(e) => {
                        err = Some(e);
                        None
                    }
                }
            }),
            comparator,
            context,
        );
        if let Some(e) = err {
            return Err(e);
        }
        *dict = ok!(res);
        return Ok(dict.is_some());
    };

    // Fallback to the full merge.
    let mut prev_key_builder = CellDataBuilder::new();

    let mut iter_stack = Vec::<IterStackItem>::new();
    let mut res_stack = Vec::<MergeStackItem<V, AugMergeStackItemMode<A>>>::new();

    let mut prev_key = None::<K>;
    for item in entries {
        let is_first = prev_key.is_none();
        let key = extract_key(&item);
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

        // Build key.
        let mut key_builder = CellDataBuilder::new();
        ok!(key.store_into_data(&mut key_builder));
        debug_assert_eq!(key_builder.size_bits(), K::BITS);
        prev_key = Some(key);

        let key = key_builder.as_data_slice();

        let value = ok!(extract_value(item));
        let before_remove = value.is_none();

        // Update stack to contain a path to the `key`.
        let seek_state = if is_first {
            ok!(IterStackItem::seek_first(
                &mut iter_stack,
                root.clone(),
                key,
                before_remove,
                &mut res_stack,
                comparator,
                context,
            ))
        } else {
            ok!(IterStackItem::seek_next(
                &mut iter_stack,
                prev_key_builder.as_data_slice(),
                key,
                before_remove,
                &mut res_stack,
                comparator,
                context,
            ))
        };

        // Update the previous key builder.
        prev_key_builder.clear_bits();
        prev_key_builder.store_slice_data(key).unwrap();

        //
        let iter = iter_stack.last_mut().unwrap();
        match (seek_state, value) {
            // Add or remove value at fork.
            (SeekState::NotFound { remaining_key }, value) => {
                // Check if the new key is greater than the label in the current fork.
                if ok!(remaining_key.lex_cmp(&iter.label())).is_gt()
                    && iter.state != IterStackItem::STATE_USED_FULL
                {
                    ok!(iter.add_subtrees(1, false, &mut res_stack, comparator, context));
                    iter.state = IterStackItem::STATE_USED_FULL;
                }

                if let Some((extra, value)) = value {
                    ok!(MergeStackItem::add_value(
                        &mut res_stack,
                        key_builder,
                        value,
                        extra,
                        comparator,
                        context,
                    ));
                }
            }
            // Add or remove value at existing.
            (SeekState::Found, value) => {
                debug_assert_eq!(iter.remaining_key_bits, 0);

                iter.state = IterStackItem::STATE_USED_FULL;
                if let Some((extra, value)) = value {
                    ok!(MergeStackItem::add_value(
                        &mut res_stack,
                        key_builder,
                        value,
                        extra,
                        comparator,
                        context,
                    ));
                } else {
                    iter.flatten = true;
                }
            }
        }
    }

    if prev_key.is_none() {
        // Do nothing when the iterator was empty.
        return Ok(false);
    }

    ok!(IterStackItem::seek_end(
        &mut iter_stack,
        &mut res_stack,
        comparator,
        context,
    ));

    *dict = ok!(MergeStackItem::finalize(
        res_stack,
        K::BITS,
        comparator,
        context
    ));
    Ok(true)
}

struct IterStackItem {
    /// Key bits before this node.
    prefix: CellDataBuilder,
    /// Original cell with this node.
    cell: Cell,
    /// Cell slice range of the label.
    label: Label,
    /// Item state:
    /// * `-1`: not processed yet;
    /// * `0`: left child was used;
    /// * `1`: right child was used;
    /// * `2`: fully processed.
    state: i8,
    /// Whether children of this node must be merged with this node.
    flatten: bool,
    /// Simplified cell slice range with only data offset.
    after_label_offset: u16,
    /// Number of bits after the label of this node (not including the split bit).
    remaining_key_bits: u16,
}

impl IterStackItem {
    const STATE_UNUSED: i8 = -1;
    const STATE_USED_FULL: i8 = 2;

    /// Creates a stack item from the dictionary subtree.
    ///
    /// * `prefix` must contain all key bits before this node.
    /// * `key_bit_len` is a number of remaining key bits
    ///   (i.e. `prefix.size_bits() + key_bit_len == full_key_bit_len`).
    fn load(prefix: CellDataBuilder, key_bit_len: u16, cell: Cell) -> Result<Self, Error> {
        let mut data = ok!(cell.as_slice());
        let label = ok!(read_label(&mut data, key_bit_len));
        Ok(Self {
            prefix,
            after_label_offset: data.offset_bits(),
            remaining_key_bits: key_bit_len - label.size_bits(),
            label: Label::new(cell.as_ref(), label),
            cell,
            state: Self::STATE_UNUSED,
            flatten: false,
        })
    }

    fn add_subtrees<V: Store, M: MergeStackItemMode>(
        &self,
        until: i8,
        flatten: bool,
        res_stack: &mut Vec<MergeStackItem<V, M>>,
        comparator: M::ExtraComparator,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        if self.remaining_key_bits == 0 {
            let mut prefix = self.prefix.clone();
            ok!(prefix.store_slice_data(self.label()));
            return MergeStackItem::add_raw_leaf(
                res_stack,
                prefix,
                self.cell.clone(),
                self.after_label_offset,
                comparator,
                context,
            );
        }

        let remaining_key_len = self.remaining_key_bits - 1;
        let label = self.label();

        for i in (self.state + 1)..=until {
            let Some(value) = self.cell.reference_cloned(i as u8) else {
                return Err(Error::CellUnderflow);
            };

            let mut prefix = self.prefix.clone();
            ok!(prefix.store_slice_data(label));
            ok!(prefix.store_bit(i != 0));

            if flatten || self.flatten {
                ok!(MergeStackItem::add_flattened_subtree(
                    res_stack,
                    prefix,
                    remaining_key_len,
                    value,
                    comparator,
                    context
                ));
            } else {
                ok!(MergeStackItem::add_subtree(
                    res_stack,
                    prefix,
                    remaining_key_len,
                    value,
                    comparator,
                    context
                ));
            }
        }
        Ok(())
    }

    fn label(&self) -> CellSlice<'_> {
        match self.label {
            Label::Inline(range) => range.apply_allow_exotic(&self.cell),
            Label::Same(label) => label,
        }
    }

    fn seek_first<'k, V: Store, M: MergeStackItemMode>(
        stack: &mut Vec<Self>,
        root: Cell,
        key: CellSlice<'k>,
        before_remove: bool,
        res_stack: &mut Vec<MergeStackItem<V, M>>,
        comparator: M::ExtraComparator,
        context: &dyn CellContext,
    ) -> Result<SeekState<'k>, Error> {
        debug_assert!(stack.is_empty());
        stack.push(ok!(IterStackItem::load(
            CellDataBuilder::new(),
            key.size_bits(),
            root
        )));
        Self::do_seek(stack, key, before_remove, res_stack, comparator, context)
    }

    fn seek_next<'k, V: Store, M: MergeStackItemMode>(
        stack: &mut Vec<Self>,
        prev_key: CellSlice<'_>,
        mut key: CellSlice<'k>,
        before_remove: bool,
        res_stack: &mut Vec<MergeStackItem<V, M>>,
        comparator: M::ExtraComparator,
        context: &dyn CellContext,
    ) -> Result<SeekState<'k>, Error> {
        debug_assert!(!stack.is_empty());

        // Fast check if the iterator was already finished.
        if let Some(last) = stack.last()
            && stack.len() == 1
            && last.state == Self::STATE_USED_FULL
        {
            return Ok(SeekState::NotFound { remaining_key: key });
        }

        // At this point `iter_stack` contains the path to the `prev_key`.
        let lcp = prev_key.longest_common_data_prefix(&key);
        let common_bits = lcp.size_bits();

        // Unwind iter_stack until it becomes a (partial) path to the `key`.
        let mut flatten = before_remove;
        while let Some(item) = stack.last_mut() {
            let prefix_len = item.prefix.size_bits();

            // Skip all segments further than the `lcp`.
            if prefix_len > common_bits {
                let item = stack.pop().unwrap();

                // If child item was flattened (due to remove) its parent
                // must also be flattened.
                flatten |= item.flatten;

                if item.state != Self::STATE_USED_FULL {
                    ok!(item.add_subtrees(1, flatten, res_stack, comparator, context));
                }
                continue;
            }

            // ... flatten only up to the first divergent node.
            item.flatten |= flatten;

            // At some point we must reach the segment which can contain the `key`.
            key.skip_first(prefix_len, 0)?;
            break;
        }

        // Seek for the rest.
        Self::do_seek(stack, key, before_remove, res_stack, comparator, context)
    }

    fn seek_end<V: Store, M: MergeStackItemMode>(
        stack: &mut Vec<Self>,
        res_stack: &mut Vec<MergeStackItem<V, M>>,
        comparator: M::ExtraComparator,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        let mut flatten = false;
        while let Some(item) = stack.pop() {
            // If child item was flattened (due to remove) its parent
            // must also be flattened.
            flatten |= item.flatten;
            if item.state != Self::STATE_USED_FULL {
                ok!(item.add_subtrees(1, flatten, res_stack, comparator, context));
            }
        }
        Ok(())
    }

    fn do_seek<'k, V: Store, M: MergeStackItemMode>(
        stack: &mut Vec<Self>,
        mut key: CellSlice<'k>,
        before_remove: bool,
        res_stack: &mut Vec<MergeStackItem<V, M>>,
        comparator: M::ExtraComparator,
        context: &dyn CellContext,
    ) -> Result<SeekState<'k>, Error> {
        loop {
            let item = stack.last_mut().unwrap();

            // Remove the prefix from the key.
            match key.strip_data_prefix(&item.label()) {
                Some(remaining_key) => {
                    if remaining_key.is_data_empty() {
                        // All key parts were collected <=> value found.
                        debug_assert_eq!(item.state, Self::STATE_UNUSED);
                        return Ok(SeekState::Found);
                    } else {
                        key = remaining_key;
                    }
                }
                None => return Ok(SeekState::NotFound { remaining_key: key }),
            }

            // Load next child based on the next bit.
            let child_index = ok!(key.load_bit()) as i8;
            let Some(child) = item.cell.reference_cloned(child_index as u8) else {
                return Err(Error::CellUnderflow);
            };

            // Add skipped subtrees.
            ok!(item.add_subtrees(
                child_index - 1,
                before_remove,
                res_stack,
                comparator,
                context,
            ));

            // Update the last visited segment state.
            debug_assert!(item.state < child_index);
            item.state = child_index;

            // Load the next child.
            let mut prefix = item.prefix.clone();
            ok!(prefix.store_slice_data(item.label()));
            ok!(prefix.store_bit(child_index != 0));
            let item = ok!(IterStackItem::load(prefix, key.size_bits(), child));

            // Add item to the iterator stack.
            stack.push(item);
        }
    }
}

enum SeekState<'k> {
    NotFound { remaining_key: CellSlice<'k> },
    Found,
}

impl<V: Store, M: MergeStackItemMode> MergeStackItem<V, M> {
    /// Adds a new item to the stack.
    ///
    /// Prefix must contain the full item key.
    fn add_value(
        stack: &mut Vec<Self>,
        key: CellDataBuilder,
        value: V,
        extra: M::LeafExtra,
        comparator: M::ExtraComparator,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        let lcp_len = ok!(Self::reduce(
            stack,
            key.as_data_slice(),
            key.size_bits(),
            comparator,
            context,
        ));

        stack.push(Self::Leaf {
            prefix: key,
            value,
            extra,
            prev_lcp_len: lcp_len,
            mode: PhantomData,
        });
        Ok(())
    }

    /// Adds a new serialized item to the stack.
    ///
    /// Prefix must contain the full item key.
    fn add_raw_leaf(
        stack: &mut Vec<Self>,
        prefix: CellDataBuilder,
        value: Cell,
        after_label_offset: u16,
        comparator: M::ExtraComparator,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        let lcp_len = ok!(Self::reduce(
            stack,
            prefix.as_data_slice(),
            prefix.size_bits(),
            comparator,
            context,
        ));

        stack.push(Self::DenormNode {
            prefix,
            cell: value,
            after_label_offset,
            is_leaf: true,
            prev_lcp_len: lcp_len,
            mode: PhantomData,
        });
        Ok(())
    }

    /// Adds a subtree item to the stack.
    /// Merges prefix with the subtree label.
    ///
    /// Prefix must contain all key bits before the subtree label.
    fn add_flattened_subtree(
        res_stack: &mut Vec<Self>,
        mut prefix: CellDataBuilder,
        key_bit_len: u16,
        value: Cell,
        comparator: M::ExtraComparator,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        let full_key_bit_len = prefix.size_bits() + key_bit_len;

        let mut data = ok!(value.as_slice());

        // Load label from the subtree root and append it to the prefix.
        let label = ok!(read_label(&mut data, key_bit_len));
        let is_leaf = label.size_bits() == key_bit_len;
        ok!(prefix.store_slice_data(label));

        let lcp_len = ok!(Self::reduce(
            res_stack,
            prefix.as_data_slice(),
            full_key_bit_len,
            comparator,
            context,
        ));

        res_stack.push(Self::DenormNode {
            prefix,
            after_label_offset: data.offset_bits(),
            cell: value,
            is_leaf,
            prev_lcp_len: lcp_len,
            mode: PhantomData,
        });
        Ok(())
    }

    /// Adds a subtree item to the stack as is.
    ///
    /// Prefix must contain all key bits before the subtree label.
    fn add_subtree(
        res_stack: &mut Vec<Self>,
        prefix: CellDataBuilder,
        key_bit_len: u16,
        value: Cell,
        comparator: M::ExtraComparator,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        let lcp_len = ok!(Self::reduce(
            res_stack,
            prefix.as_data_slice(),
            prefix.size_bits() + key_bit_len,
            comparator,
            context,
        ));

        let extra = ok!(M::read_node_extra(&value, key_bit_len));

        res_stack.push(Self::Node {
            prefix,
            value,
            extra,
            prev_lcp_len: lcp_len,
            mode: PhantomData,
        });
        Ok(())
    }
}

/// Owned version of the dict label.
enum Label {
    Inline(CellSliceRange),
    Same(CellSlice<'static>),
}

impl Label {
    fn new(cell: &'_ DynCell, label: CellSlice<'_>) -> Self {
        if std::ptr::addr_eq(label.cell(), cell) {
            Self::Inline(label.range())
        } else if let Ok(bit) = label.get_bit(0) {
            Self::Same(label.range().apply_allow_exotic(match bit {
                false => Cell::all_zeros_ref(),
                true => Cell::all_ones_ref(),
            }))
        } else {
            Self::Inline(CellSliceRange::empty())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boc::Boc;
    use crate::dict::Dict;

    #[test]
    fn mix_insert_remove() -> anyhow::Result<()> {
        let mut target = Dict::<u32, u32>::new();
        target.set(1, 1)?;
        target.set(65536, 65536)?;
        target.set(3145983, 3145983)?;
        target.set(755237119, 755237119)?;

        println!(
            "TARGET: {}",
            Boc::encode_base64(target.root.as_ref().unwrap())
        );

        for item in target.iter() {
            let (key, value) = item?;
            println!("== [{key}]: {value}");
        }

        let mut dict = Dict::<u32, u32>::new();
        dict.set(0, 0)?;
        dict.set(1, 1)?;
        dict.set(65536, 65536)?;
        dict.set(755237119, 755237119)?;

        println!(
            "INITIAL: {}",
            Boc::encode_base64(dict.root.as_ref().unwrap())
        );
        for item in dict.iter() {
            let (key, value) = item?;
            println!("== [{key}]: {value}");
        }

        dict_modify_from_sorted_iter(
            &mut dict.root,
            [(0u32, false), (3145983, true)],
            |(key, _)| *key,
            |(key, add)| Ok(add.then_some(key)),
            Cell::empty_context(),
        )?;

        println!(
            "RESULT: {}",
            Boc::encode_base64(dict.root.as_ref().unwrap())
        );
        for item in dict.iter() {
            let (key, value) = item?;
            println!("== [{key}]: {value}");
        }

        assert_eq!(dict.root, target.root);
        Ok(())
    }
}
