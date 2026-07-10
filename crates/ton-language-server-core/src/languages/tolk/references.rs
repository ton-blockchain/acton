use super::{TolkResolveSnapshot, TolkWorkspaceEngine, logical_path_for_uri};
use crate::{DocumentSnapshot, Location};
use tolk_resolver::{Resolved, SymbolId, resolve_index::LocalDefId};
use tolk_ty::GlobalUsages;

impl TolkWorkspaceEngine {
    pub(super) fn references(
        &self,
        document: &DocumentSnapshot,
        position: crate::Position,
        include_declaration: bool,
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

        snapshot
            .resolved_at(file_id, offset)
            .map_or_else(Vec::new, |resolved| {
                snapshot.references_for_resolved(&resolved, include_declaration)
            })
    }
}

impl TolkResolveSnapshot {
    pub(super) fn references_for_resolved(
        &self,
        resolved: &Resolved,
        include_declaration: bool,
    ) -> Vec<Location> {
        let mut locations = match resolved {
            Resolved::Global(symbol_id) => self.global_references(*symbol_id),
            Resolved::Local(local_id) => self.local_references(*local_id),
            Resolved::Unresolved => Vec::new(),
        };
        if include_declaration {
            locations.extend(self.location_for_resolved(resolved));
        }
        locations
    }

    fn global_references(&self, symbol_id: SymbolId) -> Vec<Location> {
        // Tolk projects can contain dozens or hundreds of files: the Tolk
        // standard library, Acton libraries, tests, scripts, and contracts.
        // A global symbol is usually used in only a few of them, so references
        // should stay backed by resolver/type-inference indexes instead of
        // rescanning source text.
        //
        // If this becomes hot for large workspaces, use the import graph to
        // restrict candidate files to the definition file and files that can
        // reach it, as the old ton-ls implementation intended.
        let usages = GlobalUsages::new(self.project_index.as_ref(), &self.all_body_types);
        usages
            .for_symbol(symbol_id)
            .flat_map(|reference| self.location_for_span(reference.file_id, reference.usage.span))
            .collect()
    }

    fn local_references(&self, local_id: LocalDefId) -> Vec<Location> {
        self.project_index
            .get_resolved_uses(local_id.file_id)
            .map_or_else(Vec::new, |resolve_index| {
                resolve_index
                    .local_usages_of(local_id)
                    .flat_map(|usage| self.location_for_span(local_id.file_id, usage.span))
                    .collect()
            })
    }
}
