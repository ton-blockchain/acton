#![allow(clippy::unwrap_used)]

// Parsing of the debug marks dictionary (BOC) produced by the Fift assembler.
//
// When Fift assembles .fif code containing MARK_* pseudo-instructions, it strips
// them from the bytecode but records each mark's position into a separate dictionary:
//
//   RawDict<256>  (key = cell hash)
//     value = 1 bit (is_normal) + HashmapE<u32, ...>  (key = mark_id)
//       value = ref -> RawDict<10>  (key = 10-bit offset within cell)
//
// The cell hashes recorded by the assembler may differ from what TVM sees at runtime,
// because TVM code is stored as a method dictionary (RawDict<19>) where each leaf cell
// contains both the hashmap label (trie key prefix) and the method bytecode. The assembler
// only sees the bytecode part, so its hash and offsets need adjustment.
//
// We reconcile this using the code BOC: for each method, we find the actual leaf cell
// in the trie and compute the offset adjustment (leaf_bits - value_bits).
//
// The result is: HashMap<cell_hash, Vec<(offset, mark_id)>> in TVM-visible coordinates.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, CellFamily, CellSlice, Load};
use tycho_types::dict::{Dict, RawDict};
use tycho_types::prelude::DynCell;

/// A single debug mark position: (`bit_offset_in_cell`, `mark_id`).
pub type MarkEntry = (i32, i32);

/// `cell_hash` (uppercase hex, 64 chars) -> sorted list of mark entries.
pub type DebugMarksDict = HashMap<String, Vec<MarkEntry>>;

/// Parse debug marks and code BOCs (base64-encoded) and produce a mapping
/// from TVM-visible cell hashes to debug mark positions.
#[must_use]
pub fn parse_debug_marks(marks_boc: &[u8], code_boc: &[u8]) -> DebugMarksDict {
    let code_cell = Boc::decode(code_boc).unwrap();
    let marks_cell = Boc::decode(marks_boc).unwrap();

    let hash_remap = build_hash_remap(&code_cell);

    let outer_dict = RawDict::<256>::from(Some(marks_cell));
    let mut result = DebugMarksDict::new();

    for kv in outer_dict.iter() {
        let kv = kv.unwrap();

        let raw_hash = hash_from_key_slice(kv.0.as_data_slice());
        let mut slice = kv.1;
        let is_normal = slice.load_bit().unwrap();

        let (final_hash, offset_adj) = if is_normal {
            (raw_hash.clone(), 0)
        } else if let Some((leaf_hash, adj)) = hash_remap.get(&raw_hash) {
            (leaf_hash.clone(), *adj)
        } else {
            eprintln!("WARNING: is_normal=false but hash {raw_hash} not in code dict");
            (raw_hash, 0)
        };

        let inner_dict = Dict::<u32, CellSlice>::load_from(&mut slice).unwrap();
        let entries = result.entry(final_hash).or_default();

        for inner_kv in inner_dict.iter() {
            let mut inner_kv = inner_kv.unwrap();
            let mark_id = inner_kv.0;

            // each mark_id maps to a ref cell containing RawDict<10> of offsets
            let ref_cell = clone_dyn_cell(inner_kv.1.load_reference().unwrap());
            let mut ref_slice = ref_cell.as_slice().unwrap();
            let offsets_dict =
                RawDict::<10>::load_from_root_ext(&mut ref_slice, Cell::empty_context()).unwrap();

            for offset_kv in offsets_dict.iter() {
                let offset_kv = offset_kv.unwrap();
                let offset = offset_kv.0.as_data_slice().load_uint(10).unwrap() as i32;
                entries.push((offset + offset_adj, mark_id as i32));
            }
        }
    }

    for entries in result.values_mut() {
        entries.sort_unstable();
    }
    result
}

/// Load base64 content from a file, trimming whitespace.
#[must_use]
pub fn read_base64_file(path: &Path) -> String {
    fs::read_to_string(path).unwrap().trim().to_string()
}

// ---------------------------------------------------------------------------
// Hash remapping: assembler hashes -> TVM-visible leaf cell hashes
// ---------------------------------------------------------------------------

// For each method in the code dict, maps:
//   value_hash (assembler-side) -> (leaf_hash (TVM-side), offset_adjustment)
fn build_hash_remap(code_cell: &Cell) -> HashMap<String, (String, i32)> {
    let mut code_slice = code_cell.as_slice().unwrap();
    let dict_cell = clone_dyn_cell(code_slice.load_reference().unwrap());
    let mut dict_slice = dict_cell.as_slice().unwrap();
    let method_dict =
        RawDict::<19>::load_from_root_ext(&mut dict_slice, Cell::empty_context()).unwrap();

    let mut remap = HashMap::new();
    for kv in method_dict.iter() {
        let kv = kv.unwrap();

        // value_hash = hash of just the bytecode (what the assembler recorded)
        let value_bits = kv.1.size_bits();
        let mut builder = CellBuilder::new();
        builder.store_slice(kv.1).unwrap();
        let value_cell = builder.build().unwrap();
        let value_hash = cell_hash_string(&value_cell);

        // leaf_hash = hash of the full trie leaf cell (what TVM sees)
        let key_binary = key_slice_to_binary(&kv.0.as_data_slice(), 19);
        let leaf_cell = find_leaf_cell(&dict_cell, &key_binary);
        let leaf_hash = cell_hash_string(&leaf_cell);
        let leaf_bits = leaf_cell.as_slice().unwrap().size_bits();

        let adjustment = i32::from(leaf_bits) - i32::from(value_bits);
        remap.insert(value_hash, (leaf_hash, adjustment));
    }
    remap
}

// ---------------------------------------------------------------------------
// Hashmap trie traversal (to find actual leaf cells)
// ---------------------------------------------------------------------------

// Read a hashmap label (hml_short / hml_long / hml_same) per TVM spec.
fn read_label(slice: &mut CellSlice, m: usize) -> String {
    if slice.load_bit().unwrap() {
        if slice.load_bit().unwrap() {
            // hml_same: 11 + bit_value + repeat_count
            let bit = slice.load_bit().unwrap();
            let len_bits = label_len_bits(m);
            let len = if len_bits > 0 {
                slice.load_uint(len_bits as u16).unwrap() as usize
            } else {
                0
            };
            (if bit { "1" } else { "0" }).repeat(len)
        } else {
            // hml_long: 10 + length + key_bits
            let len_bits = label_len_bits(m);
            let len = if len_bits > 0 {
                slice.load_uint(len_bits as u16).unwrap() as usize
            } else {
                0
            };
            read_bits_as_string(slice, len)
        }
    } else {
        // hml_short: 0 + unary(length) + key_bits
        let mut len = 0;
        while slice.load_bit().unwrap() {
            len += 1;
        }
        read_bits_as_string(slice, len)
    }
}

fn label_len_bits(m: usize) -> usize {
    if m == 0 {
        return 0;
    }
    ((m + 1) as f64).log2().ceil() as usize
}

// Walk the hashmap trie following key bits to find the leaf cell.
fn find_leaf_cell(cell: &Cell, key: &str) -> Cell {
    let mut slice = cell.as_slice().unwrap();
    let label = read_label(&mut slice, key.len());

    assert!(key.starts_with(&label), "key doesn't match label");

    if label.len() == key.len() {
        return cell.clone();
    }

    let remaining = &key[label.len()..];
    let branch_bit = remaining.as_bytes()[0];

    let left = clone_dyn_cell(slice.load_reference().unwrap());
    let right = clone_dyn_cell(slice.load_reference().unwrap());
    let child = if branch_bit == b'1' { right } else { left };

    find_leaf_cell(&child, &remaining[1..])
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn clone_dyn_cell(cell: &DynCell) -> Cell {
    Boc::decode(Boc::encode(cell)).unwrap()
}

fn read_bits_as_string(slice: &mut CellSlice, len: usize) -> String {
    let mut out = String::with_capacity(len);
    for _ in 0..len {
        out.push(if slice.load_bit().unwrap() { '1' } else { '0' });
    }
    out
}

fn key_slice_to_binary(slice: &CellSlice, bits: usize) -> String {
    read_bits_as_string(&mut slice.clone(), bits)
}

// load_uint() returns u64, can't fit 256 bits; load_biguint() absent in this tycho-types version
fn hash_from_key_slice(mut slice: CellSlice) -> String {
    let mut bytes = [0u8; 32];
    for b in &mut bytes {
        *b = slice.load_uint(8).unwrap() as u8;
    }
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

fn cell_hash_string(cell: &Cell) -> String {
    cell.repr_hash().to_string().to_uppercase()
}
