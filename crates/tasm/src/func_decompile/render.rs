use crate::types::{ArgValue, Instruction};
use num_bigint::BigUint;
use num_traits::ToPrimitive;
use tycho_types::boc::Boc;
use tycho_types::cell::Cell;
use tycho_types::util::Bitstring;

pub(crate) fn push_line(lines: &mut Vec<String>, depth: usize, line: String) {
    lines.push(format!("{}{}", "    ".repeat(depth), line));
}

pub(crate) fn format_instruction_line(instruction: &Instruction, depth: usize) -> String {
    match instruction {
        Instruction::Plain(plain) => {
            let mut line = plain.name.clone();
            if !plain.args.is_empty() {
                line.push(' ');
            }
            for (idx, arg) in plain.args.iter().enumerate() {
                line.push_str(&format_arg_value(arg, depth + 1));
                if idx + 1 < plain.args.len() {
                    line.push(' ');
                }
            }
            line
        }
        Instruction::Ref(reference) => format!(
            "ref {{ {} }}",
            format_arg_value(&reference.code, depth.saturating_add(1))
        ),
        Instruction::ExoticCell(exotic) => format!("exotic {}", format_cell_literal(&exotic.cell)),
    }
}

fn format_arg_value(arg: &ArgValue, depth: usize) -> String {
    match arg {
        ArgValue::Int(value) => value.to_string(),
        ArgValue::UInt(value) => value.to_string(),
        ArgValue::Control(control) => format!("c{}", control.idx),
        ArgValue::StackRegister(reg) => format!("s{}", reg.idx),
        ArgValue::Cell(cell) => format_cell_literal(cell),
        ArgValue::Code { code, .. } => {
            let mut rendered = String::new();
            rendered.push_str("{ ");
            for (idx, instruction) in code.instructions.iter().enumerate() {
                if idx > 4 {
                    rendered.push_str("... ");
                    break;
                }
                if idx > 0 {
                    rendered.push_str("; ");
                }
                rendered.push_str(&format_instruction_line(instruction, depth + 1));
            }
            rendered.push_str(" }");
            rendered
        }
        ArgValue::CodeDictionary(dict) => {
            let ids = dict
                .methods
                .iter()
                .take(8)
                .map(|m| m.id.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            if dict.methods.len() > 8 {
                format!("[{ids}, ...]")
            } else {
                format!("[{ids}]")
            }
        }
    }
}

pub(crate) fn format_cell_literal(cell: &Cell) -> String {
    let slice = cell.as_slice_allow_exotic();
    if slice.size_refs() == 0 {
        format!("x{{{}}}", slice.display_data())
    } else {
        format!("boc{{{}}}", Boc::encode_hex(cell.clone()))
    }
}

pub(crate) fn format_func_slice_expr(cell: &Cell) -> String {
    let slice = cell.as_slice_allow_exotic();
    if slice.size_refs() != 0 {
        // Non-empty refs cannot be represented as a plain slice constant in FunC.
        return "\"\"".to_string();
    }

    let bits_hex = slice.display_data().to_string();
    bitstring_hex_to_func_slice_expr(&bits_hex).unwrap_or_else(|| "\"\"".to_string())
}

fn bitstring_hex_to_func_slice_expr(bits_hex: &str) -> Option<String> {
    let (bytes, bit_len) = Bitstring::from_hex_str(bits_hex).ok()?;
    if bit_len == 0 {
        return Some("\"\"".to_string());
    }

    let total_bits = bit_len as usize;
    let mut offset = 0usize;
    let mut builder_expr = "begin_cell()".to_string();
    while offset < total_bits {
        let chunk_len = (total_bits - offset).min(256);
        let chunk_value = bits_to_biguint(&bytes, offset, chunk_len);
        builder_expr = format!("store_uint({builder_expr}, {chunk_value}, {chunk_len})");
        offset += chunk_len;
    }

    Some(format!("begin_parse(end_cell({builder_expr}))"))
}

fn bits_to_biguint(bytes: &[u8], start_bit: usize, bit_len: usize) -> BigUint {
    let mut value = BigUint::default();
    for bit_idx in start_bit..(start_bit + bit_len) {
        value <<= 1usize;
        if bit_is_one(bytes, bit_idx) {
            value += BigUint::from(1u8);
        }
    }
    value
}

fn bit_is_one(bytes: &[u8], bit_idx: usize) -> bool {
    let byte = bytes.get(bit_idx / 8).copied().unwrap_or(0);
    let shift = 7 - (bit_idx % 8);
    ((byte >> shift) & 1) != 0
}

pub(crate) fn arg_to_string(arg: &ArgValue) -> Option<String> {
    match arg {
        ArgValue::Int(v) => Some(v.to_string()),
        ArgValue::UInt(v) => Some(v.to_string()),
        ArgValue::Control(c) => Some(format!("c{}", c.idx)),
        ArgValue::StackRegister(s) => Some(format!("s{}", s.idx)),
        ArgValue::Cell(c) => Some(format_cell_literal(c)),
        ArgValue::Code { .. } | ArgValue::CodeDictionary(_) => None,
    }
}

pub(crate) fn arg_as_u64(arg: &ArgValue) -> Option<u64> {
    match arg {
        ArgValue::UInt(value) => value.to_u64(),
        ArgValue::Int(value) => value.to_u64(),
        _ => None,
    }
}

pub(crate) fn arg_as_i64(arg: &ArgValue) -> Option<i64> {
    match arg {
        ArgValue::Int(value) => value.to_i64(),
        ArgValue::UInt(value) => value.to_i64(),
        _ => None,
    }
}
