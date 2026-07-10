use super::{TolkResolveSnapshot, TolkWorkspaceEngine, logical_path_for_uri};
use crate::{DocumentHighlight, DocumentHighlightKind, DocumentSnapshot, Position};
use tolk_resolver::{Resolved, SymbolKind};
use tolk_syntax::{Assign, AstNode, Call, DotAccess, SetAssign, TryFromNode};

impl TolkWorkspaceEngine {
    pub(super) fn document_highlights(
        &self,
        document: &DocumentSnapshot,
        position: Position,
    ) -> Vec<DocumentHighlight> {
        let snapshot = {
            let state = self.state.read().expect("Tolk workspace lock poisoned");
            state.latest_snapshot.clone()
        };
        let Some(snapshot) = snapshot else {
            return Vec::new();
        };
        let path = logical_path_for_uri(document.uri());
        let Some(file_id) = snapshot.project_index.get_file_by_path(&path) else {
            return Vec::new();
        };
        let offset = document
            .text_index()
            .position_to_offset(document.text(), position);
        let Some(resolved) = snapshot.resolved_at(file_id, offset) else {
            return Vec::new();
        };

        let mut highlights = snapshot
            .references_for_resolved(&resolved, true)
            .into_iter()
            .filter(|location| location.uri == *document.uri())
            .map(|location| {
                let kind = snapshot.highlight_kind(document, file_id, location.range);
                DocumentHighlight::new(location.range, kind)
            })
            .collect::<Vec<_>>();
        highlights.sort_by_key(|highlight| highlight.range.start);
        highlights
    }
}

impl TolkResolveSnapshot {
    fn highlight_kind(
        &self,
        document: &DocumentSnapshot,
        file_id: u32,
        range: crate::Range,
    ) -> DocumentHighlightKind {
        let Some(file) = self.file_db.get_by_id(file_id) else {
            return DocumentHighlightKind::Read;
        };
        let start = document
            .text_index()
            .position_to_offset(document.text(), range.start);
        let end = document
            .text_index()
            .position_to_offset(document.text(), range.end);
        let Some(identifier) = file
            .source()
            .tree
            .root_node()
            .descendant_for_byte_range(start, end)
        else {
            return DocumentHighlightKind::Read;
        };
        let Some(parent) = identifier.parent() else {
            return DocumentHighlightKind::Read;
        };

        if Assign::try_from_node(parent).is_ok_and(|assignment| assignment.is_lhs(&identifier))
            || SetAssign::try_from_node(parent)
                .is_ok_and(|assignment| assignment.is_lhs(&identifier))
        {
            return DocumentHighlightKind::Write;
        }

        self.mutating_call_kind(file_id, parent)
            .unwrap_or(DocumentHighlightKind::Read)
    }

    fn mutating_call_kind(
        &self,
        file_id: u32,
        parent: tree_sitter::Node<'_>,
    ) -> Option<DocumentHighlightKind> {
        let dot_access = DotAccess::try_from_node(parent).ok()?;
        let call = Call::try_from_node(dot_access.syntax().parent()?).ok()?;
        let callee = call.callee_identifier()?;
        let Resolved::Global(symbol_id) = self.resolved_at(file_id, callee.start_byte())? else {
            return None;
        };
        let symbol = self.project_index.resolve_symbol(symbol_id)?;

        matches!(
            symbol.kind,
            SymbolKind::Method {
                is_mutable: true,
                ..
            }
        )
        .then_some(DocumentHighlightKind::Write)
    }
}
