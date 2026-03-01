use crate::types::{ArgValue, Instruction};
use num_traits::ToPrimitive;
use tycho_types::boc::Boc;
use tycho_types::cell::Cell;

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
