use crate::rules::diagnostic::{Annotation, Applicability, Diagnostic, DiagnosticTag, Edit, Fix};
use crate::rules::violation::Violation;
use crate::{Checker, FixAvailability, Rule};
use tolk_macros::ViolationMetadata;
use tolk_resolver::AstNodeSpanExt;
use tolk_resolver::file_index::{FileId, Span};
use tolk_syntax::ast::expressions::{Expr, NotNull};
use tolk_syntax::{AstNode, TryFromNode};
use tolk_ty::InferenceResult;

/// ### What it does
/// Detects a not-null assertion when the expression's inferred type cannot be `null`.
///
/// ### Why is this bad?
/// The `!` operator only removes `null` from a nullable type. Applying it to an expression
/// that is already non-null has no effect and can mislead readers into expecting a nullable value.
///
/// ### Behavior notes
/// For a direct chain such as `value!!`, E031 reports only the outermost unnecessary `!`.
/// When E014 is also enabled, E014 owns the chain's auto-fix to avoid overlapping edits.
///
/// ### Example
/// ```tolk twoslash
/// fun addOne(value: int?): int {
///     if (value != null) {
///         return value! + 1;
///         //          ^ E031: unnecessary not-null assertion
///     }
///     return 0;
/// }
/// ```
///
/// Use instead:
/// ```tolk
/// fun addOne(value: int?): int {
///     if (value != null) {
///         return value + 1;
///     }
///     return 0;
/// }
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct UnnecessaryNotNullAssertion;

impl Violation for UnnecessaryNotNullAssertion {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    fn message(&self) -> String {
        "unnecessary not-null assertion".to_string()
    }
}

pub fn check_not_null(
    checker: &mut Checker,
    file_id: FileId,
    node: &NotNull,
    current_inference: Option<&InferenceResult>,
) -> Option<()> {
    // Report only the outermost useless operator in a direct chain such as `value!!!`.
    if node
        .syntax()
        .parent()
        .is_some_and(|parent| NotNull::try_from_node(parent).is_ok())
    {
        return None;
    }

    let inner = node.inner()?;
    let inference = current_inference?;
    let inner_ty = inference.type_of(inner.span())?;
    let intrn = &*checker.type_db.intrn;
    if intrn.can_rhs_be_assigned(inner_ty, intrn.ty_null) {
        return None;
    }
    let inner_ty_display = intrn.display(inner_ty).to_string();

    let operator_span = Span::from_offset(node.span().end().checked_sub(1)?);

    let mut diagnostic = Diagnostic::warning_for(file_id, UnnecessaryNotNullAssertion)
        .with_annotations(vec![Annotation {
            span: operator_span,
            message: Some(format!(
                "this expression has non-null type `{inner_ty_display}`"
            )),
            is_primary: true,
            tags: vec![DiagnosticTag::Unnecessary],
        }])
        .with_help(format!(
            "remove `!`; `{inner_ty_display}` cannot be `null` here"
        ));

    // E014 replaces the whole repeated chain. Avoid an overlapping edit when both rules run.
    let e014_fixes_chain =
        matches!(inner, Expr::NotNull(_)) && checker.should_run(Rule::SeveralNotNullAssertions);
    if !e014_fixes_chain {
        diagnostic = diagnostic.with_fixes(vec![Fix {
            message: "remove the unnecessary `!`".to_string(),
            edits: vec![Edit {
                span: operator_span,
                replacement: String::new(),
                file_id,
            }],
            applicability: Applicability::Auto,
        }]);
    }
    checker.emit_diagnostic(diagnostic);

    Some(())
}
