use super::{
    TolkResolveSnapshot, TolkWorkspaceEngine, logical_path_for_uri, resolved_from_inference,
};
use crate::{DocumentSnapshot, Location};
use tolk_resolver::{FileId, Resolved, SymbolId};

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

impl TolkResolveSnapshot {
    pub(super) fn resolved_at(&self, file_id: FileId, offset: usize) -> Option<Resolved> {
        if let Some(name_use) = self.project_index.find_use(file_id, offset)
            && !matches!(name_use.resolved, Resolved::Unresolved)
        {
            return Some(name_use.resolved.clone());
        }

        if let Some(symbol) = self.project_index.find_symbol_at(file_id, offset) {
            return Some(Resolved::Global(symbol.id));
        }

        if let Some(resolve_index) = self.project_index.get_resolved_uses(file_id)
            && let Some(local) = resolve_index.find_local_at(offset)
        {
            return Some(Resolved::Local(local.id));
        }

        let file_info = self.file_db.get_by_id(file_id)?;
        let symbol = file_info.find_symbol_at(offset)?;
        self.inferred_resolved_at(symbol.id, offset)
    }

    fn inferred_resolved_at(&self, symbol_id: SymbolId, offset: usize) -> Option<Resolved> {
        let inference = self
            .all_body_types
            .get(&symbol_id.file_id)?
            .get(&symbol_id)?;
        resolved_from_inference(inference, offset)
    }

    pub(super) fn location_for_resolved(&self, resolved: &Resolved) -> Vec<Location> {
        match resolved {
            Resolved::Global(symbol_id) => self
                .project_index
                .resolve_symbol(*symbol_id)
                .map_or_else(Vec::new, |symbol| {
                    self.location_for_span(symbol.id.file_id, symbol.name_span)
                }),
            Resolved::Local(local_id) => self
                .project_index
                .get_resolved_uses(local_id.file_id)
                .and_then(|resolve_index| resolve_index.find_local(*local_id))
                .map_or_else(Vec::new, |local| {
                    self.location_for_span(local.id.file_id, local.def_span)
                }),
            Resolved::Unresolved => Vec::new(),
        }
    }
}
