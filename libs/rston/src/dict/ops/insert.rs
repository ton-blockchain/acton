use crate::cell::*;
use crate::dict::{
    AugDictFn, Branch, Segment, SetMode, make_leaf, make_leaf_with_extra, read_label,
    rebuild_aug_dict_from_stack, rebuild_dict_from_stack, split_aug_edge, split_edge,
};
use crate::error::Error;

/// Inserts the value associated with key in dictionary
/// in accordance with the logic of the specified [`SetMode`].
pub fn dict_insert(
    dict: &mut Option<Cell>,
    key: &mut CellSlice,
    key_bit_len: u16,
    value: &dyn Store,
    mode: SetMode,
    context: &dyn CellContext,
) -> Result<bool, Error> {
    if key.size_bits() != key_bit_len {
        return Err(Error::CellUnderflow);
    }

    let mut data = match dict.as_ref() {
        // TODO: change mode to `LoadMode::UseGas` if copy-on-write for libraries is not ok.
        Some(data) => ok!(context.load_dyn_cell(data.as_ref(), LoadMode::Full)),
        None if mode.can_add() => {
            *dict = Some(ok!(make_leaf(key, key_bit_len, value, context)));
            return Ok(true);
        }
        None => return Ok(false),
    };

    let mut stack = Vec::<Segment>::new();

    let leaf = loop {
        let mut remaining_data = ok!(data.as_slice());

        // Read the next part of the key from the current data
        let prefix = &mut ok!(read_label(&mut remaining_data, key.size_bits()));

        // Match the prefix with the key
        let lcp = key.longest_common_data_prefix(prefix);
        match lcp.size_bits().cmp(&key.size_bits()) {
            // If all bits match, an existing value was found
            std::cmp::Ordering::Equal => {
                // Check if we can replace the value
                if !mode.can_replace() {
                    // TODO: what is the desired behavior for root as a library?
                    return Ok(false);
                }
                // Replace the existing value
                break ok!(make_leaf(prefix, key.size_bits(), value, context));
            }
            // LCP is less than prefix, an edge to slice was found
            std::cmp::Ordering::Less if lcp.size_bits() < prefix.size_bits() => {
                // Check if we can add a new value
                if !mode.can_add() {
                    // TODO: what is the desired behavior for root as a library?
                    return Ok(false);
                }
                break ok!(split_edge(
                    &remaining_data,
                    prefix,
                    &lcp,
                    key,
                    value,
                    context
                ));
            }
            // The key contains the entire prefix, but there are still some bits left
            std::cmp::Ordering::Less => {
                // Fail fast if there are not enough references in the fork
                if data.reference_count() < 2 {
                    return Err(Error::CellUnderflow);
                }

                // Remove the LCP from the key
                let key_bit_len = key.size_bits();
                key.skip_first(lcp.size_bits(), 0).ok();

                // Load the next branch
                let next_branch = match key.load_bit() {
                    Ok(bit) => Branch::from(bit),
                    Err(e) => return Err(e),
                };

                let child = match data.reference(next_branch as u8) {
                    // TODO: change mode to `LoadMode::UseGas` if copy-on-write for libraries is not ok
                    Some(cell) => ok!(context.load_dyn_cell(cell, LoadMode::Full)),
                    None => return Err(Error::CellUnderflow),
                };

                // Push an intermediate edge to the stack
                stack.push(Segment {
                    data,
                    next_branch,
                    key_bit_len,
                });
                data = child;
            }
            std::cmp::Ordering::Greater => {
                debug_assert!(false, "LCP of prefix and key can't be greater than key");
                unsafe { std::hint::unreachable_unchecked() };
            }
        }
    };

    *dict = Some(ok!(rebuild_dict_from_stack(stack, leaf, context)));
    Ok(true)
}

/// Inserts the value associated with key in aug dictionary
/// in accordance with the logic of the specified [`SetMode`] and comparator for extra
#[allow(clippy::too_many_arguments)]
pub fn aug_dict_insert(
    dict: &mut Option<Cell>,
    key: &mut CellSlice,
    key_bit_len: u16,
    extra: &dyn Store,
    value: &dyn Store,
    mode: SetMode,
    comparator: AugDictFn,
    context: &dyn CellContext,
) -> Result<bool, Error> {
    if key.size_bits() != key_bit_len {
        return Err(Error::CellUnderflow);
    }

    let mut data = match dict.as_ref() {
        // TODO: change mode to `LoadMode::UseGas` if copy-on-write for libraries is not ok.
        Some(data) => {
            ok!(context.load_dyn_cell(data.as_ref(), LoadMode::Full))
        }
        None if mode.can_add() => {
            let cell = ok!(make_leaf_with_extra(
                key,
                key_bit_len,
                extra,
                value,
                context
            ));
            *dict = Some(cell);
            return Ok(true);
        }
        None => return Ok(false),
    };

    let mut stack = Vec::<Segment>::new();

    let leaf = loop {
        let mut remaining_data = ok!(data.as_slice());
        // Read the next part of the key from the current data
        let prefix = &mut ok!(read_label(&mut remaining_data, key.size_bits()));
        // Match the prefix with the key
        let lcp = key.longest_common_data_prefix(prefix);
        match lcp.size_bits().cmp(&key.size_bits()) {
            // If all bits match, an existing value was found
            std::cmp::Ordering::Equal => {
                // Check if we can replace the value
                if !mode.can_replace() {
                    // TODO: what is the desired behavior for root as a library?
                    return Ok(false);
                }
                // Replace the existing value
                break ok!(make_leaf_with_extra(
                    prefix,
                    key.size_bits(),
                    extra,
                    value,
                    context
                ));
            }
            // LCP is less than prefix, an edge to slice was found
            std::cmp::Ordering::Less if lcp.size_bits() < prefix.size_bits() => {
                // Check if we can add a new value
                if !mode.can_add() {
                    // TODO: what is the desired behavior for root as a library?
                    return Ok(false);
                }
                break ok!(split_aug_edge(
                    &mut remaining_data,
                    prefix,
                    &lcp,
                    key,
                    extra,
                    value,
                    comparator,
                    context,
                ));
            }
            // The key contains the entire prefix, but there are still some bits left
            std::cmp::Ordering::Less => {
                // Fail fast if there are not enough references in the fork
                if data.reference_count() < 2 {
                    return Err(Error::CellUnderflow);
                }

                // Remove the LCP from the key
                let key_bit_len = key.size_bits();
                key.skip_first(lcp.size_bits(), 0).ok();
                // Load the next branch
                let next_branch = match key.load_bit() {
                    Ok(bit) => Branch::from(bit),
                    Err(e) => return Err(e),
                };
                let child = match data.reference(next_branch as u8) {
                    // TODO: change mode to `LoadMode::UseGas` if copy-on-write for libraries is not ok
                    Some(cell) => ok!(context.load_dyn_cell(cell, LoadMode::Full)),
                    None => return Err(Error::CellUnderflow),
                };

                // Push an intermediate edge to the stack
                stack.push(Segment {
                    data,
                    next_branch,
                    key_bit_len,
                });
                data = child;
            }
            std::cmp::Ordering::Greater => {
                debug_assert!(false, "LCP of prefix and key can't be greater than key");
                unsafe { std::hint::unreachable_unchecked() };
            }
        }
    };

    *dict = Some(ok!(rebuild_aug_dict_from_stack(
        stack, leaf, comparator, context,
    )));

    Ok(true)
}

/// Inserts the value associated with key in dictionary
/// in accordance with the logic of the specified [`SetMode`].
///
/// Returns a tuple with a new dict root, changed flag and the previous value.
pub fn dict_insert_owned(
    dict: &mut Option<Cell>,
    key: &mut CellSlice,
    key_bit_len: u16,
    value: &dyn Store,
    mode: SetMode,
    context: &dyn CellContext,
) -> Result<(bool, Option<CellSliceParts>), Error> {
    fn last(
        stack: &[Segment],
        context: &dyn CellContext,
        mode: LoadMode,
    ) -> Result<Option<Cell>, Error> {
        match stack.last() {
            Some(Segment {
                data, next_branch, ..
            }) => match data.reference_cloned(*next_branch as u8) {
                Some(cell) => context.load_cell(cell, mode).map(Some),
                None => Err(Error::CellUnderflow),
            },
            None => Ok(None),
        }
    }

    if key.size_bits() != key_bit_len {
        return Err(Error::CellUnderflow);
    }

    let root = match &dict {
        // TODO: change mode to `LoadMode::UseGas` if copy-on-write for libraries is not ok.
        Some(data) => ok!(context.load_cell(data.clone(), LoadMode::Full)),
        None if mode.can_add() => {
            *dict = Some(ok!(make_leaf(key, key_bit_len, value, context)));
            return Ok((true, None));
        }
        None => return Ok((false, None)),
    };
    let mut data = root.as_ref();
    let mut stack = Vec::<Segment>::new();

    let (mut leaf, value_range) = loop {
        let mut remaining_data = ok!(data.as_slice());

        // Read the next part of the key from the current data
        let prefix = &mut ok!(read_label(&mut remaining_data, key.size_bits()));

        // Match the prefix with the key
        let lcp = key.longest_common_data_prefix(prefix);
        match lcp.size_bits().cmp(&key.size_bits()) {
            // If all bits match, an existing value was found
            std::cmp::Ordering::Equal => {
                // Check if we can replace the value
                if !mode.can_replace() {
                    // TODO: change mode to `LoadMode::Noop` if copy-on-write for libraries is not ok.
                    let value = (
                        remaining_data.range(),
                        ok!(last(&stack, context, LoadMode::Resolve))
                            .unwrap_or_else(|| root.clone()),
                    );
                    // TODO: what is the desired behavior for root as a library?
                    return Ok((false, Some(value)));
                }
                // Replace the existing value
                break (
                    ok!(make_leaf(prefix, key.size_bits(), value, context)),
                    Some(remaining_data.range()),
                );
            }
            // LCP is less than prefix, an edge to slice was found
            std::cmp::Ordering::Less if lcp.size_bits() < prefix.size_bits() => {
                // Check if we can add a new value
                if !mode.can_add() {
                    // TODO: what is the desired behavior for root as a library?
                    return Ok((false, None));
                }
                break (
                    ok!(split_edge(
                        &remaining_data,
                        prefix,
                        &lcp,
                        key,
                        value,
                        context
                    )),
                    None,
                );
            }
            // The key contains the entire prefix, but there are still some bits left
            std::cmp::Ordering::Less => {
                // Fail fast if there are not enough references in the fork
                if data.reference_count() < 2 {
                    return Err(Error::CellUnderflow);
                }

                // Remove the LCP from the key
                let key_bit_len = key.size_bits();
                key.skip_first(lcp.size_bits(), 0).ok();

                // Load the next branch
                let next_branch = match key.load_bit() {
                    Ok(bit) => Branch::from(bit),
                    Err(e) => return Err(e),
                };

                let child = match data.reference(next_branch as u8) {
                    // TODO: change mode to `LoadMode::UseGas` if copy-on-write for libraries is not ok
                    Some(cell) => ok!(context.load_dyn_cell(cell, LoadMode::Full)),
                    None => return Err(Error::CellUnderflow),
                };

                // Push an intermediate edge to the stack
                stack.push(Segment {
                    data,
                    next_branch,
                    key_bit_len,
                });
                data = child;
            }
            std::cmp::Ordering::Greater => {
                debug_assert!(false, "LCP of prefix and key can't be greater than key");
                unsafe { std::hint::unreachable_unchecked() };
            }
        }
    };

    let value = match value_range {
        Some(range) => {
            // TODO: change mode to `LoadMode::Noop` if copy-on-write for libraries is not ok
            let last = ok!(last(&stack, context, LoadMode::Resolve));
            leaf = ok!(rebuild_dict_from_stack(stack, leaf, context));
            Some((range, last.unwrap_or(root)))
        }
        None => {
            leaf = ok!(rebuild_dict_from_stack(stack, leaf, context));
            None
        }
    };

    *dict = Some(leaf);
    Ok((true, value))
}
