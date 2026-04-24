use crate::rules::diagnostic::{Annotation, Applicability, Diagnostic, Edit, Fix};
use crate::rules::violation::Violation;
use crate::{Checker, FixAvailability};
use tolk_macros::ViolationMetadata;
use tolk_resolver::AstNodeSpanExt;
use tolk_resolver::file_index::FileId;
use tolk_syntax::AstNode;
use tolk_syntax::ast::expressions::{Expr, IsType, Unary};

/// ### What it does
/// Detects negated type checks written as `!(a is T)`.
///
/// ### Why is this bad?
/// Tolk has a dedicated `!is` operator, which is clearer than negating `is`.
///
/// ### Example
/// ```tolk twoslash
/// if (!(value is int)) {
/// //  ^^^^^^^^^^^^^^^ E022: negated `is` type check can use `!is`
///     return;
/// }
/// ```
///
/// Use instead:
/// ```tolk
/// if (value !is int) {
///     return;
/// }
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct NegatedIsTypeCanUseNotIs;

impl Violation for NegatedIsTypeCanUseNotIs {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Always;

    fn message(&self) -> String {
        "negated `is` type check can use `!is`".to_string()
    }
}

pub fn check_unary(checker: &mut Checker, file_id: FileId, node: &Unary) -> Option<()> {
    let file = checker.file_db.get_by_id(file_id)?;
    let source = file.source().source.as_ref();

    if node.operator_name(source) != "!" {
        return None;
    }

    let argument = node.argument()?;
    let is_type = as_is_type(argument)?;
    if is_type.operator_name(source) != "is" {
        return None;
    }

    let replacement = build_not_is_replacement(source, is_type)?;

    let diagnostic = Diagnostic::warning_for(file_id, NegatedIsTypeCanUseNotIs)
        .with_annotations(vec![Annotation {
            span: node.span(),
            message: Some("can be written with `!is`".to_string()),
            is_primary: true,
            tags: vec![],
        }])
        .with_fixes(vec![Fix {
            message: "use `!is` operator".to_string(),
            edits: vec![Edit {
                span: node.span(),
                replacement,
                file_id,
            }],
            applicability: Applicability::Auto,
        }])
        .with_help("replace `!(a is T)` with `a !is T`");
    checker.emit_diagnostic(diagnostic);
    Some(())
}

fn as_is_type(expr: Expr) -> Option<IsType> {
    match expr {
        Expr::IsType(is_type) => Some(is_type),
        Expr::Paren(paren) => match paren.inner()? {
            Expr::IsType(is_type) => Some(is_type),
            _ => None,
        },
        _ => None,
    }
}

fn build_not_is_replacement(source: &str, is_type: IsType) -> Option<String> {
    let operator = is_type.operator()?;
    let is_span = is_type.span();
    let operator_span = operator.span();

    let start = operator_span.start.checked_sub(is_span.start)? as usize;
    let end = operator_span.end.checked_sub(is_span.start)? as usize;

    let is_text = is_type.text(source);
    let bytes = is_text.as_bytes();
    let prefix = std::str::from_utf8(bytes.get(..start)?).ok()?;
    let suffix = std::str::from_utf8(bytes.get(end..)?).ok()?;

    Some(format!("{prefix}!is{suffix}"))
}
