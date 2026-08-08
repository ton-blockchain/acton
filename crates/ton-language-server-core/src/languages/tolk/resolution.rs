use super::TolkResolveSnapshot;
use super::file_info::FileInfoExt;
use crate::{DocumentSnapshot, DocumentUri, Location};
use tolk_resolver::resolve_index::LocalDef;
use tolk_resolver::{FileId, NameUse, Resolved, Span, SymbolId};
use tolk_ty::{FileBodyTypes, InferenceResult, TyId};
use tree_sitter::Node;

pub(super) struct ResolvedTarget {
    pub(super) resolved: Resolved,
    pub(super) span: Span,
}

impl TolkResolveSnapshot {
    fn body_types(&self, file_id: FileId) -> Option<&FileBodyTypes> {
        if let Some((override_file_id, body_types)) = &self.body_types_override
            && *override_file_id == file_id
        {
            return Some(body_types);
        }

        self.all_body_types.get(&file_id)
    }

    /// Returns the client-visible URI assigned to a file in this snapshot.
    ///
    /// The URI index is materialized together with the project index and is
    /// expected to cover every project `FileId`, including virtual
    /// embedded-stdlib files. Returning `None` lets request handlers omit an
    /// isolated stale result instead of panicking if a snapshot is internally
    /// inconsistent.
    #[must_use]
    pub(super) fn file_uri(&self, file_id: FileId) -> Option<&DocumentUri> {
        self.file_uris.get(&file_id)
    }

    /// Finds the project file that corresponds to an open document.
    ///
    /// The document URI is converted with [`crate::DocumentUri::logical_path`], so
    /// lookup uses the same normalized key as workspace indexing. Returns
    /// `None` when the document is not part of this snapshot.
    pub(super) fn find_document_file(&self, document: &DocumentSnapshot) -> Option<FileId> {
        self.project_index
            .get_file_by_path(&document.uri().logical_path())
    }

    pub(super) fn resolved_at(&self, file_id: FileId, offset: usize) -> Option<Resolved> {
        self.resolved_target_at(file_id, offset)
            .map(|target| target.resolved)
    }

    pub(super) fn resolved_target_at(
        &self,
        file_id: FileId,
        offset: usize,
    ) -> Option<ResolvedTarget> {
        self.indexed_target_at(file_id, offset)
            .or_else(|| self.inferred_target_at(file_id, offset))
    }

    pub(super) fn resolved_targets_at(
        &self,
        file_id: FileId,
        offset: usize,
    ) -> Vec<ResolvedTarget> {
        let indexed = self.indexed_target_at(file_id, offset);
        let inferred = self.inferred_target_at(file_id, offset);
        let mut targets = Vec::with_capacity(2);

        if let Some(target) = indexed {
            targets.push(target);
        }
        if let Some(target) = inferred
            && targets
                .iter()
                .all(|existing| existing.resolved != target.resolved)
        {
            targets.push(target);
        }

        targets
    }

    pub(super) fn import_target_at(&self, file_id: FileId, offset: usize) -> Option<FileId> {
        self.project_index
            .imports()
            .get(&file_id)?
            .iter()
            .find(|resolved| resolved.import().path_span.contains(offset))?
            .target()
    }

    fn indexed_target_at(&self, file_id: FileId, offset: usize) -> Option<ResolvedTarget> {
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

        None
    }

    fn inferred_target_at(&self, file_id: FileId, offset: usize) -> Option<ResolvedTarget> {
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
        let Some(uri) = self.file_uri(file_id).cloned() else {
            return Vec::new();
        };
        let range = file.range_for_span(span);

        vec![Location::new(uri, range)]
    }

    pub(super) fn inferred_type_of_node(&self, file_id: FileId, node: Node<'_>) -> Option<TyId> {
        self.inferred_type_of_span(file_id, Span::from_syntax(&node))
    }

    pub(super) fn inferred_type_of_span(&self, file_id: FileId, span: Span) -> Option<TyId> {
        let file = self.file_db.get_by_id(file_id)?;
        let symbol = file.find_symbol_at(span.start())?;
        let inference = self.body_types(file_id)?.get(&symbol.id)?;

        inference.type_of(span)
    }

    pub(super) fn type_of_resolved(&self, resolved: &Resolved) -> Option<TyId> {
        match resolved {
            Resolved::Global(symbol_id) => self.type_db_cache.top_level_type(*symbol_id),
            Resolved::Local(local_id) => {
                let local = self
                    .project_index
                    .get_resolved_uses(local_id.file_id)?
                    .find_local(*local_id)?;

                self.local_type(local)
            }
            Resolved::Unresolved => None,
        }
    }

    pub(super) fn local_type(&self, local: &LocalDef) -> Option<TyId> {
        let file = self.file_db.get_by_id(local.id.file_id)?;
        let symbol = file.find_symbol_at(local.def_span.start())?;

        self.body_types(local.id.file_id)?
            .get(&symbol.id)?
            .type_of(local.def_span)
    }

    fn inferred_name_use_at(&self, symbol_id: SymbolId, offset: usize) -> Option<&NameUse> {
        let inference = self.body_types(symbol_id.file_id)?.get(&symbol_id)?;

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
