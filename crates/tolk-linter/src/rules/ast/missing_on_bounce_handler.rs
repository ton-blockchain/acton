use crate::rules::diagnostic::{Annotation, Diagnostic, Severity};
use crate::rules::violation::{FixAvailability, Violation, ViolationMetadata};
use crate::Checker;
use tolk_macros::ViolationMetadata;
use tolk_resolver::file_index::{FileId, SymbolKind};

/// ### What it does
/// Checks that contracts with bounceable messages have an `onBouncedMessage` handler.
///
/// ### Why is this bad?
/// When sending a message without `bounce: BounceMode.NoBounce`, the message can bounce
/// back if the destination contract fails to process it. Without an `onBouncedMessage`
/// handler, bounced messages will be silently ignored, which may lead to an inconsistent state.
///
/// ### Example
/// ```tolk
/// fun onInternalMessage(in: InMessage) {
///     val msg = createMessage({
///         bounce: BounceMode.Bounce,  // or just omitting bounce field
///         dest: someAddress,
///         value: ton("1"),
///         body: SomeOp {},
///     });
///     msg.send();
/// }
/// // Missing onBouncedMessage handler!
/// ```
///
/// Use instead:
/// ```tolk
/// fun onInternalMessage(in: InMessage) {
///     val msg = createMessage({
///         bounce: BounceMode.NoBounce,  // explicitly disable bouncing
///         dest: someAddress,
///         value: ton("1"),
///         body: SomeOp {},
///     });
///     msg.send();
/// }
/// ```
///
/// Or add a bounce handler:
/// ```tolk
/// fun onInternalMessage(in: InMessage) {
///     val msg = createMessage({
///         dest: someAddress,
///         value: ton("1"),
///         body: SomeOp {},
///     });
///     msg.send();
/// }
///
/// fun onBouncedMessage(in: InMessageBounced) {
///     // Handle bounced messages
/// }
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct MissingOnBounceHandler;

impl Violation for MissingOnBounceHandler {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::None;

    fn message(&self) -> String {
        "message is marked bounceable, but onBouncedMessage handler does not exist".to_string()
    }
}

/// Checks the entire project for missing onBouncedMessage handlers.
/// This should be called after all files have been processed.
pub fn check_project(checker: &mut Checker) {
    // For each file, check if it has onInternalMessage but no onBouncedMessage
    let file_ids: Vec<FileId> = checker.file_db.iter().map(|f| f.id()).collect();

    for file_id in file_ids {
        check_file(checker, file_id);
    }
}

fn check_file(checker: &mut Checker, file_id: FileId) {
    let Some(file_info) = checker.file_db.get_by_id(file_id) else {
        return;
    };

    // Check if this is a workspace file (skip stdlib)
    if !file_info.is_workspace_file() {
        return;
    }

    let decls = &file_info.index().decls;

    // Find onInternalMessage and onBouncedMessage in this file
    let has_on_internal_message = decls.iter().any(|d| d.name.as_ref() == "onInternalMessage");
    let has_on_bounced_message = decls.iter().any(|d| d.name.as_ref() == "onBouncedMessage");

    // If no onInternalMessage, nothing to check
    if !has_on_internal_message {
        return;
    }

    // If onBouncedMessage exists, no problem
    if has_on_bounced_message {
        return;
    }

    // Find onInternalMessage symbol
    let Some(on_internal_symbol) = decls.iter().find(|d| d.name.as_ref() == "onInternalMessage")
    else {
        return;
    };

    // Check if any function reachable from onInternalMessage calls createMessage with bounce != false
    let bounceable_calls = find_bounceable_create_message_calls(checker, file_id, on_internal_symbol.id);

    for (call_span, call_file_id) in bounceable_calls {
        let diagnostic = Diagnostic {
            file_id: call_file_id,
            severity: Severity::Warning,
            name: MissingOnBounceHandler::rule().name(),
            code: MissingOnBounceHandler::code().map(|c| c.to_string()),
            message: MissingOnBounceHandler.message(),
            annotations: vec![Annotation {
                span: call_span,
                message: Some("this message may bounce".to_string()),
                is_primary: true,
                tags: vec![],
            }],
            fixes: vec![],
            help: Some(
                "add an `onBouncedMessage` handler to handle bounced messages, \
                 or set `bounce: BounceMode.NoBounce` if bouncing is not expected"
                    .to_string(),
            ),
        };
        checker.emit_diagnostic(MissingOnBounceHandler::rule(), diagnostic);
    }
}

use rustc_hash::FxHashSet;
use tolk_resolver::file_index::{Span, SymbolId};

/// Finds all createMessage calls reachable from the given function
/// that have bounceable mode (bounce != BounceMode.NoBounce).
/// Returns tuples of (bounce_field_span, file_id) for highlighting.
fn find_bounceable_create_message_calls(
    checker: &Checker,
    _file_id: FileId,
    start_symbol: SymbolId,
) -> Vec<(Span, FileId)> {
    let mut result = Vec::new();
    let mut visited = FxHashSet::default();
    let mut stack = vec![start_symbol];

    while let Some(caller) = stack.pop() {
        if !visited.insert(caller) {
            continue;
        }

        // Get all calls from this function
        if let Some(edges) = checker.calls_from(caller) {
            for edge in edges {
                let callee = edge.callee;

                // Check if this is a call to createMessage
                if let Some(callee_symbol) = checker.type_db.project_index.resolve_symbol(callee) {
                    if callee_symbol.name.as_ref() == "createMessage" {
                        // Check if bounce field is set to NoBounce
                        if let Some(bounce_span) =
                            get_bounceable_field_span(checker, caller.file_id, edge.span)
                        {
                            result.push((bounce_span, caller.file_id));
                        }
                        continue;
                    }

                    // If it's a user function, add to stack to explore further
                    if matches!(
                        callee_symbol.kind,
                        SymbolKind::Function { .. } | SymbolKind::Method { .. }
                    ) {
                        stack.push(callee);
                    }
                }
            }
        }
    }

    result
}

/// Returns the span of the bounce field if the message is bounceable.
/// Returns None if bounce is set to BounceMode.NoBounce (no error needed).
/// Returns Some(span) with the bounce field span (or call span if bounce not specified).
fn get_bounceable_field_span(checker: &Checker, file_id: FileId, call_span: Span) -> Option<Span> {
    use tolk_resolver::AstNodeSpanExt;
    use tolk_syntax::{AstNode, Call, HasName, ObjectLit, TryFromNode};

    let file_info = checker.file_db.get_by_id(file_id)?;

    let root = file_info.source().tree.root_node();
    let source = file_info.source().source.as_ref();

    // Find the call node at this span
    let call_node = root.descendant_for_byte_range(call_span.start(), call_span.end())?;

    // Walk up to find the Call node
    let mut current = Some(call_node);
    while let Some(node) = current {
        if let Ok(call) = Call::try_from_node(node) {
            // Find the ObjectLit argument (the struct literal)
            for arg in call.arguments() {
                if let Some(expr) = arg.expr() {
                    if let Ok(obj_lit) = ObjectLit::try_from_node(expr.syntax()) {
                        // Look for bounce field
                        for field in obj_lit.arguments() {
                            if let Some(name) = field.name() {
                                if name.text(source) == "bounce" {
                                    // Check if value is BounceMode.NoBounce
                                    if let Some(value) = field.value() {
                                        let value_text = value
                                            .syntax()
                                            .utf8_text(source.as_bytes())
                                            .unwrap_or("");
                                        if value_text.trim() == "BounceMode.NoBounce" {
                                            // No error needed
                                            return None;
                                        }
                                        // Bounceable - return the field span for highlighting
                                        return Some(field.span());
                                    }
                                }
                            }
                        }
                        // bounce field not found - message is bounceable by default
                        // Return the call span since there's no bounce field to highlight
                        return Some(call_span);
                    }
                }
            }
            break;
        }
        current = node.parent();
    }

    // Could not find call structure - assume bounceable
    Some(call_span)
}
