use crate::InferenceResult;
use std::collections::HashMap;
use tolk_resolver::file_index::{FileId, SymbolId};
use tolk_resolver::project_index::ProjectIndex;
use tolk_resolver::resolve_index::NameUse;

/// A resolved global usage with the file where it appears.
#[derive(Debug, Clone, Copy)]
pub struct GlobalUsage<'a> {
    pub file_id: FileId,
    pub usage: &'a NameUse,
}

/// Unified iterator source for global usages from resolver indexes and body inference.
pub struct GlobalUsages<'a> {
    project_index: &'a ProjectIndex,
    body_types: &'a HashMap<FileId, HashMap<SymbolId, InferenceResult>>,
}

impl<'a> GlobalUsages<'a> {
    #[must_use]
    pub const fn new(
        project_index: &'a ProjectIndex,
        body_types: &'a HashMap<FileId, HashMap<SymbolId, InferenceResult>>,
    ) -> Self {
        Self {
            project_index,
            body_types,
        }
    }

    pub fn for_symbol(&self, symbol_id: SymbolId) -> impl Iterator<Item = GlobalUsage<'_>> + '_ {
        let resolved =
            self.project_index
                .resolved_uses
                .iter()
                .flat_map(move |(&file_id, index)| {
                    index
                        .global_usages_of(symbol_id)
                        .map(move |usage| GlobalUsage { file_id, usage })
                });

        let inferred = self
            .body_types
            .iter()
            .flat_map(move |(&file_id, file_body_types)| {
                file_body_types.values().flat_map(move |inference| {
                    inference
                        .global_usages_of(symbol_id)
                        .map(move |usage| GlobalUsage { file_id, usage })
                })
            });

        resolved.chain(inferred)
    }
}
