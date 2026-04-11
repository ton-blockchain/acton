use super::mode_literal_helpers::resolve_call_symbol;
use super::safety_comment_helpers::has_safety_comment_above;
use crate::rules::diagnostic::{Annotation, Diagnostic};
use crate::rules::violation::Violation;
use crate::{Checker, FixAvailability};
use tolk_macros::ViolationMetadata;
use tolk_resolver::AstNodeSpanExt;
use tolk_resolver::file_index::FileId;
use tolk_syntax::ast::expressions::{Call, DotAccessField, Expr};
use tolk_ty::InferenceResult;

/// ### What it does
/// Requires a `SAFETY` comment for `msg.send(...)` calls that use dangerous send-mode flags.
///
/// ### Why is this bad?
/// Flags like `SEND_MODE_CARRY_ALL_BALANCE` or `SEND_MODE_DESTROY` can drain balance
/// or destroy the contract. These calls should always document their assumptions.
///
/// ### Example
/// ```tolk
/// outMsg.send(SEND_MODE_CARRY_ALL_BALANCE);
/// ```
///
/// Use instead:
/// ```tolk
/// // SAFETY: this path intentionally drains balance during controlled migration.
/// outMsg.send(SEND_MODE_CARRY_ALL_BALANCE);
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct DangerousSendModeMissingSafetyComment;

impl Violation for DangerousSendModeMissingSafetyComment {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::None;

    fn message(&self) -> String {
        "send with dangerous mode requires safety comment".to_string()
    }
}

pub fn check_call(
    checker: &mut Checker,
    file_id: FileId,
    call: &Call,
    current_inference: Option<&InferenceResult>,
) -> Option<()> {
    if !is_message_send_call(checker, file_id, call, current_inference) {
        return None;
    }

    let mode_arg = call.arguments().last()?;
    let mode_expr = mode_arg.expr()?;

    let file = checker.file_db.get_by_id(file_id)?;
    let source = file.source().source.as_ref();

    if !contains_dangerous_send_mode(mode_expr, source) {
        return None;
    }

    if has_safety_comment_above(source, file.line_offsets(), call.span().start()) {
        return None;
    }

    let diagnostic = Diagnostic::warning_for(file_id, DangerousSendModeMissingSafetyComment)
        .with_annotations(vec![Annotation {
            span: mode_expr.span(),
            message: Some("add `// SAFETY: ...` above this `send` call".to_string()),
            is_primary: true,
            tags: vec![],
        }])
        .with_help(
            "document why this dangerous send mode is safe in this context and which invariants are required"
        );
    checker.emit_diagnostic(diagnostic);

    None
}

fn is_message_send_call(
    checker: &Checker,
    file_id: FileId,
    call: &Call,
    current_inference: Option<&InferenceResult>,
) -> bool {
    if call.arguments().count() != 1 {
        return false;
    }

    let Some(symbol_id) = resolve_call_symbol(checker, file_id, call, current_inference) else {
        return false;
    };
    let Some(symbol) = checker.type_db.project_index.resolve_symbol(symbol_id) else {
        return false;
    };

    // We only want stdlib `OutMessage.send(self, sendMode: int)`.
    symbol.name.as_ref() == "send" && checker.file_db.is_stdlib_file(symbol_id.file_id)
}

fn contains_dangerous_send_mode(expr: Expr, source: &str) -> bool {
    match expr {
        Expr::NumberLit(lit) => lit
            .parse_u32(source)
            .is_some_and(|value| value & DANGEROUS_SEND_MODE_MASK != 0),
        Expr::Ident(ident) => is_dangerous_flag_name(ident.normalized_name(source)),
        Expr::DotAccess(dot_access) => match dot_access.field() {
            Some(DotAccessField::Ident(field_ident)) => {
                is_dangerous_flag_name(field_ident.normalized_name(source))
            }
            Some(DotAccessField::NumericIndex(_)) | None => false,
        },
        Expr::Paren(paren) => paren
            .inner()
            .is_some_and(|inner| contains_dangerous_send_mode(inner, source)),
        Expr::AsCast(cast) => cast
            .expr()
            .is_some_and(|inner| contains_dangerous_send_mode(inner, source)),
        Expr::Unary(unary) => {
            unary.operator_name(source) == "+"
                && unary
                    .argument()
                    .is_some_and(|inner| contains_dangerous_send_mode(inner, source))
        }
        Expr::Bin(bin) => {
            let op = bin.operator_name(source);
            if op != "+" && op != "|" {
                return false;
            }

            bin.left()
                .is_some_and(|left| contains_dangerous_send_mode(left, source))
                || bin
                    .right()
                    .is_some_and(|right| contains_dangerous_send_mode(right, source))
        }
        _ => false,
    }
}

fn is_dangerous_flag_name(name: &str) -> bool {
    DANGEROUS_SEND_MODE_NAMES.contains(&name)
}

const DANGEROUS_SEND_MODE_MASK: u32 = 32 | 128;
const DANGEROUS_SEND_MODE_NAMES: &[&str] = &["SEND_MODE_DESTROY", "SEND_MODE_CARRY_ALL_BALANCE"];
