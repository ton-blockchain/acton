use crate::rules::diagnostic::{Annotation, Diagnostic};
use crate::rules::violation::Violation;
use crate::{Checker, FixAvailability};
use tolk_macros::ViolationMetadata;
use tolk_resolver::{AstNodeSpanExt, FileId};
use tolk_syntax::{Expr, Paren};

/// ### What it does
/// Warns when an expression statement computes a value that is immediately discarded.
///
/// ### Why is this bad?
/// Such expressions usually have no effect and often indicate a typo, such as using `!=`
/// instead of `=` or `+=`.
///
/// ### Example
/// ```tolk
/// fun main(a: int, b: int) {
///     a != b;
/// }
/// ```
///
/// Use instead:
/// ```tolk
/// fun main(mutate a: int, b: int) {
///     a -= b;
/// }
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct UnusedExpression;

impl Violation for UnusedExpression {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::None;

    fn message(&self) -> String {
        "unused expression has no effect".to_owned()
    }
}

pub fn check_expr_stmt(checker: &mut Checker, file_id: FileId, expr: &Expr<'_>) -> Option<()> {
    if !is_effect_free_expression(expr) {
        return None;
    }

    let diagnostic = Diagnostic::warning_for(file_id, UnusedExpression)
        .with_annotations(vec![Annotation {
            span: expr.span(),
            message: Some("expression result is ignored".to_owned()),
            is_primary: true,
            tags: vec![],
        }])
        .with_help(
            "remove the expression or replace it with an assignment / mutation if you meant to update state",
        );
    checker.emit_diagnostic(diagnostic);
    Some(())
}

fn is_effect_free_expression(expr: &Expr<'_>) -> bool {
    match expr {
        Expr::VarDeclLhs(_)
        | Expr::Assign(_)
        | Expr::SetAssign(_)
        | Expr::Ternary(_)
        | Expr::Match(_)
        | Expr::Call(_)
        | Expr::Unmapped(_) => false,
        Expr::Paren(paren) => is_effect_free_paren(*paren),
        Expr::Bin(_)
        | Expr::Unary(_)
        | Expr::Lazy(_)
        | Expr::AsCast(_)
        | Expr::IsType(_)
        | Expr::NotNull(_)
        | Expr::DotAccess(_)
        | Expr::Instantiation(_)
        | Expr::ObjectLit(_)
        | Expr::Tensor(_)
        | Expr::Tuple(_)
        | Expr::Lambda(_)
        | Expr::NumberLit(_)
        | Expr::StringLit(_)
        | Expr::BoolLit(_)
        | Expr::NullLit(_)
        | Expr::Ident(_)
        | Expr::Underscore(_) => true,
    }
}

fn is_effect_free_paren(paren: Paren<'_>) -> bool {
    paren
        .inner()
        .is_some_and(|inner| is_effect_free_expression(&inner))
}

#[cfg(test)]
mod tests {
    use super::is_effect_free_expression;
    use tolk_syntax::{ExprStmt, NodeTraversalExt, TryFromNode, parse};

    fn with_first_expr(code: &str, f: impl FnOnce(tolk_syntax::Expr<'_>)) {
        let file = parse(code).expect("parse failed");
        let expr_stmt = file
            .root_node()
            .traverse()
            .find_map(|node| ExprStmt::try_from_node(node).ok())
            .expect("expression statement not found");
        let expr = expr_stmt
            .expr()
            .expect("expression statement should have an expression");
        f(expr);
    }

    #[test]
    fn lazy_expression_is_treated_as_effect_free() {
        with_first_expr(
            r"
                fun main() {
                    lazy expensive();
                }
            ",
            |expr| {
                assert!(is_effect_free_expression(&expr));
            },
        );
    }
}
