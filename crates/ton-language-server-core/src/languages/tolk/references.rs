use super::{TolkResolveSnapshot, TolkWorkspaceEngine};
use crate::{DocumentSnapshot, Location};
use tolk_resolver::{FileId, Resolved, Span};
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
        let Some(file_id) = snapshot.find_document_file(document) else {
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
        self.reference_spans_for_resolved(resolved, include_declaration)
            .into_iter()
            .flat_map(|(file_id, span)| self.location_for_span(file_id, span))
            .collect()
    }

    /// Collects references as source spans without converting them to LSP
    /// locations.
    ///
    /// Each pair identifies the source file and exact referenced-name span.
    /// When requested, the resolved declaration is appended after its usages.
    pub(super) fn reference_spans_for_resolved(
        &self,
        resolved: &Resolved,
        include_declaration: bool,
    ) -> Vec<(FileId, Span)> {
        // Tolk projects can contain dozens or hundreds of files: the Tolk
        // standard library, Acton libraries, tests, scripts, and contracts.
        // A global symbol is usually used in only a few of them, so references
        // should stay backed by resolver/type-inference indexes instead of
        // rescanning source text.
        //
        // If this becomes hot for large workspaces, use the import graph to
        // restrict candidate files to the definition file and files that can
        // reach it, as the old ton-ls implementation intended.
        let mut references = match resolved {
            Resolved::Global(symbol_id) => {
                let usages = GlobalUsages::new(self.project_index.as_ref(), &self.all_body_types);
                usages
                    .for_symbol(*symbol_id)
                    .map(|reference| (reference.file_id, reference.usage.span))
                    .collect()
            }
            Resolved::Local(local_id) => self
                .project_index
                .get_resolved_uses(local_id.file_id)
                .map_or_else(Vec::new, |resolve_index| {
                    resolve_index
                        .local_usages_of(*local_id)
                        .map(|usage| (local_id.file_id, usage.span))
                        .collect()
                }),
            Resolved::Unresolved => Vec::new(),
        };

        if include_declaration {
            let declaration = match resolved {
                Resolved::Global(symbol_id) => self
                    .project_index
                    .resolve_symbol(*symbol_id)
                    .map(|symbol| (symbol.id.file_id, symbol.name_span)),
                Resolved::Local(local_id) => self
                    .project_index
                    .get_resolved_uses(local_id.file_id)
                    .and_then(|resolve_index| resolve_index.find_local(*local_id))
                    .map(|local| (local.id.file_id, local.def_span)),
                Resolved::Unresolved => None,
            };
            references.extend(declaration);
        }

        references
    }
}
