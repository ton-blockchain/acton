use crate::rules::diagnostic::{Annotation, Applicability, Diagnostic, Edit, Fix};
use crate::rules::violation::Violation;
use crate::{Checker, FixAvailability};
use tolk_macros::ViolationMetadata;
use tolk_resolver::AstNodeSpanExt;
use tolk_resolver::file_index::FileId;
use tolk_syntax::ast::expressions::{Expr, NotNull};
use tolk_syntax::{AstNode, TryFromNode};

/// ### What it does
/// Detects repeated not-null assertions like `foo!!`.
///
/// ### Why is this bad?
/// Repeating `!` is redundant: after the first not-null assertion the value is already non-null.
///
/// ### Example
/// ```tolk twoslash
/// val x = foo!!;
/// //         ^^ E019: several not-null assertions in a row
/// ```
///
/// Use instead:
/// ```tolk
/// val x = foo!;
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct SeveralNotNullAssertions;

impl Violation for SeveralNotNullAssertions {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Always;

    fn message(&self) -> String {
        "several not-null assertions in a row".to_string()
    }
}

pub fn check_not_null(checker: &mut Checker, file_id: FileId, node: &NotNull) -> Option<()> {
    // Report once per chain (`foo!!!`) on the outermost not-null node only.
    if node
        .syntax()
        .parent()
        .is_some_and(|parent| NotNull::try_from_node(parent).is_ok())
    {
        return None;
    }

    let Some(Expr::NotNull(inner_not_null)) = node.inner() else {
        return None;
    };

    let file = checker.file_db.get_by_id(file_id)?;
    let source = file.source().source.as_ref();
    let base_expr = deepest_non_not_null_expr(Expr::NotNull(inner_not_null))?;
    let replacement = format!("{}!", base_expr.text(source));

    let diagnostic = Diagnostic::warning_for(file_id, SeveralNotNullAssertions)
        .with_annotations(vec![Annotation {
            span: node.span(),
            message: Some("redundant repeated not-null assertion".to_string()),
            is_primary: true,
            tags: vec![],
        }])
        .with_fixes(vec![Fix {
            message: "keep a single not-null assertion".to_string(),
            edits: vec![Edit {
                span: node.span(),
                replacement,
                file_id,
            }],
            applicability: Applicability::Auto,
        }])
        .with_help("remove duplicate `!` and keep only one not-null assertion");
    checker.emit_diagnostic(diagnostic);

    Some(())
}

fn deepest_non_not_null_expr(mut expr: Expr) -> Option<Expr> {
    loop {
        match expr {
            Expr::NotNull(not_null) => {
                expr = not_null.inner()?;
            }
            _ => return Some(expr),
        }
    }
}
