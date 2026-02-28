use super::inspect::{as_plain, flatten_plain_instructions};
use super::render::arg_as_u64;
use super::stage_patterns::MethodPatterns;
use super::stage_stack::{LiftState, ValueType};
use crate::types::{ArgValue, Code, Method};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MethodKind {
    RecvInternal,
    Getter,
    Helper,
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReturnKind {
    Unit,
    Int,
    Cell,
    Slice,
    Builder,
    Tuple,
}

pub(crate) fn infer_params_for_method(kind: MethodKind, state: &LiftState) -> Vec<String> {
    if kind == MethodKind::RecvInternal {
        return vec![
            "balance".to_string(),
            "msg_value".to_string(),
            "in_msg_full".to_string(),
            "in_msg_body".to_string(),
        ];
    }

    let mut params = state.params().to_vec();
    params.sort_by(|a, b| match (parse_arg_param_index(a), parse_arg_param_index(b)) {
        (Some(ia), Some(ib)) => ia.cmp(&ib),
        _ => a.cmp(b),
    });
    params
}

pub(crate) fn classify_method(
    method: &Method,
    called_targets: &BTreeSet<u64>,
    patterns: &MethodPatterns,
) -> MethodKind {
    if method.id == 0 {
        return MethodKind::RecvInternal;
    }
    if called_targets.contains(&method.id) {
        return MethodKind::Helper;
    }
    if !patterns.getglob_slots.is_empty()
        && patterns.opcodes.is_empty()
        && method.instructions.len() <= 32
    {
        return MethodKind::Getter;
    }
    MethodKind::External
}

pub(crate) fn render_method_signature(
    method: &Method,
    kind: MethodKind,
    params: &[String],
    param_types: &[ValueType],
    ret: ReturnKind,
) -> String {
    let rendered_params = render_param_list(kind, params, param_types);
    let ret_ty = return_type_name(ret);
    match kind {
        MethodKind::RecvInternal => format!("() recv_internal({rendered_params}) impure {{"),
        MethodKind::Getter => format!(
            "{ret_ty} get_method_{}({}) method_id({}) {{",
            method.id, rendered_params, method.id
        ),
        MethodKind::Helper => format!(
            "{ret_ty} __dict_method_{}({}) impure {{",
            method.id, rendered_params
        ),
        MethodKind::External => format!(
            "{ret_ty} method_{}({}) impure method_id({}) {{",
            method.id, rendered_params, method.id
        ),
    }
}

pub(crate) fn infer_return_kind(kind: MethodKind, state: &LiftState) -> ReturnKind {
    if kind == MethodKind::RecvInternal {
        return ReturnKind::Unit;
    }

    let Some(top_ty) = state.peek_expr_type_for_return() else {
        return ReturnKind::Unit;
    };

    match top_ty {
        ValueType::Int => ReturnKind::Int,
        ValueType::Cell => ReturnKind::Cell,
        ValueType::Slice => ReturnKind::Slice,
        ValueType::Builder => ReturnKind::Builder,
        ValueType::Unknown => ReturnKind::Tuple,
    }
}

fn return_type_name(ret: ReturnKind) -> &'static str {
    match ret {
        ReturnKind::Unit => "()",
        ReturnKind::Int => "int",
        ReturnKind::Cell => "cell",
        ReturnKind::Slice => "slice",
        ReturnKind::Builder => "builder",
        ReturnKind::Tuple => "tuple",
    }
}

fn render_param_list(kind: MethodKind, params: &[String], param_types: &[ValueType]) -> String {
    if kind == MethodKind::RecvInternal {
        return match params.len() {
            4 => "int balance, int msg_value, cell in_msg_full, slice in_msg_body".to_string(),
            3 => "int msg_value, cell in_msg_full, slice in_msg_body".to_string(),
            2 => "cell in_msg_full, slice in_msg_body".to_string(),
            1 => "slice in_msg_body".to_string(),
            _ => String::new(),
        };
    }

    if params.is_empty() {
        String::new()
    } else {
        params
            .iter()
            .enumerate()
            .map(|(idx, p)| {
                let ty = param_types.get(idx).copied().unwrap_or(ValueType::Unknown);
                format!("{} {p}", param_type_name(ty))
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn param_type_name(ty: ValueType) -> &'static str {
    match ty {
        ValueType::Int => "int",
        ValueType::Cell => "cell",
        ValueType::Slice => "slice",
        ValueType::Builder => "builder",
        ValueType::Unknown => "int",
    }
}

pub(crate) fn collect_call_targets(methods: &[Method]) -> BTreeSet<u64> {
    let mut targets = BTreeSet::new();
    for method in methods {
        let flattened = flatten_plain_instructions(&method.instructions);
        for plain in flattened {
            if plain.name != "CALLDICT" {
                continue;
            }
            if let Some(target) = plain.args.first().and_then(arg_as_u64) {
                targets.insert(target);
            }
        }
    }
    targets
}

pub(crate) fn extract_method_dictionary(code: &Code) -> Option<&crate::types::CodeDictionary> {
    code.instructions.iter().find_map(|instruction| {
        let plain = as_plain(instruction)?;
        if plain.name != "DICTPUSHCONST" {
            return None;
        }
        plain.args.iter().find_map(|arg| match arg {
            ArgValue::CodeDictionary(dict) => Some(dict),
            _ => None,
        })
    })
}

fn parse_arg_param_index(name: &str) -> Option<usize> {
    name.strip_prefix("arg")?.parse::<usize>().ok()
}
