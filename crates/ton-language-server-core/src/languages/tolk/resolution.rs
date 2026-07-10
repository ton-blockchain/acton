use super::{TolkResolveSnapshot, fallback_uri_for_path, range_for_span};
use crate::Location;
use tolk_resolver::{FileId, NameUse, Resolved, Span, SymbolId};
use tolk_ty::InferenceResult;

pub(super) struct ResolvedTarget {
    pub(super) resolved: Resolved,
    pub(super) span: Span,
}

impl TolkResolveSnapshot {
    pub(super) fn resolved_at(&self, file_id: FileId, offset: usize) -> Option<Resolved> {
        self.resolved_target_at(file_id, offset)
            .map(|target| target.resolved)
    }

    pub(super) fn resolved_target_at(
        &self,
        file_id: FileId,
        offset: usize,
    ) -> Option<ResolvedTarget> {
        if let Some(name_use) = self.project_index.find_use(file_id, offset)
            && !matches!(name_use.resolved, Resolved::Unresolved)
        {
            return Some(ResolvedTarget {
                resolved: name_use.resolved.clone(),
                span: name_use.span,
            });
        }

        if let Some(symbol) = self.project_index.find_symbol_at(file_id, offset) {
            return Some(ResolvedTarget {
                resolved: Resolved::Global(symbol.id),
                span: symbol.name_span,
            });
        }

        if let Some(resolve_index) = self.project_index.get_resolved_uses(file_id)
            && let Some(local) = resolve_index.find_local_at(offset)
        {
            return Some(ResolvedTarget {
                resolved: Resolved::Local(local.id),
                span: local.def_span,
            });
        }

        let file_info = self.file_db.get_by_id(file_id)?;
        let symbol = file_info.find_symbol_at(offset)?;
        let name_use = self.inferred_name_use_at(symbol.id, offset)?;

        Some(ResolvedTarget {
            resolved: name_use.resolved.clone(),
            span: name_use.span,
        })
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
        self.inferred_name_use_at(symbol_id, offset)
            .map(|name_use| name_use.resolved.clone())
    }

    fn inferred_name_use_at(&self, symbol_id: SymbolId, offset: usize) -> Option<&NameUse> {
        let inference = self
            .all_body_types
            .get(&symbol_id.file_id)?
            .get(&symbol_id)?;

        name_use_from_inference(inference, offset)
    }
}

fn name_use_from_inference(inference: &InferenceResult, offset: usize) -> Option<&NameUse> {
    if let Some(name_use) = inference.resolve(Span::from_offset(offset)) {
        return Some(name_use);
    }

    inference
        .resolved_refs
        .iter()
        .find(|name_use| name_use.span.contains(offset))
}
