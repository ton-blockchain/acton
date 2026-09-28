use crate::cell::*;
use crate::dict::{
    AugDictFn, Branch, DictBound, DictOwnedEntry, Segment, read_label, rebuild_aug_dict_from_stack,
    rebuild_dict_from_stack, write_label_parts,
};
use crate::error::Error;

/// Removes the value associated with key in dictionary.
/// Returns a tuple with a new dictionary cell and an optional removed value.
pub fn dict_remove_owned(
    dict: &mut Option<Cell>,
    key: &mut CellSlice,
    key_bit_len: u16,
    allow_subtree: bool,
    context: &dyn CellContext,
) -> Result<Option<CellSliceParts>, Error> {
    if !allow_subtree && key.size_bits() != key_bit_len {
        return Err(Error::CellUnderflow);
    }

    let Some(root) = &dict else {
        return Ok(None);
    };

    let Some((mut stack, removed, prev_key_bit_len)) =
        ok!(dict_find_value_to_remove(root, key, context))
    else {
        return Ok(None);
    };

    // Rebuild the leaf node
    let value = if let Some(last) = stack.pop() {
        let (leaf, value) = ok!(last.rebuild_as_removed(key, prev_key_bit_len, context));
        *dict = Some(ok!(rebuild_dict_from_stack(stack, leaf, context)));
        value
    } else {
        let value = root.clone();
        *dict = None;
        value
    };

    Ok(Some((removed, value)))
}

/// Removes the value associated with key in aug dictionary.
/// Returns a tuple with a new dictionary cell and an optional removed value.
pub fn aug_dict_remove_owned(
    dict: &mut Option<Cell>,
    key: &mut CellSlice,
    key_bit_len: u16,
    allow_subtree: bool,
    comparator: AugDictFn,
    context: &dyn CellContext,
) -> Result<Option<CellSliceParts>, Error> {
    if !allow_subtree && key.size_bits() != key_bit_len {
        return Err(Error::CellUnderflow);
    }

    let Some(root) = &dict else {
        return Ok(None);
    };

    let Some((mut stack, removed, prev_key_bit_len)) =
        ok!(dict_find_value_to_remove(root, key, context))
    else {
        return Ok(None);
    };

    // Rebuild the leaf node
    let value = if let Some(last) = stack.pop() {
        let (leaf, value) = ok!(last.rebuild_as_removed(key, prev_key_bit_len, context));
        *dict = Some(ok!(rebuild_aug_dict_from_stack(
            stack, leaf, comparator, context,
        )));
        value
    } else {
        let value = root.clone();
        *dict = None;
        value
    };

    Ok(Some((removed, value)))
}

fn dict_find_value_to_remove<'a, 'c: 'a>(
    root: &'a Cell,
    key: &mut CellSlice,
    context: &'c dyn CellContext,
) -> Result<Option<(Vec<Segment<'a>>, CellSliceRange, u16)>, Error> {
    // TODO: change mode to `LoadMode::UseGas` if copy-on-write for libraries is not ok
    let mut data = ok!(context.load_dyn_cell(root.as_ref(), LoadMode::Full));

    let mut stack = Vec::<Segment>::new();

    let mut prev_key_bit_len = key.size_bits();
    let removed = loop {
        let mut remaining_data: CellSlice<'_> = ok!(data.as_slice());

        // Read the next part of the key from the current data
        let prefix = &mut ok!(read_label(&mut remaining_data, key.size_bits()));

        // Match the prefix with the key
        let lcp = key.longest_common_data_prefix(prefix);
        match lcp.size_bits().cmp(&key.size_bits()) {
            // If all bits match, an existing value was found
            std::cmp::Ordering::Equal => break remaining_data.range(),
            // LCP is less than prefix, an edge to slice was found
            std::cmp::Ordering::Less if lcp.size_bits() < prefix.size_bits() => {
                return Ok(None);
            }
            // The key contains the entire prefix, but there are still some bits left
            std::cmp::Ordering::Less => {
                // Fail fast if there are not enough references in the fork
                if data.reference_count() < 2 {
                    return Err(Error::CellUnderflow);
                }

                // Remove the LCP from the key
                prev_key_bit_len = key.size_bits();
                key.skip_first(lcp.size_bits(), 0).ok();

                // Load the next branch
                let next_branch = Branch::from(ok!(key.load_bit()));

                let child = match data.reference(next_branch as u8) {
                    // TODO: change mode to `LoadMode::UseGas` if copy-on-write for libraries is not ok
                    Some(child) => ok!(context.load_dyn_cell(child, LoadMode::Full)),
                    None => return Err(Error::CellUnderflow),
                };

                // Push an intermediate edge to the stack
                stack.push(Segment {
                    data,
                    next_branch,
                    key_bit_len: prev_key_bit_len,
                });
                data = child;
            }
            std::cmp::Ordering::Greater => {
                debug_assert!(false, "LCP of prefix and key can't be greater than key");
                unsafe { std::hint::unreachable_unchecked() };
            }
        }
    };

    Ok(Some((stack, removed, prev_key_bit_len)))
}

/// Removes the specified dict bound and returns a removed key and cell slice parts.
pub fn dict_remove_bound_owned(
    dict: &mut Option<Cell>,
    mut key_bit_len: u16,
    bound: DictBound,
    signed: bool,
    context: &dyn CellContext,
) -> Result<Option<DictOwnedEntry>, Error> {
    let root = match &dict {
        // TODO: change mode to `LoadMode::UseGas` if copy-on-write for libraries is not ok
        Some(data) => ok!(context.load_cell(data.clone(), LoadMode::Full)),
        None => return Ok(None),
    };
    let mut data = root.as_ref();

    let mut stack = Vec::<Segment>::new();

    let mut direction = None;
    let mut key = CellDataBuilder::new();

    // Try to find the required leaf
    let mut prev_key_bit_len = key_bit_len;
    let removed = loop {
        let mut remaining_data = ok!(data.as_slice());

        // Read the key part written in the current edge
        let prefix = &ok!(read_label(&mut remaining_data, key_bit_len));
        if !prefix.is_data_empty() {
            ok!(key.store_slice_data(prefix));
        }

        match key_bit_len.checked_sub(prefix.size_bits()) {
            Some(0) => break remaining_data.range(),
            Some(remaining) => {
                if remaining_data.size_refs() < 2 {
                    return Err(Error::CellUnderflow);
                }
                prev_key_bit_len = key_bit_len;
                key_bit_len = remaining - 1;
            }
            None => return Err(Error::CellUnderflow),
        };

        let next_branch = bound.update_direction(prefix, signed, &mut direction);
        ok!(key.store_bit(next_branch.into_bit()));

        let child = match data.reference(next_branch as u8) {
            // TODO: change mode to `LoadMode::UseGas` if copy-on-write for libraries is not ok
            Some(cell) => ok!(context.load_dyn_cell(cell, LoadMode::Full)),
            None => return Err(Error::CellUnderflow),
        };

        // Push an intermediate edge to the stack
        stack.push(Segment {
            data,
            next_branch,
            key_bit_len: 0,
        });
        data = child;
    };

    // Rebuild the leaf node
    let (leaf, removed) = if let Some(last) = stack.pop() {
        let index = last.next_branch as u8;

        // Load value branch
        let value = match last.data.reference_cloned(index) {
            // TODO: change mode to `LoadMode::Noop` if copy-on-write for libraries is not ok
            Some(cell) => ok!(context.load_cell(cell, LoadMode::Resolve)),
            None => return Err(Error::CellUnderflow),
        };

        // Load parent label
        let pfx = ok!(read_label(
            &mut last.data.as_slice_allow_exotic(),
            prev_key_bit_len
        ));

        // Load the opposite branch
        let mut opposite = match last.data.reference(1 - index) {
            // TODO: change mode to `LoadMode::UseGas` if copy-on-write for libraries is not ok
            Some(cell) => ok!(context
                .load_dyn_cell(cell, LoadMode::Full)
                .and_then(CellSlice::new)),
            None => return Err(Error::CellUnderflow),
        };
        let rem = ok!(read_label(&mut opposite, key_bit_len));

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

        // Return the new cell and the removed one
        (leaf, (removed, value))
    } else {
        *dict = None;
        return Ok(Some((key, (removed, root))));
    };

    *dict = Some(ok!(rebuild_dict_from_stack(stack, leaf, context)));
    Ok(Some((key, removed)))
}
