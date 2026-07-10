use super::{TolkWorkspaceEngine, logical_path_for_uri};
use crate::{DocumentSnapshot, Location};
use tolk_resolver::Resolved;

impl TolkWorkspaceEngine {
    pub(super) fn definition(
        &self,
        document: &DocumentSnapshot,
        position: crate::Position,
    ) -> Vec<Location> {
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

        if let Some(name_use) = snapshot.project_index.find_use(file_id, offset)
            && !matches!(name_use.resolved, Resolved::Unresolved)
        {
            return snapshot.location_for_resolved(&name_use.resolved);
        }

        if let Some(symbol) = snapshot.project_index.find_symbol_at(file_id, offset) {
            return snapshot.location_for_span(symbol.id.file_id, symbol.name_span);
        }

        if let Some(resolve_index) = snapshot.project_index.get_resolved_uses(file_id)
            && let Some(local) = resolve_index.find_local_at(offset)
        {
            return snapshot.location_for_span(local.id.file_id, local.def_span);
        }

        let Some(file_info) = snapshot.file_db.get_by_id(file_id) else {
            return Vec::new();
        };
        let Some(symbol) = file_info.find_symbol_at(offset) else {
            return Vec::new();
        };
        snapshot
            .inferred_resolved_at(symbol.id, offset)
            .map_or_else(Vec::new, |resolved| {
                snapshot.location_for_resolved(&resolved)
            })
    }
}
