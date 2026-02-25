use crate::Checker;
use tolk_resolver::AstNodeSpanExt;
use tolk_resolver::file_index::{FileId, SymbolId};
use tolk_resolver::resolve_index::Resolved;
use tolk_syntax::AstNode;
use tolk_syntax::ast::expressions::{Bin, Call, Expr};
use tolk_ty::InferenceResult;

pub(super) struct RewrittenMode {
    pub(super) text: String,
    pub(super) has_number_literal: bool,
    pub(super) fully_mapped: bool,
}

pub(super) fn rewrite_mode_expr(expr: &Expr, source: &str, flags: &[(u32, &str)]) -> RewrittenMode {
    match expr {
        Expr::NumberLit(lit) => {
            let literal_text = lit.text(source);
            let mapped = parse_int_literal(literal_text)
                .and_then(|value| mode_value_to_constants(value, flags));
            let fully_mapped = mapped.is_some();

            RewrittenMode {
                text: mapped.unwrap_or_else(|| literal_text.to_string()),
                has_number_literal: true,
                fully_mapped,
            }
        }
        Expr::Paren(paren) => {
            if let Some(inner) = paren.inner() {
                let rewritten = rewrite_mode_expr(&inner, source, flags);
                RewrittenMode {
                    text: format!("({})", rewritten.text),
                    has_number_literal: rewritten.has_number_literal,
                    fully_mapped: rewritten.fully_mapped,
                }
            } else {
                RewrittenMode {
                    text: paren.text(source).to_string(),
                    has_number_literal: false,
                    fully_mapped: true,
                }
            }
        }
        Expr::Bin(bin) => rewrite_bin(bin, source, flags),
        _ => RewrittenMode {
            text: expr.text(source).to_string(),
            has_number_literal: false,
            fully_mapped: true,
        },
    }
}

fn rewrite_bin(bin: &Bin, source: &str, flags: &[(u32, &str)]) -> RewrittenMode {
    let Some(left) = bin.left() else {
        return RewrittenMode {
            text: bin.text(source).to_string(),
            has_number_literal: false,
            fully_mapped: true,
        };
    };
    let Some(right) = bin.right() else {
        return RewrittenMode {
            text: bin.text(source).to_string(),
            has_number_literal: false,
            fully_mapped: true,
        };
    };

    let left_rewritten = rewrite_mode_expr(&left, source, flags);
    let right_rewritten = rewrite_mode_expr(&right, source, flags);
    let has_number_literal =
        left_rewritten.has_number_literal || right_rewritten.has_number_literal;

    if !has_number_literal {
        return RewrittenMode {
            text: bin.text(source).to_string(),
            has_number_literal: false,
            fully_mapped: true,
        };
    }

    if bin.operator_name(source) != "+" {
        return RewrittenMode {
            text: bin.text(source).to_string(),
            has_number_literal: true,
            fully_mapped: false,
        };
    }

    RewrittenMode {
        text: format!("{} + {}", left_rewritten.text, right_rewritten.text),
        has_number_literal: true,
        fully_mapped: left_rewritten.fully_mapped && right_rewritten.fully_mapped,
    }
}

pub(super) fn parse_int_literal(raw: &str) -> Option<u32> {
    let normalized = raw.replace('_', "");

    if let Some(hex) = normalized
        .strip_prefix("0x")
        .or_else(|| normalized.strip_prefix("0X"))
    {
        return u32::from_str_radix(hex, 16).ok();
    }
    if let Some(binary) = normalized
        .strip_prefix("0b")
        .or_else(|| normalized.strip_prefix("0B"))
    {
        return u32::from_str_radix(binary, 2).ok();
    }

    normalized.parse::<u32>().ok()
}

pub(super) fn mode_value_to_constants(mut value: u32, flags: &[(u32, &str)]) -> Option<String> {
    if value == 0 {
        return flags
            .iter()
            .find_map(|(flag, name)| (*flag == 0).then(|| (*name).to_string()));
    }

    let mut parts: Vec<&str> = vec![];
    for &(flag, constant) in flags.iter().rev() {
        if flag == 0 {
            continue;
        }
        if value >= flag && (value & flag) == flag {
            parts.push(constant);
            value -= flag;
        }
    }

    if value != 0 || parts.is_empty() {
        return None;
    }

    parts.reverse();
    Some(parts.join(" + "))
}

pub(super) fn resolve_call_symbol(
    checker: &Checker,
    file_id: FileId,
    call: &Call,
    current_inference: Option<&InferenceResult>,
) -> Option<SymbolId> {
    let callee_ident = call.callee_identifier()?;
    let resolve_index = checker.resolve_index_for(file_id);

    if let Some(resolve_index) = resolve_index
        && let Some(name_use) = resolve_index.find_use(callee_ident.start_byte())
        && let Resolved::Global(symbol_id) = name_use.resolved
    {
        return Some(symbol_id);
    }

    if let Some(current_inference) = current_inference
        && let Some(name_use) = current_inference.resolve(callee_ident.span())
        && let Resolved::Global(symbol_id) = name_use.resolved
    {
        return Some(symbol_id);
    }

    None
}

pub(super) fn is_stdlib_or_acton_symbol(checker: &Checker, symbol_id: SymbolId) -> bool {
    checker.file_db.is_stdlib_file(symbol_id.file_id)
        || checker.file_db.is_acton_file(symbol_id.file_id)
}
