use super::{TolkResolveSnapshot, fallback_uri_for_path, range_for_span};
use crate::Location;
use tolk_resolver::{FileId, Resolved, Span, SymbolId};
use tolk_ty::InferenceResult;

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

    pub(super) fn location_for_span(&self, file_id: FileId, span: Span) -> Vec<Location> {
        let Some(file) = self.file_db.get_by_id(file_id) else {
            return Vec::new();
        };
        let uri = self
            .path_to_uri
            .get(file.path())
            .cloned()
            .unwrap_or_else(|| fallback_uri_for_path(file.path()));
        let source = file.source().source.as_ref();
        let range = range_for_span(source, span);

        vec![Location::new(uri, range)]
    }

    pub(super) fn inferred_resolved_at(
        &self,
        symbol_id: SymbolId,
        offset: usize,
    ) -> Option<Resolved> {
        let inference = self
            .all_body_types
            .get(&symbol_id.file_id)?
            .get(&symbol_id)?;

        resolved_from_inference(inference, offset)
    }
}

fn resolved_from_inference(inference: &InferenceResult, offset: usize) -> Option<Resolved> {
    if let Some(resolved) = inference.resolve(Span::from_offset(offset)) {
        return Some(resolved.resolved.clone());
    }

    inference
        .resolved_refs
        .iter()
        .find(|name_use| name_use.span.contains(offset))
        .map(|resolved| resolved.resolved.clone())
}
