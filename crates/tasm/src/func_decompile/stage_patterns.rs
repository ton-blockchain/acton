use super::inspect::flatten_plain_instructions;
use super::ast::StmtAst;
use super::render::{arg_as_i64, arg_as_u64};
use crate::types::{ArgValue, Method, PlainInstruction};

#[derive(Default, Debug)]
pub(crate) struct MethodPatterns {
    pub(crate) has_bounce_guard: bool,
    pub(crate) has_empty_body_guard: bool,
    pub(crate) has_op_and_query_id: bool,
    pub(crate) opcodes: Vec<u64>,
    pub(crate) call_targets: Vec<u64>,
    pub(crate) getglob_slots: Vec<u64>,
    pub(crate) setglob_slots: Vec<u64>,
    pub(crate) throw_codes: Vec<i64>,
    pub(crate) storage_load_layout: Option<Vec<String>>,
    pub(crate) storage_save_layout: Option<Vec<String>>,
    pub(crate) has_raw_reserve: bool,
    pub(crate) has_send_raw_msg: bool,
    pub(crate) has_set_code: bool,
}

impl MethodPatterns {
    pub(crate) fn analyze(method: &Method) -> Self {
        let mut patterns = Self::default();
        let flattened = flatten_plain_instructions(&method.instructions);
        let names: Vec<&str> = flattened.iter().map(|plain| plain.name.as_str()).collect();

        patterns.has_bounce_guard = has_bounce_guard(&flattened);
        patterns.has_empty_body_guard = names.contains(&"SEMPTY")
            && (names.contains(&"IFJMP")
                || names.contains(&"IFRET")
                || names.contains(&"IFNOTJMP")
                || names.contains(&"IFJMPREF"));
        patterns.has_op_and_query_id = has_op_and_query_load(&flattened);
        patterns.opcodes = extract_opcodes(&flattened);
        patterns.storage_load_layout = detect_storage_load_layout(&flattened);
        patterns.storage_save_layout = detect_storage_save_layout(&flattened);
        patterns.has_raw_reserve = names.contains(&"RAWRESERVE");
        patterns.has_send_raw_msg = names.contains(&"SENDRAWMSG");
        patterns.has_set_code = names.contains(&"SETCODE");

        for plain in flattened {
            if plain.name == "CALLDICT"
                && let Some(target) = plain.args.first().and_then(arg_as_u64)
                && !patterns.call_targets.contains(&target)
            {
                patterns.call_targets.push(target);
            }
            if plain.name == "GETGLOB"
                && let Some(slot) = plain.args.first().and_then(arg_as_u64)
                && !patterns.getglob_slots.contains(&slot)
            {
                patterns.getglob_slots.push(slot);
            }
            if plain.name == "SETGLOB"
                && let Some(slot) = plain.args.first().and_then(arg_as_u64)
                && !patterns.setglob_slots.contains(&slot)
            {
                patterns.setglob_slots.push(slot);
            }
            if plain.name.starts_with("THROW")
                && let Some(code) = plain.args.first().and_then(arg_as_i64)
                && !patterns.throw_codes.contains(&code)
            {
                patterns.throw_codes.push(code);
            }
        }

        patterns.call_targets.sort_unstable();
        patterns.getglob_slots.sort_unstable();
        patterns.setglob_slots.sort_unstable();
        patterns.throw_codes.sort_unstable();

        patterns
    }
}

pub(crate) fn apply_pattern_rewrites(stmts: &mut [StmtAst], patterns: &MethodPatterns) {
    let _ = (stmts, patterns);
}

fn has_op_and_query_load(instructions: &[&PlainInstruction]) -> bool {
    for window in instructions.windows(2) {
        let a = window[0];
        let b = window[1];
        if a.name == "LDU"
            && b.name == "LDU"
            && a.args.first().and_then(arg_as_u64) == Some(32)
            && b.args.first().and_then(arg_as_u64) == Some(64)
        {
            return true;
        }
    }
    false
}

fn has_bounce_guard(instructions: &[&PlainInstruction]) -> bool {
    let mut has_ldu4 = false;
    let mut has_and = false;
    let mut has_one = false;
    let mut has_branch = false;

    for plain in instructions {
        if plain.name == "LDU" && plain.args.first().and_then(arg_as_u64) == Some(4) {
            has_ldu4 = true;
        }
        if plain.name == "AND" {
            has_and = true;
        }
        if plain.name.starts_with("PUSHINT") && plain.args.first().and_then(arg_as_u64) == Some(1) {
            has_one = true;
        }
        if matches!(
            plain.name.as_str(),
            "IFJMP" | "IFRET" | "IFJMPREF" | "IFELSE" | "IFNOTJMP"
        ) {
            has_branch = true;
        }
    }

    has_ldu4 && has_and && has_one && has_branch
}

fn detect_storage_load_layout(instructions: &[&PlainInstruction]) -> Option<Vec<String>> {
    for (i, plain) in instructions.iter().enumerate() {
        if plain.name != "PUSHCTR" {
            continue;
        }
        let Some(ArgValue::Control(control)) = plain.args.first() else {
            continue;
        };
        if control.idx != 4 {
            continue;
        }
        let Some(next) = instructions.get(i + 1) else {
            continue;
        };
        if next.name != "CTOS" {
            continue;
        }
        let mut layout = Vec::new();
        for rest in instructions.iter().skip(i + 2) {
            if !is_storage_load_instruction(&rest.name) {
                break;
            }
            layout.push(rest.name.clone());
        }
        if !layout.is_empty() {
            return Some(layout);
        }
    }
    None
}

fn detect_storage_save_layout(instructions: &[&PlainInstruction]) -> Option<Vec<String>> {
    for (i, plain) in instructions.iter().enumerate() {
        if plain.name != "NEWC" {
            continue;
        }
        let mut layout = Vec::new();
        for rest in instructions.iter().skip(i + 1) {
            if !is_storage_store_instruction(&rest.name) {
                break;
            }
            layout.push(rest.name.clone());
        }
        if !layout.is_empty() {
            return Some(layout);
        }
    }
    None
}

fn is_storage_load_instruction(name: &str) -> bool {
    matches!(
        name,
        "LDU" | "LDI" | "LDGRAMS" | "LDMSGADDR" | "LDREF" | "LDSLICE" | "LDICT" | "ENDS"
    )
}

fn is_storage_store_instruction(name: &str) -> bool {
    matches!(
        name,
        "STU" | "STI" | "STGRAMS" | "STSLICER" | "STREF" | "STDICT" | "ENDC" | "POPCTR" | "SETGLOB"
    )
}

fn extract_opcodes(instructions: &[&PlainInstruction]) -> Vec<u64> {
    let mut opcodes = Vec::new();
    for (i, plain) in instructions.iter().enumerate() {
        if !plain.name.starts_with("PUSHINT") {
            continue;
        }
        let Some(value) = plain.args.first().and_then(arg_as_u64) else {
            continue;
        };
        if value <= 255 {
            continue;
        }

        let end = (i + 8).min(instructions.len());
        let mut saw_equal = false;
        let mut saw_branch = false;
        for tail in instructions.iter().take(end).skip(i + 1) {
            if tail.name == "EQUAL" {
                saw_equal = true;
            }
            if matches!(
                tail.name.as_str(),
                "IFJMP" | "IFJMPREF" | "IFNOTJMP" | "IFRET" | "IFELSE"
            ) {
                saw_branch = true;
            }
        }
        if saw_equal && saw_branch && !opcodes.contains(&value) {
            opcodes.push(value);
        }
    }
    opcodes
}
