use crate::rules::diagnostic::{Annotation, Diagnostic};
use crate::rules::violation::Violation;
use crate::{Checker, FixAvailability};
use tolk_macros::ViolationMetadata;
use tolk_resolver::AstNodeSpanExt;
use tolk_resolver::file_index::{FileId, SymbolId};
use tolk_resolver::resolve_index::{LocalDefKind, Resolved};
use tolk_syntax::ast::expressions::{Assign, Call, Expr, VarDeclLhs};
use tolk_syntax::{AstNode, HasTreeSitterKind, TryFromNode, match_parents};
use tolk_ty::InferenceResult;

/// ### What it does
/// Checks for messages created with `createMessage(...)` but named as generic
/// `msg` or `message`.
///
/// ### Why is this bad?
/// Messages are first-class entities in Tolk and should have meaningful names.
/// Generic names like `msg` and `message` make code harder to read and reason
/// about.
///
/// ### Example
/// ```tolk twoslash
/// val msg = createMessage({ ... });
/// //  ^^^ S005: message should be properly named
/// msg.send(SEND_MODE_REGULAR);
/// ```
///
/// Use instead:
/// ```tolk
/// val deployMessage = createMessage({ ... });
/// deployMessage.send(SEND_MODE_REGULAR);
/// ```
///
/// ### Behavior notes
/// The rule checks local variables named exactly `msg` or `message` whose
/// initializer resolves to `createMessage(...)`.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct MessageShouldBeNamed;

impl Violation for MessageShouldBeNamed {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::None;

    fn message(&self) -> String {
        "message should be properly named".to_string()
    }
}

/// ### What it does
/// Checks for inline sending of newly created messages: `createMessage(...).send(...)`.
///
/// ### Why is this bad?
/// Creating and sending message in one expression hides message intent.
/// Prefer giving message a name before sending.
///
/// ### Example
/// ```tolk twoslash
/// createMessage({ ... }).send(SEND_MODE_REGULAR);
/// //                    ^^^^^ S006: create named message before send
/// ```
///
/// Use instead:
/// ```tolk
/// val deployMessage = createMessage({ ... });
/// deployMessage.send(SEND_MODE_REGULAR);
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct CreateMessageInlineSend;

impl Violation for CreateMessageInlineSend {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::None;

    fn message(&self) -> String {
        "create named message before send".to_string()
    }
}

pub fn check_file_for_message_name(checker: &mut Checker, file_id: FileId) -> Option<()> {
    let file = checker.file_db.get_by_id(file_id)?;
    let root = file.source().tree.root_node();
    let resolve_index = checker.resolve_index_for(file_id)?;
    let per_decl_inference = checker.body_types.get(&file_id)?;

    for local in &resolve_index.locals {
        if !matches!(local.kind, LocalDefKind::Var { .. }) {
            continue;
        }
        if local.name.as_ref() != "msg" && local.name.as_ref() != "message" {
            // fast path for 99.9% of locals
            continue;
        }

        let Some(def_node) =
            root.descendant_for_byte_range(local.def_span.start(), local.def_span.end())
        else {
            continue;
        };
        let Some(var_lhs) = match_parents!(def_node, VarDeclLhs(...)) else {
            continue;
        };
        let Some(assign_node) = var_lhs.syntax().parent() else {
            continue;
        };
        let Ok(assign) = Assign::try_from_node(assign_node) else {
            continue;
        };
        let Some(rhs) = assign.right() else {
            continue;
        };

        let Some(owner) = file.find_symbol_at(local.def_span.start()) else {
            continue;
        };
        let Some(owner_inference) = per_decl_inference.get(&owner.id) else {
            continue;
        };

        if !is_create_message_expr(checker, file_id, &rhs, owner_inference) {
            continue;
        }

        let diagnostic = Diagnostic::warning_for(file_id, MessageShouldBeNamed)
            .with_annotations(vec![Annotation {
                span: local.def_span,
                message: Some("message should be properly named, not `msg`".to_owned()),
                is_primary: true,
                tags: vec![],
            }])
            .with_help(
                "use a descriptive message name, for example `deployMessage` or `transferMessage`",
            );
        checker.emit_diagnostic(diagnostic);
    }

    Some(())
}

pub fn check_call_for_inline_send(
    checker: &mut Checker,
    file_id: FileId,
    call: &Call,
    current_inference: &InferenceResult,
) -> Option<()> {
    if !is_send_call(checker, file_id, call, current_inference) {
        return None;
    }

    let qualifier = call.callee_qualifier()?;
    if !is_create_message_expr(checker, file_id, &qualifier, current_inference) {
        return None;
    }

    let diagnostic = Diagnostic::warning_for(file_id, CreateMessageInlineSend)
        .with_annotations(vec![Annotation {
            span: call.span(),
            message: Some(
                "avoid `createMessage(...).send(...)`, create named message first".to_owned(),
            ),
            is_primary: true,
            tags: vec![],
        }])
        .with_help(
            "split into two statements: create message in a variable, then call `.send(...)`",
        );
    checker.emit_diagnostic(diagnostic);

    None
}

fn is_send_call(
    checker: &Checker,
    file_id: FileId,
    call: &Call,
    current_inference: &InferenceResult,
) -> bool {
    let Some(symbol_id) = resolve_call_symbol(checker, file_id, call, current_inference) else {
        return false;
    };
    let Some(symbol) = checker.type_db.project_index.resolve_symbol(symbol_id) else {
        return false;
    };
    symbol.name.as_ref() == "send"
}

fn is_create_message_expr(
    checker: &Checker,
    file_id: FileId,
    expr: &Expr,
    current_inference: &InferenceResult,
) -> bool {
    match expr {
        Expr::Call(call) => {
            let Some(symbol_id) = resolve_call_symbol(checker, file_id, call, current_inference)
            else {
                return false;
            };
            let Some(symbol) = checker.type_db.project_index.resolve_symbol(symbol_id) else {
                return false;
            };
            symbol.name.as_ref() == "createMessage"
        }
        Expr::Paren(paren) => paren.inner().is_some_and(|inner| {
            is_create_message_expr(checker, file_id, &inner, current_inference)
        }),
        _ => false,
    }
}

fn resolve_call_symbol(
    checker: &Checker,
    file_id: FileId,
    call: &Call,
    current_inference: &InferenceResult,
) -> Option<SymbolId> {
    let callee_ident = call.callee_identifier()?;
    let resolve_index = checker.resolve_index_for(file_id);

    if let Some(resolve_index) = resolve_index
        && let Some(name_use) = resolve_index.find_use(callee_ident.start_byte())
        && let Resolved::Global(symbol_id) = name_use.resolved
    {
        return Some(symbol_id);
    }

    if let Some(name_use) = current_inference.resolve(callee_ident.span())
        && let Resolved::Global(symbol_id) = name_use.resolved
    {
        return Some(symbol_id);
    }

    None
}
