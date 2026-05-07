use crate::rules::diagnostic::{Annotation, Diagnostic};
use crate::rules::violation::Violation;
use crate::{Checker, FixAvailability};
use tolk_macros::ViolationMetadata;
use tolk_resolver::file_index::{FileId, Span, SymbolId};
use tolk_resolver::resolve_index::Resolved;
use tolk_resolver::{AstNodeSpanExt, SymbolKind};
use tolk_syntax::AstNode;
use tolk_syntax::ast::expressions::Expr;
use tree_sitter::Node;

/// ### What it does
/// Warns when symbolic throw codes are referenced as bare constants.
///
/// ### Why is this bad?
/// Using `Errors.SomeName` keeps custom exit codes consistent across contracts, tests, and compiler ABI output.
/// Bare constants such as `ERR_NOT_OWNER` are harder to discover and produce inconsistent symbolic names.
///
/// ### Example
/// ```tolk twoslash
/// struct Storage {
///     ownerAddress: address
/// }
///
/// const ERR_NOT_OWNER = 401
///
/// fun onInternalMessage(in: InMessage) {
///     val storage = lazy Storage.load();
///     val isOwner = in.senderAddress == storage.ownerAddress;
///     assert (isOwner) throw ERR_NOT_OWNER;
///     //                     ^^^^^^^^^^^^^ E028: throw code should use `Errors.<Name>`
/// }
/// ```
///
/// Use instead:
/// ```tolk
/// enum Errors {
///     NotOwner = 401
/// }
///
/// struct Storage {
///     ownerAddress: address
/// }
///
/// fun onInternalMessage(in: InMessage) {
///     val storage = lazy Storage.load();
///     val isOwner = in.senderAddress == storage.ownerAddress;
///     assert (isOwner) throw Errors.NotOwner;
/// }
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct ThrowRequiresErrorsEnum;

impl Violation for ThrowRequiresErrorsEnum {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::None;

    fn message(&self) -> String {
        "throw code should use `Errors.<Name>`".to_owned()
    }
}

pub fn check_throw_expr(checker: &mut Checker, file_id: FileId, expr: &Expr<'_>) -> Option<()> {
    let code_expr = unwrap_throw_code_expr(expr)?;

    let detail = match &code_expr {
        Expr::Ident(ident) => {
            let symbol_id = resolve_global_symbol_id(checker, file_id, ident.syntax())?;
            let symbol = checker.type_db.project_index.resolve_symbol(symbol_id)?;
            match symbol.kind {
                SymbolKind::Constant => {
                    format!("use `Errors.<Name>` instead of constant `{}`", symbol.name)
                }
                _ => return None,
            }
        }
        _ => return None,
    };

    let diagnostic = Diagnostic::warning_for(file_id, ThrowRequiresErrorsEnum)
        .with_annotations(vec![Annotation {
            span: code_expr.span(),
            message: Some(detail),
            is_primary: true,
            tags: vec![],
        }])
        .with_help(
            "declare symbolic exit codes in `enum Errors` and reference them as `Errors.<Name>` in `throw` expressions",
        );
    checker.emit_diagnostic(diagnostic);
    Some(())
}

fn unwrap_throw_code_expr<'tree>(expr: &Expr<'tree>) -> Option<Expr<'tree>> {
    match expr {
        Expr::Paren(paren) => unwrap_throw_code_expr(&paren.inner()?),
        Expr::AsCast(as_cast) => unwrap_throw_code_expr(&as_cast.expr()?),
        Expr::Tensor(tensor) => unwrap_throw_code_expr(&tensor.elements().next()?),
        _ => Some(Expr::from(expr.syntax())),
    }
}

fn resolve_global_symbol_id(
    checker: &Checker,
    file_id: FileId,
    node: Node<'_>,
) -> Option<SymbolId> {
    let span = Span::from_syntax(&node);

    if let Some(resolve_index) = checker.resolve_index_for(file_id)
        && let Some(name_use) = resolve_index.find_use(span.start())
        && let Resolved::Global(symbol_id) = name_use.resolved
    {
        return Some(symbol_id);
    }

    None
}
