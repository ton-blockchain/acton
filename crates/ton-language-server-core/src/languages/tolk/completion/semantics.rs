use super::TolkCompletionProviderContext;
use crate::languages::tolk::TolkResolveSnapshot;
use tolk_resolver::resolve_index::LocalDef;
use tolk_resolver::symbol_resolver::GlobalEnv;
use tolk_resolver::{FileId, Span, Symbol};
use tolk_syntax::{StructField, TryFromNode};
use tolk_ty::TyId;
use tree_sitter::Node;

pub(super) fn visit_visible_locals(
    context: &TolkCompletionProviderContext<'_>,
    mut visit: impl FnMut(&LocalDef),
) {
    let Some(file) = context.snapshot.file_db.get_by_id(context.file_id) else {
        return;
    };
    let Some(resolve_index) = context
        .snapshot
        .project_index
        .get_resolved_uses(context.file_id)
    else {
        return;
    };
    for local in &resolve_index.locals {
        if local_is_visible(file.source().tree.root_node(), local, context.syntax.offset) {
            visit(local);
        }
    }
}

pub(super) fn visit_visible_globals(
    context: &TolkCompletionProviderContext<'_>,
    mut visit: impl FnMut(&Symbol),
) {
    let env = GlobalEnv::new(&context.snapshot.project_index, context.file_id);
    for symbol_ids in env.visible.values() {
        for &symbol_id in symbol_ids {
            if let Some(symbol) = context.snapshot.project_index.resolve_symbol(symbol_id) {
                visit(symbol);
            }
        }
    }
}

pub(super) fn type_of_node(
    context: &TolkCompletionProviderContext<'_>,
    node: Node<'_>,
) -> Option<TyId> {
    let file = context.snapshot.file_db.get_by_id(context.file_id)?;
    let symbol = file.find_symbol_at(node.start_byte())?;
    let inference = context
        .snapshot
        .all_body_types
        .get(&context.file_id)?
        .get(&symbol.id)?;
    inference.type_of(Span::from_syntax(&node)).or_else(|| {
        let original = original_node(file.source().tree.root_node(), node)?;
        inference.type_of(Span::from_syntax(&original))
    })
}

pub(super) fn local_type(
    context: &TolkCompletionProviderContext<'_>,
    local: &LocalDef,
) -> Option<TyId> {
    let file = context.snapshot.file_db.get_by_id(context.file_id)?;
    let symbol = file.find_symbol_at(local.def_span.start())?;
    context
        .snapshot
        .all_body_types
        .get(&context.file_id)?
        .get(&symbol.id)?
        .type_of(local.def_span)
}

pub(super) fn raw_text(
    snapshot: &TolkResolveSnapshot,
    file_id: FileId,
    span: Span,
) -> Option<String> {
    let file = snapshot.file_db.get_by_id(file_id)?;
    file.source()
        .source
        .get(span.start()..span.end())
        .map(str::to_owned)
}

pub(super) fn struct_field_is_private(snapshot: &TolkResolveSnapshot, field: &Symbol) -> bool {
    let Some(file) = snapshot.file_db.get_by_id(field.id.file_id) else {
        return false;
    };
    let Some(name) = file
        .source()
        .tree
        .root_node()
        .descendant_for_byte_range(field.name_span.start(), field.name_span.end())
    else {
        return false;
    };
    name.parent()
        .and_then(|node| StructField::try_from_node(node).ok())
        .and_then(|field| field.modifiers())
        .is_some_and(|modifiers| modifiers.has_private())
}

fn local_is_visible(root: Node<'_>, local: &LocalDef, offset: usize) -> bool {
    if local.def_span.start() > offset {
        return false;
    }
    let Some(node) = root.descendant_for_byte_range(local.def_span.start(), local.def_span.end())
    else {
        return false;
    };
    let Some(scope) = local_scope(node) else {
        return false;
    };
    scope.start_byte() <= offset && offset <= scope.end_byte()
}

fn local_scope(mut node: Node<'_>) -> Option<Node<'_>> {
    loop {
        if matches!(
            node.kind(),
            "block_statement"
                | "catch_clause"
                | "function_declaration"
                | "method_declaration"
                | "get_method_declaration"
                | "lambda_expression"
                | "struct_declaration"
                | "type_alias_declaration"
        ) {
            return Some(node);
        }
        node = node.parent()?;
    }
}

fn original_node<'tree>(root: Node<'tree>, synthetic: Node<'_>) -> Option<Node<'tree>> {
    let start = synthetic.start_byte();
    let mut node = root.descendant_for_byte_range(start, start.saturating_add(1))?;
    loop {
        if node.start_byte() == start && node.kind() == synthetic.kind() {
            return Some(node);
        }
        node = node.parent()?;
    }
}
