//! Parser for method identifiers embedded in conventional TON contract code.
//!
//! Fift's `PROGRAM{ ... }END>s` and compatible compilers emit a dispatcher
//! with a constant dictionary whose keys are numeric method identifiers.
//! This module reads only
//! that dispatcher prefix; it does not disassemble arbitrary control flow or
//! infer methods from contracts using a custom dispatcher.
//!
//! See the TON documentation for the
//! [`DICTPUSHCONST` instruction](https://docs.ton.org/foundations/whitepapers/tvm)
//! and the conventional [get-method ID calculation](https://docs.ton.org/tvm/get-method).

use anyhow::{Context, ensure};
use tycho_types::cell::Cell;
use tycho_types::dict::RawIter;

const SETCP_OPCODE: u8 = 0xff;
const DICTPUSHCONST_PREFIX: u64 = 0b1_1110_1001_0100;
const MAX_METHOD_ID_BITS: u16 = 32;

/// Extracts method identifiers from the constant method dictionary emitted at
/// the start of conventional TON contract code.
///
/// The expected prefix is `SETCP0`, followed by `DICTPUSHCONST`. Contracts
/// using another dispatcher layout are rejected rather than partially parsed.
/// Returned identifiers are unsigned dictionary keys in ascending order.
///
/// For example, this TASM prefix declares the ordinary method `0` and get
/// method `65536` before dispatching through the dictionary:
///
/// ```text
/// SETCP0
/// DICTPUSHCONST 19 [
///     0 => {
///         RET
///     }
///     65536 => {
///         PUSHINT_4 1
///         RET
///     }
/// ]
/// DICTIGETJMPZ
/// ```
pub fn parse_contract_methods(code: &Cell) -> anyhow::Result<Vec<u64>> {
    let mut slice = code
        .as_slice()
        .context("failed to load contract code cell")?;

    let opcode = slice
        .load_u8()
        .context("failed to load SETCP opcode from contract code")?;
    let codepage = slice
        .load_u8()
        .context("failed to load contract codepage")?;
    ensure!(
        opcode == SETCP_OPCODE && codepage == 0,
        "contract code does not start with SETCP0"
    );

    // DICTPUSHCONST starts with a 13-bit opcode prefix followed by the
    // non-empty dictionary marker. Cell references are stored separately from
    // data bits, so its root can be loaded after reading the 10-bit key length.
    let dict_opcode = slice
        .load_uint(13)
        .context("failed to load DICTPUSHCONST opcode from contract code")?;
    let has_dict = slice
        .load_bit()
        .context("failed to load DICTPUSHCONST dictionary flag")?;
    ensure!(
        dict_opcode == DICTPUSHCONST_PREFIX && has_dict,
        "SETCP0 is not followed by DICTPUSHCONST"
    );

    let key_bit_len = slice
        .load_uint(10)
        .context("failed to load contract method dictionary key length")?
        as u16;
    ensure!(
        key_bit_len <= MAX_METHOD_ID_BITS,
        "unsupported contract method dictionary key length: {key_bit_len}"
    );

    let root = Some(
        slice
            .load_reference_cloned()
            .context("failed to load contract method dictionary root")?,
    );
    let mut method_ids = Vec::new();
    for entry in RawIter::new(&root, key_bit_len) {
        let (key, _) = entry.context("failed to iterate contract method dictionary")?;
        debug_assert_eq!(key.size_bits(), key_bit_len);

        // Raw dictionary keys are left-aligned in CellDataBuilder, matching
        // the representation used by the C++ indexer. Contract method IDs use
        // 19-bit keys in normal compiler output; accepting up to 32 bits keeps
        // this parser compatible with the original implementation.
        let raw = key.raw_data();
        let prefix = u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]);
        let method_id = if key_bit_len == 0 {
            0
        } else {
            prefix >> (MAX_METHOD_ID_BITS - key_bit_len)
        };
        method_ids.push(u64::from(method_id));
    }

    Ok(method_ids)
}
