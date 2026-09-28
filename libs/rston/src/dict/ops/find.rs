use std::ops::ControlFlow;

use crate::cell::*;
use crate::dict::{Branch, DictBound, DictOwnedEntry, SearchByExtra, Segment, read_label};
use crate::error::Error;

/// Returns cell slice parts of the value corresponding to the key.
pub fn dict_find_owned(
    dict: Option<&Cell>,
    key_bit_len: u16,
    mut key: CellSlice<'_>,
    towards: DictBound,
    inclusive: bool,
    signed: bool,
    context: &dyn CellContext,
) -> Result<Option<DictOwnedEntry>, Error> {
    if key.size_bits() != key_bit_len {
        return Err(Error::CellUnderflow);
    }

    enum Leaf {
        Value(CellSliceRange),
        Divergence(Branch),
    }

    let root = match dict {
        Some(data) => ok!(context.load_cell(data.clone(), LoadMode::Full)),
        None => return Ok(None),
    };

    let mut original_key_range = key.range();
    let mut result_key = CellDataBuilder::new();

    let mut data = root.as_ref();
    let mut stack = Vec::<Segment>::new();
    let mut prev = None;

    // Try to find the required leaf
    let value_range = loop {
        let mut remaining_data = ok!(data.as_slice());

        // Read the next part of the key from the current data
        let prefix = &mut ok!(read_label(&mut remaining_data, key.size_bits()));

        // Match the prefix with the key
        let lcp = key.longest_common_data_prefix(prefix);
        let lcp_len = lcp.size_bits();
        match lcp_len.cmp(&key.size_bits()) {
            // If all bits match, an existing value was found
            std::cmp::Ordering::Equal => break Leaf::Value(remaining_data.range()),
            // LCP is less than prefix, an edge to slice was found
            std::cmp::Ordering::Less => {
                // LCP is less than prefix, an edge to slice was found
                if lcp_len < prefix.size_bits() {
                    let mut next_branch = Branch::from(ok!(key.get_bit(lcp_len)));
                    if signed && stack.is_empty() && lcp_len == 0 {
                        next_branch = next_branch.reversed();
                    }

                    break Leaf::Divergence(next_branch);
                }

                // The key contains the entire prefix, but there are still some bits left.
                // Fail fast if there are not enough references in the fork
                if data.reference_count() < 2 {
                    return Err(Error::CellUnderflow);
                }

                // Remove the LCP from the key
                key.skip_first(lcp.size_bits(), 0).ok();

                // Load the next branch
                let next_branch = Branch::from(ok!(key.load_bit()));

                let child = match data.reference(next_branch as u8) {
                    Some(cell) => ok!(context.load_dyn_cell(cell, LoadMode::Full)),
                    None => return Err(Error::CellUnderflow),
                };

                // Push an intermediate edge to the stack
                stack.push(Segment {
                    data,
                    next_branch,
                    key_bit_len: key.size_bits(),
                });
                prev = Some((data, next_branch));
                data = child;
            }
            std::cmp::Ordering::Greater => {
                debug_assert!(false, "LCP of prefix and key can't be greater than key");
                unsafe { std::hint::unreachable_unchecked() };
            }
        }
    };

    // Return a value with the exact key
    if inclusive && let Leaf::Value(value_range) = value_range {
        let cell = match stack.last() {
            Some(Segment {
                data, next_branch, ..
            }) => match data.reference_cloned(*next_branch as u8) {
                Some(cell) => ok!(context.load_cell(cell, LoadMode::Resolve)),
                None => return Err(Error::CellUnderflow),
            },
            None => root,
        };

        let original_key = ok!(original_key_range.apply(key.cell()));
        ok!(result_key.store_slice_data(original_key));

        return Ok(Some((result_key, (value_range, cell))));
    }

    // Rewind back to the divergent branch
    let rev_direction = towards.into_branch().reversed();
    let (mut data, mut remaining_bits, first_branch) = 'fork: {
        if let Leaf::Divergence(next_branch) = value_range
            && next_branch == rev_direction
        {
            // Skip rewinding if the key diverged towards the opposite direction.
            let remaining_bits = key.size_bits();
            let prefix_len = key_bit_len - remaining_bits;
            original_key_range = original_key_range.get_prefix(prefix_len, 0);
            let _compatibility_gas = ok!(context.load_dyn_cell(data, LoadMode::UseGas));
            break 'fork (data, remaining_bits, None);
        }

        while let Some(Segment {
            data,
            next_branch,
            key_bit_len: remaining_bits,
        }) = stack.pop()
        {
            let prefix_len = key_bit_len - remaining_bits;
            let signed_root = signed && prefix_len == 1;

            // Pop until the first diverged branch
            let first_branch = if signed_root && next_branch != rev_direction {
                rev_direction
            } else if !signed_root && next_branch == rev_direction {
                rev_direction.reversed()
            } else {
                continue;
            };

            // Remove the last bit from the prefix (we are changing it to the opposite)
            original_key_range = original_key_range.get_prefix(prefix_len - 1, 0);
            prev = Some((data, next_branch));
            break 'fork (data, remaining_bits, Some(first_branch));
        }
        // There is no next/prev element if rewind consumed all stack
        return Ok(None);
    };

    // Store the longest suitable prefix
    let original_key = ok!(original_key_range.apply(key.cell()));
    ok!(result_key.store_slice_data(original_key));

    // Prepare the node to start the final search
    if let Some(branch) = first_branch {
        ok!(result_key.store_bit(branch.into_bit()));
        let child = match data.reference(branch as u8) {
            // TODO: possibly incorrect for signed find
            Some(child) => ok!(context.load_dyn_cell(child, LoadMode::Full)),
            None => return Err(Error::CellUnderflow),
        };
        prev = Some((data, branch));
        data = child;
    }

    // Try to find the required leaf
    let value_range = loop {
        let mut remaining_data = ok!(data.as_slice());

        // Read the key part written in the current edge
        let prefix = &ok!(read_label(&mut remaining_data, remaining_bits));
        if !prefix.is_data_empty() {
            ok!(result_key.store_slice_data(prefix));
        }

        match remaining_bits.checked_sub(prefix.size_bits()) {
            Some(0) => break remaining_data.range(),
            Some(remaining) => {
                if remaining_data.size_refs() < 2 {
                    return Err(Error::CellUnderflow);
                }
                remaining_bits = remaining - 1;
            }
            None => return Err(Error::CellUnderflow),
        }

        ok!(result_key.store_bit(rev_direction.into_bit()));

        let child = match data.reference(rev_direction as u8) {
            Some(child) => ok!(context.load_dyn_cell(child, LoadMode::Full)),
            None => return Err(Error::CellUnderflow),
        };
        prev = Some((data, rev_direction));
        data = child;
    };

    let cell = match prev {
        Some((prev, next_branch)) => match prev.reference_cloned(next_branch as u8) {
            Some(cell) => ok!(context.load_cell(cell, LoadMode::Resolve)),
            None => return Err(Error::CellUnderflow),
        },
        None => root,
    };

    Ok(Some((result_key, (value_range, cell))))
}

/// Finds the specified dict bound and returns a key and a value corresponding to the key.
pub fn dict_find_bound<'a: 'b, 'b, 'c: 'a>(
    dict: Option<&'a Cell>,
    mut key_bit_len: u16,
    bound: DictBound,
    signed: bool,
    context: &'c dyn CellContext,
) -> Result<Option<(CellDataBuilder, CellSlice<'b>)>, Error> {
    let mut data = match dict {
        Some(data) => ok!(context
            .load_dyn_cell(data.as_ref(), LoadMode::Full)
            .and_then(CellSlice::new)),
        None => return Ok(None),
    };

    let mut direction = None;
    let mut key = CellDataBuilder::new();

    // Try to find the required leaf
    loop {
        // Read the key part written in the current edge
        let prefix = ok!(read_label(&mut data, key_bit_len));
        if !prefix.is_data_empty() {
            ok!(key.store_slice_data(prefix));
        }

        match key_bit_len.checked_sub(prefix.size_bits()) {
            Some(0) => break,
            Some(remaining) => {
                if data.size_refs() < 2 {
                    return Err(Error::CellUnderflow);
                }
                key_bit_len = remaining - 1;
            }
            None => return Err(Error::CellUnderflow),
        }

        let next_branch = bound.update_direction(&prefix, signed, &mut direction);
        ok!(key.store_bit(next_branch.into_bit()));

        // Load next child based on the next bit
        data = match data.cell().reference(next_branch as u8) {
            Some(data) => ok!(context
                .load_dyn_cell(data, LoadMode::Full)
                .and_then(CellSlice::new)),
            None => return Err(Error::CellUnderflow),
        };
    }

    // Return the last slice as data
    Ok(Some((key, data)))
}

/// Finds the specified dict bound and returns a key and cell slice parts corresponding to the key.
pub fn dict_find_bound_owned(
    dict: Option<&Cell>,
    mut key_bit_len: u16,
    bound: DictBound,
    signed: bool,
    context: &dyn CellContext,
) -> Result<Option<(CellDataBuilder, CellSliceParts)>, Error> {
    let root = match dict {
        Some(data) => ok!(context.load_cell(data.clone(), LoadMode::Full)),
        None => return Ok(None),
    };
    let mut data = ok!(root.as_slice());
    let mut prev = None;

    let mut direction = None;
    let mut key = CellDataBuilder::new();

    // Try to find the required leaf
    loop {
        // Read the key part written in the current edge
        let prefix = ok!(read_label(&mut data, key_bit_len));
        #[allow(clippy::needless_borrow)]
        if !prefix.is_data_empty() {
            ok!(key.store_slice_data(prefix));
        }

        match key_bit_len.checked_sub(prefix.size_bits()) {
            Some(0) => break,
            Some(remaining) => {
                if data.size_refs() < 2 {
                    return Err(Error::CellUnderflow);
                }
                key_bit_len = remaining - 1;
            }
            None => return Err(Error::CellUnderflow),
        }

        let next_branch = bound.update_direction(&prefix, signed, &mut direction);
        ok!(key.store_bit(next_branch.into_bit()));

        // Load next child based on the next bit
        prev = Some((data.cell(), next_branch));
        data = match data.cell().reference(next_branch as u8) {
            Some(data) => ok!(context
                .load_dyn_cell(data, LoadMode::Full)
                .and_then(CellSlice::new)),
            None => return Err(Error::CellUnderflow),
        };
    }

    // Build cell slice parts
    let range = data.range();
    let slice = match prev {
        Some((prev, next_branch)) => {
            let cell = match prev.reference_cloned(next_branch as u8) {
                Some(cell) => ok!(context.load_cell(cell, LoadMode::Resolve)),
                None => return Err(Error::CellUnderflow),
            };
            (range, cell)
        }
        None => (range, root),
    };

    // Return the last slice as data
    Ok(Some((key, slice)))
}

/// Searches for an item using a predicate on extra values.
pub fn aug_dict_find_by_extra<'a, A, S>(
    dict: Option<&'a Cell>,
    mut key_bit_len: u16,
    mut flow: S,
) -> Result<Option<(CellDataBuilder, A, CellSlice<'a>)>, Error>
where
    S: SearchByExtra<A>,
    A: Load<'a> + 'a,
{
    struct Leaf<'a, A> {
        prefix: CellSlice<'a>,
        extra: A,
        value: CellSlice<'a>,
    }

    struct Edge<'a, A> {
        prefix: CellSlice<'a>,
        key_bit_len: u16,
        extra: A,
        left: &'a DynCell,
        right: &'a DynCell,
    }

    enum Next<'a, A> {
        Leaf(Leaf<'a, A>),
        Edge(Edge<'a, A>),
    }

    impl<'a, A> Next<'a, A> {
        fn prefix(&self) -> &CellSlice<'a> {
            match self {
                Self::Leaf(leaf) => &leaf.prefix,
                Self::Edge(edge) => &edge.prefix,
            }
        }

        fn extra(&'a self) -> &'a A {
            match self {
                Self::Leaf(leaf) => &leaf.extra,
                Self::Edge(edge) => &edge.extra,
            }
        }
    }

    fn preload_branch<'a, A>(data: &'a DynCell, mut key_bit_len: u16) -> Result<Next<'a, A>, Error>
    where
        A: Load<'a> + 'a,
    {
        let mut data = ok!(data.as_slice());
        let prefix = ok!(read_label(&mut data, key_bit_len));

        let is_edge = match key_bit_len.checked_sub(prefix.size_bits()) {
            Some(0) => false,
            Some(remaining) => {
                if data.size_refs() < 2 {
                    return Err(Error::CellUnderflow);
                }
                key_bit_len = remaining - 1;
                true
            }
            None => return Err(Error::CellUnderflow),
        };

        let mut children = None;
        if is_edge {
            let left = ok!(data.load_reference());
            let right = ok!(data.load_reference());
            children = Some((left, right));
        };
        let extra = ok!(A::load_from(&mut data));

        Ok(match children {
            None => Next::Leaf(Leaf {
                prefix,
                extra,
                value: data,
            }),
            Some((left, right)) => Next::Edge(Edge {
                prefix,
                key_bit_len,
                extra,
                left,
                right,
            }),
        })
    }

    let data = match dict {
        Some(data) => data.as_ref(),
        None => return Ok(None),
    };

    let mut key_builder = CellDataBuilder::new();

    let mut next = ok!(preload_branch::<A>(data, key_bit_len));
    loop {
        let prefix = next.prefix();
        if !prefix.is_data_empty() {
            ok!(key_builder.store_slice_data(prefix));
        }

        match next {
            Next::Leaf(leaf) if flow.on_leaf(&leaf.extra) => {
                break Ok(Some((key_builder, leaf.extra, leaf.value)));
            }
            Next::Leaf(_) => break Ok(None),
            Next::Edge(edge) => {
                key_bit_len = edge.key_bit_len;

                let left = ok!(preload_branch::<A>(edge.left, key_bit_len));
                let right = ok!(preload_branch::<A>(edge.right, key_bit_len));

                next = match flow.on_edge(left.extra(), right.extra()) {
                    ControlFlow::Continue(Branch::Left) => {
                        ok!(key_builder.store_bit_zero());
                        left
                    }
                    ControlFlow::Continue(Branch::Right) => {
                        ok!(key_builder.store_bit_one());
                        right
                    }
                    ControlFlow::Break(()) => return Ok(None),
                };
            }
        }
    }
}
