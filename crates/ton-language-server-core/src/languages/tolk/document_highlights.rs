use super::{TolkWorkspaceEngine, range_for_span};
use crate::{DocumentHighlight, DocumentHighlightKind, DocumentSnapshot, Position};
use tolk_analysis::UseFlags;

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
        let Some(file_id) = snapshot.find_document_file(document) else {
            return Vec::new();
        };
        let offset = document
            .text_index()
            .position_to_offset(document.text(), position);
        let Some(resolved) = snapshot.resolved_at(file_id, offset) else {
            return Vec::new();
        };

        let mut highlights = snapshot
            .reference_spans_for_resolved(&resolved, true)
            .into_iter()
            .filter(|(reference_file_id, _)| *reference_file_id == file_id)
            .map(|(_, span)| {
                let kind = if snapshot
                    .use_facts
                    .get(&file_id)
                    .and_then(|facts| facts.per_usage.get(&span))
                    .is_some_and(|flags| flags.contains(UseFlags::WRITE))
                {
                    DocumentHighlightKind::Write
                } else {
                    DocumentHighlightKind::Read
                };
                DocumentHighlight::new(range_for_span(document.text(), span), kind)
            })
            .collect::<Vec<_>>();
        highlights.sort_by_key(|highlight| highlight.range.start);
        highlights
    }
}
