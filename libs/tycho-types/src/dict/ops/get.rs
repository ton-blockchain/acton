use crate::cell::*;
use crate::dict::{Branch, make_leaf, read_label, split_edge};
use crate::error::Error;

/// Returns a `CellSlice` of the value corresponding to the key.
pub fn dict_get<'a: 'b, 'b, 'c: 'a>(
    dict: Option<&'a Cell>,
    key_bit_len: u16,
    mut key: CellSlice<'b>,
    context: &'c dyn CellContext,
) -> Result<Option<CellSlice<'a>>, Error> {
    if key.size_bits() != key_bit_len {
        return Err(Error::CellUnderflow);
    }

    let mut data = match dict {
        Some(data) => ok!(context
            .load_dyn_cell(data.as_ref(), LoadMode::Full)
            .and_then(CellSlice::new)),
        None => return Ok(None),
    };

    // Try to find the required leaf
    let is_key_empty = loop {
        // Read the key part written in the current edge
        let prefix = ok!(read_label(&mut data, key.size_bits()));

        // Remove this prefix from the key
        match key.strip_data_prefix(&prefix) {
            Some(stripped_key) => {
                if stripped_key.is_data_empty() {
                    // All key parts were collected <=> value found
                    break true;
                } else if data.size_refs() < 2 {
                    // Reached leaf while key was not fully constructed
                    return Ok(None);
                } else {
                    key = stripped_key;
                }
            }
            None => break key.is_data_empty(),
        }

        // Load next child based on the next bit
        let child_index = ok!(key.load_bit()) as u8;
        data = match data.cell().reference(child_index) {
            Some(cell) => ok!(context
                .load_dyn_cell(cell, LoadMode::Full)
                .and_then(CellSlice::new)),
            None => return Err(Error::CellUnderflow),
        };
    };

    // Return the last slice as data
    Ok(if is_key_empty { Some(data) } else { None })
}

/// Returns cell slice parts of the value corresponding to the key.
pub fn dict_get_owned(
    dict: Option<&Cell>,
    key_bit_len: u16,
    mut key: CellSlice<'_>,
    context: &dyn CellContext,
) -> Result<Option<CellSliceParts>, Error> {
    if key.size_bits() != key_bit_len {
        return Err(Error::CellUnderflow);
    }

    let root = match dict {
        Some(cell) => ok!(context.load_cell(cell.clone(), LoadMode::Full)),
        None => return Ok(None),
    };
    let mut data = ok!(root.as_slice());
    let mut prev = None;

    // Try to find the required leaf
    let is_key_empty = loop {
        // Read the key part written in the current edge
        let prefix = ok!(read_label(&mut data, key.size_bits()));

        // Remove this prefix from the key
        match key.strip_data_prefix(&prefix) {
            Some(stripped_key) => {
                if stripped_key.is_data_empty() {
                    // All key parts were collected <=> value found
                    break true;
                } else if data.size_refs() < 2 {
                    // Reached leaf while key was not fully constructed
                    return Ok(None);
                } else {
                    key = stripped_key;
                }
            }
            None => break key.is_data_empty(),
        }

        // Load next child based on the next bit
        let child_index = ok!(key.load_bit()) as u8;
        prev = Some((data.cell(), child_index));
        data = match data.cell().reference(child_index) {
            Some(cell) => ok!(context
                .load_dyn_cell(cell, LoadMode::Full)
                .and_then(CellSlice::new)),
            None => return Err(Error::CellUnderflow),
        };
    };

    // Return the last slice as data
    Ok(if is_key_empty {
        Some(match prev {
            Some((prev, child_index)) => {
                let cell = match prev.reference_cloned(child_index) {
                    Some(cell) => ok!(context.load_cell(cell, LoadMode::Resolve)),
                    None => return Err(Error::CellUnderflow),
                };
                (data.range(), cell)
            }
            None => {
                let range = data.range();
                (range, root)
            }
        })
    } else {
        None
    })
}

/// Gets subdictionary by specified prefiex
/// Returns optional dictionary as Cell representation if specified prefix is present in dictionary
pub fn dict_get_subdict<'a: 'b, 'b, 'c: 'a>(
    dict: Option<&'a Cell>,
    key_bit_len: u16,
    prefix: &mut CellSlice<'b>,
    context: &'c dyn CellContext,
) -> Result<Option<Cell>, Error> {
    match dict {
        None => Ok(None),
        Some(cell) => {
            let prefix_len = prefix.size_bits();
            if prefix_len == 0 || key_bit_len < prefix_len {
                return Ok(Some(cell.clone()));
            }

            let mut data = match dict {
                Some(data) => ok!(context
                    .load_dyn_cell(data.as_ref(), LoadMode::Full)
                    .and_then(CellSlice::new)),
                None => return Ok(None),
            };

            // Try to find the required root
            let subtree = loop {
                // Read the key part written in the current edge
                let label = &mut ok!(read_label(&mut data, prefix.size_bits()));
                let lcp = prefix.longest_common_data_prefix(label);
                match lcp.size_bits().cmp(&prefix.size_bits()) {
                    std::cmp::Ordering::Equal => {
                        // Found exact key
                        let new_leaf = ok!(make_leaf(label, lcp.size_bits(), &data, context));
                        break new_leaf;
                    }
                    std::cmp::Ordering::Less if lcp.size_bits() < label.size_bits() => {
                        // Have to split edge
                        let value = ok!(CellBuilder::new().build_ext(context));
                        let split_edge =
                            ok!(split_edge(&data, label, &lcp, prefix, &value, context));
                        break split_edge;
                    }
                    std::cmp::Ordering::Less => {
                        if data.cell().reference_count() < 2 {
                            return Err(Error::CellUnderflow);
                        }
                        prefix.skip_first(lcp.size_bits(), 0).ok();
                        let next_branch = match prefix.load_bit() {
                            Ok(bit) => Branch::from(bit),
                            Err(e) => return Err(e),
                        };

                        let child = match data.cell().reference(next_branch as u8) {
                            // TODO: change mode to `LoadMode::UseGas` if copy-on-write for libraries is not ok
                            Some(cell) => ok!(context
                                .load_dyn_cell(cell, LoadMode::Full)
                                .and_then(CellSlice::new)),
                            None => return Err(Error::CellUnderflow),
                        };
                        data = child;
                    }
                    std::cmp::Ordering::Greater => {
                        debug_assert!(false, "LCP of prefix and key can't be greater than key");
                        unsafe { std::hint::unreachable_unchecked() };
                    }
                }
            };

            Ok(Some(subtree))
        }
    }
}
