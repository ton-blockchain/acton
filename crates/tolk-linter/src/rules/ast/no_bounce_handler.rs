use crate::diagnostic::{Annotation, Diagnostic, Severity};
use crate::{Checker, FixAvailability, Violation, ViolationMetadata};
use rustc_hash::FxHashSet;
use std::collections::VecDeque;
use tolk_macros::ViolationMetadata;
use tolk_resolver::{AstNodeSpanExt, FileId, SymbolId};
use tolk_syntax::{Call, Expr, HasName, InstanceArg};
use tree_sitter::Node;

#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct NoBounceHandler;

impl Violation for NoBounceHandler {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::None;

    fn message(&self) -> String {
        "contract sends a message that may bounce but `onBouncedMessage` handler doesn't exist"
            .to_string()
    }
}

pub fn check_call_expr(
    checker: &mut Checker,
    file_id: FileId,
    node: &Call,
    current_decl: Option<SymbolId>,
) -> Option<()> {
    let name_node = node.callee_identifier()?;
    let func_name = checker.file_db.text_of(file_id, &name_node)?;
    if func_name != "createMessage" {
        return None;
    };
    let arg = node.arguments().first()?;
    let Expr::ObjectLit(literal) = arg.expr()? else {
        return None;
    };
    let bounce_arg = literal.arguments().find(|arg| {
        let Some(name) = arg.name() else {
            return false;
        };
        checker.file_db.text_of(file_id, &name) == Some("bounce".into())
    })?;
    let Expr::DotAccess(dot_access) = bounce_arg.value()? else {
        return None;
    };

    let Expr::Ident(enum_value) = dot_access.obj()? else {
        return None;
    };

    let enum_value = checker.file_db.text_of(file_id, &enum_value)?;
    if enum_value == "NoBounce" {
        return None;
    }

    let current_decl = current_decl?;

    // BFS up the inverted call graph to find all onInternalMessage callers
    let mut visited = FxHashSet::default();
    let mut queue = VecDeque::new();
    queue.push_back(current_decl);
    visited.insert(current_decl);

    let mut on_internal_message_ids: Vec<SymbolId> = Vec::new();

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
    let diagnostic = Diagnostic {
        file_id,
        severity: Severity::Warning,
        name: NoBounceHandler::rule().name(),
        code: NoBounceHandler::code().map(|c| c.to_string()),
        message: NoBounceHandler.message(),
        annotations: vec![
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
        ],
        fixes: vec![],
        help: Some("Add bounce handler or use `BounceMode.NoBounce`".to_string()),
    };
    checker.emit_diagnostic(NoBounceHandler::rule(), diagnostic);
}
