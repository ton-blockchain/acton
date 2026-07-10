use super::{TolkResolveSnapshot, TolkWorkspaceEngine, logical_path_for_uri};
use crate::{DocumentSnapshot, Location, Position};
use tolk_resolver::{FileId, Resolved, Span, SymbolId};
use tolk_ty::{TyData, TyId};

impl TolkWorkspaceEngine {
    pub(super) fn type_definition(
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
        let path = logical_path_for_uri(document.uri());
        let Some(file_id) = snapshot.project_index.get_file_by_path(&path) else {
            return Vec::new();
        };
        let offset = document
            .text_index()
            .position_to_offset(document.text(), position);

        snapshot.type_definition(file_id, offset)
    }
}

impl TolkResolveSnapshot {
    fn type_definition(&self, file_id: FileId, offset: usize) -> Vec<Location> {
        if let Some(Resolved::Global(symbol_id)) = self.resolved_at(file_id, offset)
            && self
                .project_index
                .resolve_symbol(symbol_id)
                .is_some_and(|symbol| symbol.is_type())
        {
            return self.location_for_resolved(&Resolved::Global(symbol_id));
        }

        self.type_at(file_id, offset)
            .and_then(|ty| self.type_symbol(ty))
            .map_or_else(Vec::new, |symbol_id| {
                self.location_for_resolved(&Resolved::Global(symbol_id))
            })
    }

    fn type_at(&self, file_id: FileId, offset: usize) -> Option<TyId> {
        if let Some(Resolved::Local(local_id)) = self.resolved_at(file_id, offset) {
            let local = self
                .project_index
                .get_resolved_uses(local_id.file_id)?
                .find_local(local_id)?;
            return self.local_type(local);
        }

        let file = self.file_db.get_by_id(file_id)?;
        let symbol = file.find_symbol_at(offset)?;
        let inference = self.all_body_types.get(&file_id)?.get(&symbol.id)?;
        let span = identifier_span_at(file.source(), offset)?;
        inference.type_of(span)
    }

    fn type_symbol(&self, ty: TyId) -> Option<SymbolId> {
        match self.type_interner.data(ty) {
            TyData::Struct { def, .. }
            | TyData::Enum { def, .. }
            | TyData::TypeAlias { def, .. } => Some(*def),
            TyData::GenericTypeWithTs { inner_ty, .. } => self.type_symbol(*inner_ty),
            _ => None,
        }
    }
}

fn identifier_span_at(source_file: &tolk_syntax::SourceFile, offset: usize) -> Option<Span> {
    [offset, offset.saturating_sub(1)]
        .into_iter()
        .find_map(|offset| {
            let mut node = source_file
                .tree
                .root_node()
                .descendant_for_byte_range(offset, offset)?;

            loop {
                if matches!(node.kind(), "identifier" | "type_identifier") {
                    return Some(Span::from_syntax(&node));
                }
                node = node.parent()?;
            }
        })
}
