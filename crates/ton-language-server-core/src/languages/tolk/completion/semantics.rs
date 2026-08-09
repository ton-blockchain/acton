use super::TolkCompletionProviderContext;
use crate::languages::tolk::TolkResolveSnapshot;
use tolk_resolver::resolve_index::LocalDef;
use tolk_resolver::{FileId, Span, Symbol};
use tolk_syntax::AstNode;
use tolk_ty::TyId;
use tree_sitter::Node;

impl<'a> TolkCompletionProviderContext<'a> {
    pub(super) fn visible_locals(&'a self) -> Box<dyn Iterator<Item = &'a LocalDef> + 'a> {
        if self.snapshot.file_db.get_by_id(self.file_id).is_none() {
            return Box::new(std::iter::empty());
        }
        let Some(resolve_index) = self.snapshot.project_index.get_resolved_uses(self.file_id)
        else {
            return Box::new(std::iter::empty());
        };

        let snapshot = self.snapshot;
        let file_id = self.file_id;
        let offset = self.syntax.offset;
        Box::new(resolve_index.locals.iter().filter(move |local| {
            snapshot
                .file_db
                .get_by_id(file_id)
                .is_some_and(|file| local_is_visible(file.source().tree.root_node(), local, offset))
        }))
    }

    pub(super) fn visible_globals(&'a self) -> Box<dyn Iterator<Item = &'a Symbol> + 'a> {
        let symbol_ids = self
            .visible_globals
            .visible
            .values()
            .flat_map(|symbol_ids| symbol_ids.iter());
        Box::new(
            symbol_ids
                .filter_map(|symbol_id| self.snapshot.project_index.resolve_symbol(*symbol_id)),
        )
    }

    pub(super) fn type_of_node<'tree, N>(&self, node: N) -> Option<TyId>
    where
        N: AstNode<'tree>,
    {
        let syntax = node.syntax();
        self.snapshot.inferred_type_of_node(self.file_id, syntax)
    }
}

pub(super) fn local_type(
    context: &TolkCompletionProviderContext<'_>,
    local: &LocalDef,
) -> Option<TyId> {
    context.snapshot.local_type(local)
}

pub(super) fn raw_text(
    snapshot: &TolkResolveSnapshot,
    file_id: FileId,
    span: Span,
) -> Option<String> {
    let file = snapshot.file_db.get_by_id(file_id)?;
    Some(file.text_at(span).to_owned())
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
