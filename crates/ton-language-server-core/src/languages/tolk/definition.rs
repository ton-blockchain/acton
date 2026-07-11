use super::TolkWorkspaceEngine;
use crate::{DocumentSnapshot, Location, Position};
use tolk_resolver::Span;

impl TolkWorkspaceEngine {
    pub(super) fn definition(
        &self,
        document: &DocumentSnapshot,
        position: Position,
    ) -> Vec<Location> {
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

        if let Some(target) = snapshot.import_target_at(file_id, offset) {
            return snapshot.location_for_span(target, Span::file_start());
        }

        snapshot
            .resolved_targets_at(file_id, offset)
            .into_iter()
            .flat_map(|target| snapshot.location_for_resolved(&target.resolved))
            .collect()
    }
}
