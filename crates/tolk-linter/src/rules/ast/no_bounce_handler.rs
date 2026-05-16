use crate::diagnostic::{Annotation, Diagnostic};
use crate::{Checker, FixAvailability, Violation};
use rustc_hash::{FxBuildHasher, FxHashSet};
use std::collections::VecDeque;
use tolk_macros::ViolationMetadata;
use tolk_resolver::{AstNodeSpanExt, FileId, SymbolId};
use tolk_syntax::{Call, Expr, HasName, InstanceArg};
use tree_sitter::Node;

/// ### What it does
/// Reports `createMessage({ bounce: BounceMode.<...> })` calls reachable from
/// `onInternalMessage` when the contract file does not declare
/// `onBouncedMessage`.
///
/// ### Why is this bad?
/// If an outgoing internal message can bounce, the bounced transaction returns
/// to `onBouncedMessage`. Without that handler, the contract cannot restore
/// state, refund accounting, or observe the failure.
///
/// ### Example
/// ```tolk
/// fun onInternalMessage(in: InMessage) {
///     sendRefund(in.senderAddress);
/// }
///
/// fun sendRefund(dest: address) {
///     val refundMessage = createMessage({
///         bounce: BounceMode.Only256BitsOfBody,
///         //      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^ E007: contract sends a message that may bounce but `onBouncedMessage` handler doesn't exist
///         value: ton("0.1"),
///         dest,
///     });
///     refundMessage.send(SEND_MODE_REGULAR);
/// }
/// ```
///
/// Use instead:
/// ```tolk
/// fun onBouncedMessage(in: InMessageBounced) {
///     // handle bounced refund message
/// }
/// ```
///
/// Or send explicitly without bounce handling:
/// ```tolk
/// val refundMessage = createMessage({
///     bounce: BounceMode.NoBounce,
///     value: ton("0.1"),
///     dest,
///     body: beginCell().endCell(),
/// });
/// ```
///
/// ### Behavior notes
/// - `BounceMode.NoBounce` is ignored by this rule.
/// - Legacy boolean `bounce` values are not currently matched; the check looks
///   for `BounceMode` field access.
/// - The handler must be declared as `onBouncedMessage` in the same file as the
///   `onInternalMessage` entrypoint.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct NoBounceHandler;

impl Violation for NoBounceHandler {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::None;

    fn message(&self) -> String {
        "contract sends a message that may bounce but `onBouncedMessage` handler doesn't exist"
            .to_owned()
    }
}

pub fn check_call_expr(
    checker: &mut Checker,
    file_id: FileId,
    node: &Call,
    current_decl: Option<SymbolId>,
) -> Option<()> {
    let name_node = node.callee_identifier()?;
    if !checker
        .file_db
        .text_matches(file_id, &name_node, "createMessage")
    {
        return None;
    }
    let arg = node.arguments().first()?;
    let Expr::ObjectLit(literal) = arg.expr()? else {
        return None;
    };
    let bounce_arg = literal.arguments().find(|arg| {
        let Some(name) = arg.name() else {
            return false;
        };
        checker.file_db.text_matches(file_id, &name, "bounce")
    })?;
    let Expr::DotAccess(dot_access) = bounce_arg.value()? else {
        return None;
    };

    let enum_value = dot_access.field()?;

    if checker
        .file_db
        .text_matches(file_id, &enum_value, "NoBounce")
    {
        return None;
    }

    let current_decl = current_decl?;

    // BFS up the inverted call graph to find all onInternalMessage callers
    let mut visited = FxHashSet::with_capacity_and_hasher(10, FxBuildHasher);
    let mut queue = VecDeque::with_capacity(10);
    queue.push_back(current_decl);
    visited.insert(current_decl);

    let mut on_internal_message_ids: Vec<SymbolId> = Vec::with_capacity(2);

    while let Some(sym_id) = queue.pop_front() {
        let Some(callers) = checker.type_db.inverted_call_graph.get(&sym_id) else {
            continue;
        };
        for &caller_id in callers {
            if !visited.insert(caller_id) {
                continue;
            }
            if let Some(symbol) = checker.type_db.project_index.resolve_symbol(caller_id)
                && &*symbol.name == "onInternalMessage"
            {
                on_internal_message_ids.push(caller_id);
            }
            queue.push_back(caller_id);
        }
    }

    // For each onInternalMessage, find the onBouncedMessage in the same file
    for &on_internal_id in &on_internal_message_ids {
        let mut have_found_bounce = false;
        let contract_file_id = on_internal_id.file_id;
        if let Some(file_index) = checker
            .type_db
            .project_index
            .get_file_index(contract_file_id)
            && file_index
                .decls
                .iter()
                .any(|d| &*d.name == "onBouncedMessage")
        {
            have_found_bounce = true;
        }
        if !have_found_bounce {
            fire_diagnostic(checker, name_node, bounce_arg, file_id);
        }
    }
    Some(())
}

#[cold]
fn fire_diagnostic(
    checker: &mut Checker,
    name_node: Node,
    bounce_arg: InstanceArg,
    file_id: FileId,
) {
    let diagnostic = Diagnostic::warning_for(file_id, NoBounceHandler)
        .with_annotations(vec![
            Annotation {
                span: name_node.span(),
                message: Some("In this createMessage call".to_string()),
                is_primary: false,
                tags: vec![],
            },
            Annotation {
                span: bounce_arg.span(),
                message: Some(
                    "message is marked bounce, but bounce handler doesn't exist".to_string(),
                ),
                is_primary: true,
                tags: vec![],
            },
        ])
        .with_help("Add bounce handler or use `BounceMode.NoBounce`");
    checker.emit_diagnostic(diagnostic);
}
