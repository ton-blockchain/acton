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
/// Warns when `send` mode is passed as numeric literal instead of `SEND_MODE_*` constants.
///
/// ### Why is this bad?
/// Numeric send modes are hard to read and easy to misuse.
/// Named constants make intent explicit and reduce mistakes.
///
/// ### Example
/// ```tolk twoslash
/// outMsg.send(3);
/// //          ^ E012: send mode should use SEND_MODE_* constants
/// ```
///
/// Use instead:
/// ```tolk
/// outMsg.send(SEND_MODE_PAY_FEES_SEPARATELY | SEND_MODE_IGNORE_ERRORS);
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct SendModeLiteral;

impl Violation for SendModeLiteral {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    fn message(&self) -> String {
        "send mode should use SEND_MODE_* constants".to_string()
    }
}

pub fn check_call(
    checker: &mut Checker,
    file_id: FileId,
    call: &Call,
    current_inference: Option<&InferenceResult>,
) -> Option<()> {
    if !is_send_mode_call(checker, file_id, call, current_inference) {
        return None;
    }

    let mode_arg = call.arguments().last()?;
    let mode_expr = mode_arg.expr()?;

    let file = checker.file_db.get_by_id(file_id)?;
    let source = file.source().source.as_ref();

    let replacement = rewrite_mode_expr(&mode_expr, source, SEND_MODE_FLAGS);
    if !replacement.has_number_literal {
        return None;
    }

    let mut fixes = vec![];
    let mut help = "replace numeric mode literals with `SEND_MODE_*` constants".to_string();

    if replacement.fully_mapped {
        help = "use named `SEND_MODE_*` constants instead of numeric mode literals".to_string();
        fixes.push(Fix {
            message: "replace with SEND_MODE_* constants".to_string(),
            edits: vec![Edit {
                span: mode_expr.span(),
                replacement: replacement.text,
                file_id,
            }],
            applicability: Applicability::Auto,
        });
    }

    let diagnostic = Diagnostic::warning_for(file_id, SendModeLiteral)
        .with_annotations(vec![Annotation {
            span: mode_expr.span(),
            message: Some("numeric send mode literal is used here".to_string()),
            is_primary: true,
            tags: vec![],
        }])
        .with_fixes(fixes)
        .with_help(help);
    checker.emit_diagnostic(diagnostic);

    None
}

fn is_send_mode_call(
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

    // low-level sendRawMessage(msg, mode)
    if symbol.name.as_ref() == "sendRawMessage" {
        return true;
    }

    if symbol.name.as_ref() != "send" {
        return false;
    }

    // message.send(mode) or net.send(..., mode)
    is_stdlib_or_acton_symbol(checker, symbol_id)
}

const SEND_MODE_FLAGS: &[(u32, &str)] = &[
    (0, "SEND_MODE_REGULAR"),
    (1, "SEND_MODE_PAY_FEES_SEPARATELY"),
    (2, "SEND_MODE_IGNORE_ERRORS"),
    (16, "SEND_MODE_BOUNCE_ON_ACTION_FAIL"),
    (32, "SEND_MODE_DESTROY"),
    (64, "SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE"),
    (128, "SEND_MODE_CARRY_ALL_BALANCE"),
    (1024, "SEND_MODE_ESTIMATE_FEE_ONLY"),
];
