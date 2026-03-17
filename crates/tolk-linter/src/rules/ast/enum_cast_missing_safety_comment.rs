use super::safety_comment_helpers::has_safety_comment_above;
use crate::rules::diagnostic::{Annotation, Diagnostic};
use crate::rules::violation::Violation;
use crate::{Checker, FixAvailability};
use tolk_macros::ViolationMetadata;
use tolk_resolver::AstNodeSpanExt;
use tolk_resolver::file_index::FileId;
use tolk_syntax::ast::expressions::{AsCast, Expr};
use tolk_ty::{InferenceResult, TyData};

/// ### What it does
/// Requires a `SAFETY` comment for non-literal `as` casts to enum types.
///
/// ### Why is this bad?
/// Casting arbitrary values to enums can construct invalid enum values.
/// Compiler optimizations may rely on enum invariants, so invalid values
/// can lead to subtle and hard-to-debug behavior.
///
/// ### Example
/// ```tolk
/// enum Op { Add = 0, Sub = 1 }
///
/// fun parse(v: int): Op {
///     return v as Op;
/// }
/// ```
///
/// Use instead:
/// ```tolk
/// enum Op { Add = 0, Sub = 1 }
///
/// fun parse(v: int): Op {
///     if (v != 0 && v != 1) {
///         throw 7
///     }
///     // SAFETY: input is validated and guaranteed to be either 0 or 1.
///     return v as Op;
/// }
///
/// // or even better, use match
///
/// fun parse(v: int): Op {
///     return match (v) {
///         0 => Op.Add,
///         1 => Op.Sub,
///         else => throw 7,
///     }
/// }
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct EnumCastMissingSafetyComment;

impl Violation for EnumCastMissingSafetyComment {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::None;

    fn message(&self) -> String {
        "non-literal cast to enum requires safety comment".to_string()
    }
}

pub fn check_as_cast(
    checker: &mut Checker,
    file_id: FileId,
    cast: &AsCast,
    current_inference: Option<&InferenceResult>,
) -> Option<()> {
    let inference = current_inference?;

    if !cast_targets_enum(checker, inference, cast) {
        return None;
    }

    let expr = cast.expr()?;
    let file = checker.file_db.get_by_id(file_id)?;
    let source = file.source().source.as_ref();

    if is_numeric_literal_expr(expr, source) {
        return None;
    }

    if has_safety_comment_above(source, file.line_offsets(), cast.span().start()) {
        return None;
    }

    let diagnostic = Diagnostic::warning_for(file_id, EnumCastMissingSafetyComment)
        .with_annotations(vec![Annotation {
            span: cast.span(),
            message: Some("add `// SAFETY: ...` above this cast".to_string()),
            is_primary: true,
            tags: vec![],
        }])
        .with_help(
            "this cast can produce an invalid enum value; compiler optimizations may assume enum invariants.
Add a `// SAFETY: ...` comment and re-check that the incoming value is guaranteed to be one of enum variants.
Alternatively try to use `match` for a safer explicit conversion path, e.g. `match (v) { 0 => Op.Add, 1 => Op.Sub, else => throw 7 }`",
        );
    checker.emit_diagnostic(diagnostic);

    None
}

fn cast_targets_enum(checker: &Checker, inference: &InferenceResult, cast: &AsCast) -> bool {
    let Some(cast_ty) = inference.type_of(cast.span()) else {
        return false;
    };
    let unwrapped = checker.type_db.intrn.unwrap_alias(cast_ty);
    matches!(checker.type_db.intrn.data(unwrapped), TyData::Enum { .. })
}

fn is_numeric_literal_expr(expr: Expr<'_>, source: &str) -> bool {
    match expr {
        Expr::NumberLit(_) => true,
        Expr::Paren(paren) => paren
            .inner()
            .is_some_and(|inner| is_numeric_literal_expr(inner, source)),
        Expr::Unary(unary) => {
            matches!(unary.operator_name(source), "+" | "-")
                && unary
                    .argument()
                    .is_some_and(|arg| is_numeric_literal_expr(arg, source))
        }
        _ => false,
    }
}
