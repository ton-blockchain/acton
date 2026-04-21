use crate::rules::diagnostic::{Annotation, Diagnostic};
use crate::rules::violation::Violation;
use crate::{Checker, FixAvailability};
use tolk_macros::ViolationMetadata;
use tolk_resolver::AstNodeSpanExt;
use tolk_resolver::file_index::FileId;
use tolk_syntax::{Call, Expr, HasName, InstanceArg};
use tree_sitter::Node;

/// ### What it does
/// Warns about `MyMessage { ... }.toCell()` inside `createMessage({ body: ... })` initializer.
///
/// ### Why is this bad?
/// `createMessage` already accepts a serializable object as `body` and decides whether to inline it
/// or wrap it into a reference cell automatically. Calling `.toCell()` manually is usually
/// unnecessary and can be worse for performance because it forces an extra cell creation even
/// when the compiler could inline a small body directly.
///
/// ### Example
/// ```tolk
/// struct Transfer {
///     amount: int32
/// }
///
/// fun send(dest: address) {
///     val msg = createMessage({
///         bounce: false,
///         value: ton("0.1"),
///         dest,
///         body: Transfer { amount: 1 }.toCell(),
///     });
/// }
/// ```
///
/// Use instead:
/// ```tolk
/// struct Transfer {
///     amount: int32
/// }
///
/// fun send(dest: address) {
///     val msg = createMessage({
///         bounce: false,
///         value: ton("0.1"),
///         dest,
///         body: Transfer { amount: 1 },
///     });
/// }
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct CreateMessageBodyToCell;

impl Violation for CreateMessageBodyToCell {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::None;

    fn message(&self) -> String {
        "explicit `.toCell()` on `createMessage` body is usually unnecessary and can hurt performance"
            .to_owned()
    }
}

pub fn check_call_expr(checker: &mut Checker, file_id: FileId, node: &Call) -> Option<()> {
    let name_node = node.callee_identifier()?;
    if !checker
        .file_db
        .text_matches(file_id, &name_node, "createMessage")
    {
        return None;
    }

    let body_arg = body_argument(node, checker, file_id)?;
    let body_value = body_arg.value()?;
    let body_to_cell_call = unwrap_to_cell_call(&body_value, checker, file_id)?;
    let qualifier = body_to_cell_call.callee_qualifier()?;

    if !expr_is_object_literal(&qualifier) {
        return None;
    }

    fire_diagnostic(checker, file_id, name_node, body_arg, body_to_cell_call.0);
    Some(())
}

fn body_argument<'tree>(
    node: &Call<'tree>,
    checker: &Checker,
    file_id: FileId,
) -> Option<InstanceArg<'tree>> {
    let first_arg = node.arguments().next()?;
    let Expr::ObjectLit(literal) = first_arg.expr()? else {
        return None;
    };

    literal.arguments().find(|arg| {
        let Some(name) = arg.name() else {
            return false;
        };
        checker.file_db.text_matches(file_id, &name, "body")
    })
}

fn unwrap_to_cell_call<'tree>(
    expr: &Expr<'tree>,
    checker: &Checker,
    file_id: FileId,
) -> Option<Call<'tree>> {
    match expr {
        Expr::Call(call) => {
            let method = call.callee_identifier()?;
            checker
                .file_db
                .text_matches(file_id, &method, "toCell")
                .then_some(*call)
        }
        Expr::Paren(paren) => unwrap_to_cell_call(&paren.inner()?, checker, file_id),
        _ => None,
    }
}

fn expr_is_object_literal(expr: &Expr<'_>) -> bool {
    match expr {
        Expr::ObjectLit(_) => true,
        Expr::Paren(paren) => paren
            .inner()
            .is_some_and(|inner| expr_is_object_literal(&inner)),
        _ => false,
    }
}

#[cold]
fn fire_diagnostic(
    checker: &mut Checker,
    file_id: FileId,
    create_message_name: Node,
    body_arg: InstanceArg,
    to_cell_call: Node,
) {
    let diagnostic = Diagnostic::warning_for(file_id, CreateMessageBodyToCell)
        .with_annotations(vec![
            Annotation {
                span: create_message_name.span(),
                message: Some("in this createMessage call".to_owned()),
                is_primary: false,
                tags: vec![],
            },
            Annotation {
                span: to_cell_call.span(),
                message: Some("remove the explicit `.toCell()` here".to_owned()),
                is_primary: true,
                tags: vec![],
            },
            Annotation {
                span: body_arg
                    .name()
                    .map_or_else(|| body_arg.span(), |name| name.span()),
                message: Some("`createMessage` already serializes `body` automatically".to_owned()),
                is_primary: false,
                tags: vec![],
            },
        ])
        .with_help("compiler can inline small bodies or wrap large ones into a cell automatically");
    checker.emit_diagnostic(diagnostic);
}
