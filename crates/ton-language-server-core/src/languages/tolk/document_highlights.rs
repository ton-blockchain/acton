use super::{TolkResolveSnapshot, TolkWorkspaceEngine, range_for_span};
use crate::{DocumentHighlight, DocumentHighlightKind, DocumentSnapshot, Position};
use std::sync::Arc;
use tolk_analysis::{AnalysisDb, FileUseFacts, UseFlags};
use tolk_resolver::FileId;

impl TolkResolveSnapshot {
    fn file_use_facts(&self, file_id: FileId) -> Option<Arc<FileUseFacts>> {
        if let Some(facts) = self
            .use_facts
            .read()
            .expect("Tolk use-facts lock poisoned")
            .get(&file_id)
            .cloned()
        {
            return Some(facts);
        }

        let mut analysis_db = AnalysisDb::new();
        let facts = analysis_db.use_facts(
            &self.file_db,
            &self.project_index,
            &self.all_body_types,
            file_id,
        )?;
        let mut cache = self
            .use_facts
            .write()
            .expect("Tolk use-facts lock poisoned");

        Some(cache.entry(file_id).or_insert(facts).clone())
    }
}

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
        let use_facts = snapshot.file_use_facts(file_id);

        let mut highlights = snapshot
            .reference_spans_for_resolved(&resolved, true)
            .into_iter()
            .filter(|(reference_file_id, _)| *reference_file_id == file_id)
            .map(|(_, span)| {
                let kind = if use_facts
                    .as_ref()
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
