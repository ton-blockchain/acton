use super::inspect::{as_plain, flatten_plain_instructions};
use super::ast::{MethodSignatureAst, ParamAst};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReturnKind {
    Unit,
    Int,
    Cell,
    Slice,
    Builder,
    Tuple(Vec<ValueType>),
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
    ret: &ReturnKind,
) -> String {
    build_method_signature_ast(method, kind, params, param_types, ret).render()
}

pub(crate) fn build_method_signature_ast(
    method: &Method,
    kind: MethodKind,
    params: &[String],
    param_types: &[ValueType],
    ret: &ReturnKind,
) -> MethodSignatureAst {
    let params = render_param_pairs(kind, params, param_types);
    let return_type = return_type_name(ret);
    let (name, qualifiers) = match kind {
        MethodKind::RecvInternal => (
            "recv_internal".to_string(),
            vec!["impure".to_string()],
        ),
        MethodKind::Getter => (
            format!("get_method_{}", method.id),
            vec![format!("method_id({})", method.id)],
        ),
        MethodKind::Helper => (
            format!("__dict_method_{}", method.id),
            vec!["impure".to_string(), format!("method_id({})", method.id)],
        ),
        MethodKind::External => (
            format!("method_{}", method.id),
            vec!["impure".to_string(), format!("method_id({})", method.id)],
        ),
    };

    MethodSignatureAst {
        return_type,
        name,
        params,
        qualifiers,
    }
}

pub(crate) fn infer_return_kind(kind: MethodKind, state: &LiftState) -> ReturnKind {
    if kind == MethodKind::RecvInternal {
        return ReturnKind::Unit;
    }

    let return_types = state.return_expr_types();
    if return_types.len() > 1 {
        return ReturnKind::Tuple(return_types);
    }

    let Some(top_ty) = state.peek_expr_type_for_return() else {
        return ReturnKind::Unit;
    };

    match top_ty {
        ValueType::Int => ReturnKind::Int,
        ValueType::Cell => ReturnKind::Cell,
        ValueType::Slice => ReturnKind::Slice,
        ValueType::Builder => ReturnKind::Builder,
        ValueType::Unknown => ReturnKind::Tuple(vec![ValueType::Unknown]),
    }
}

fn return_type_name(ret: &ReturnKind) -> String {
    match ret {
        ReturnKind::Unit => "()".to_string(),
        ReturnKind::Int => "int".to_string(),
        ReturnKind::Cell => "cell".to_string(),
        ReturnKind::Slice => "slice".to_string(),
        ReturnKind::Builder => "builder".to_string(),
        ReturnKind::Tuple(types) => {
            if types.is_empty() {
                "tuple".to_string()
            } else if types.len() == 1 {
                tuple_item_type_name(types[0]).to_string()
            } else {
                let items = types
                    .iter()
                    .copied()
                    .map(tuple_item_type_name)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({items})")
            }
        }
    }
}

fn tuple_item_type_name(ty: ValueType) -> &'static str {
    match ty {
        ValueType::Int => "int",
        ValueType::Cell => "cell",
        ValueType::Slice => "slice",
        ValueType::Builder => "builder",
        ValueType::Unknown => "int",
    }
}

fn render_param_pairs(kind: MethodKind, params: &[String], param_types: &[ValueType]) -> Vec<ParamAst> {
    if kind == MethodKind::RecvInternal {
        return match params.len() {
            4 => vec![
                ParamAst { ty: "int".to_string(), name: "balance".to_string() },
                ParamAst { ty: "int".to_string(), name: "msg_value".to_string() },
                ParamAst { ty: "cell".to_string(), name: "in_msg_full".to_string() },
                ParamAst { ty: "slice".to_string(), name: "in_msg_body".to_string() },
            ],
            3 => vec![
                ParamAst { ty: "int".to_string(), name: "msg_value".to_string() },
                ParamAst { ty: "cell".to_string(), name: "in_msg_full".to_string() },
                ParamAst { ty: "slice".to_string(), name: "in_msg_body".to_string() },
            ],
            2 => vec![
                ParamAst { ty: "cell".to_string(), name: "in_msg_full".to_string() },
                ParamAst { ty: "slice".to_string(), name: "in_msg_body".to_string() },
            ],
            1 => vec![
                ParamAst { ty: "slice".to_string(), name: "in_msg_body".to_string() },
            ],
            _ => Vec::new(),
        };
    }

    if params.is_empty() {
        Vec::new()
    } else {
        params
            .iter()
            .enumerate()
            .map(|(idx, p)| {
                let ty = param_types.get(idx).copied().unwrap_or(ValueType::Unknown);
                ParamAst {
                    ty: param_type_name(ty).to_string(),
                    name: p.clone(),
                }
            })
            .collect::<Vec<_>>()
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
