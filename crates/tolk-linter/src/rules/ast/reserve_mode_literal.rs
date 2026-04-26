use super::mode_literal_helpers::{
    is_stdlib_or_acton_symbol, resolve_call_symbol, rewrite_mode_expr,
};
use crate::rules::diagnostic::{Annotation, Applicability, Diagnostic, Edit, Fix};
use crate::rules::violation::Violation;
use crate::{Checker, FixAvailability};
use tolk_macros::ViolationMetadata;
use tolk_resolver::AstNodeSpanExt;
use tolk_resolver::file_index::FileId;
use tolk_syntax::ast::expressions::Call;
use tolk_ty::InferenceResult;

/// ### What it does
/// Warns when `reserve` mode is passed as numeric literal instead of `RESERVE_MODE_*` constants.
///
/// ### Why is this bad?
/// Numeric reserve modes are hard to read and easy to misuse.
/// Named constants make intent explicit and reduce mistakes.
///
/// ### Example
/// ```tolk twoslash
/// reserveToncoinsOnBalance(ton("0.1"), 3);
/// //                                   ^ E020: reserve mode should use RESERVE_MODE_* constants
/// ```
///
/// Use instead:
/// ```tolk
/// reserveToncoinsOnBalance(ton("0.1"), RESERVE_MODE_ALL_BUT_AMOUNT + RESERVE_MODE_AT_MOST);
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct ReserveModeLiteral;

impl Violation for ReserveModeLiteral {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    fn message(&self) -> String {
        "reserve mode should use RESERVE_MODE_* constants".to_string()
    }
}

pub fn check_call(
    checker: &mut Checker,
    file_id: FileId,
    call: &Call,
    current_inference: Option<&InferenceResult>,
) -> Option<()> {
    if !is_reserve_mode_call(checker, file_id, call, current_inference) {
        return None;
    }

    let mode_arg = call.arguments().last()?;
    let mode_expr = mode_arg.expr()?;

    let file = checker.file_db.get_by_id(file_id)?;
    let source = file.source().source.as_ref();

    let replacement = rewrite_mode_expr(&mode_expr, source, RESERVE_MODE_FLAGS);
    if !replacement.has_number_literal {
        return None;
    }

    let mut fixes = vec![];
    let mut help = "replace numeric mode literals with `RESERVE_MODE_*` constants".to_string();

    if replacement.fully_mapped {
        help = "use named `RESERVE_MODE_*` constants instead of numeric mode literals".to_string();
        fixes.push(Fix {
            message: "replace with RESERVE_MODE_* constants".to_string(),
            edits: vec![Edit {
                span: mode_expr.span(),
                replacement: replacement.text,
                file_id,
            }],
            applicability: Applicability::Auto,
        });
    }

    let diagnostic = Diagnostic::warning_for(file_id, ReserveModeLiteral)
        .with_annotations(vec![Annotation {
            span: mode_expr.span(),
            message: Some("numeric reserve mode literal is used here".to_string()),
            is_primary: true,
            tags: vec![],
        }])
        .with_fixes(fixes)
        .with_help(help);
    checker.emit_diagnostic(diagnostic);

    None
}

fn is_reserve_mode_call(
    checker: &Checker,
    file_id: FileId,
    call: &Call,
    current_inference: Option<&InferenceResult>,
) -> bool {
    let Some(symbol_id) = resolve_call_symbol(checker, file_id, call, current_inference) else {
        return false;
    };
    let Some(symbol) = checker.type_db.project_index.resolve_symbol(symbol_id) else {
        return false;
    };

    let is_reserve_call = matches!(
        symbol.name.as_ref(),
        "reserveToncoinsOnBalance" | "reserveExtraCurrenciesOnBalance"
    );
    if !is_reserve_call {
        return false;
    }

    is_stdlib_or_acton_symbol(checker, symbol_id)
}

const RESERVE_MODE_FLAGS: &[(u32, &str)] = &[
    (0, "RESERVE_MODE_EXACT_AMOUNT"),
    (1, "RESERVE_MODE_ALL_BUT_AMOUNT"),
    (2, "RESERVE_MODE_AT_MOST"),
    (4, "RESERVE_MODE_INCREASE_BY_ORIGINAL_BALANCE"),
    (8, "RESERVE_MODE_NEGATE_AMOUNT"),
    (16, "RESERVE_MODE_BOUNCE_ON_ACTION_FAIL"),
];
