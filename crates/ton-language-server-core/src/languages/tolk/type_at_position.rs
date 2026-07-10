use super::{TolkResolveSnapshot, TolkWorkspaceEngine, range_for_span};
use crate::{DocumentSnapshot, Position, TypeAtPosition};
use tolk_resolver::{FileId, Span};
use tolk_syntax::{Stmt, TryFromNode};
use tolk_ty::TyId;
use tree_sitter::Node;

const UNKNOWN_TYPE: &str = "void or unknown";

impl TolkWorkspaceEngine {
    pub(super) fn type_at_position(
        &self,
        document: &DocumentSnapshot,
        position: Position,
    ) -> Option<TypeAtPosition> {
        let snapshot = {
            let state = self.state.read().expect("Tolk workspace lock poisoned");
            state.latest_snapshot.clone()
        }?;
        let file_id = snapshot.find_document_file(document)?;
        let file = snapshot.file_db.get_by_id(file_id)?;
        let offset = document
            .text_index()
            .position_to_offset(document.text(), position);
        let node = node_at_offset(file.source().tree.root_node(), offset)?;

        Some(snapshot.type_at_node(file_id, file.source().source.as_ref(), node))
    }
}

impl TolkResolveSnapshot {
    fn type_at_node(&self, file_id: FileId, source: &str, mut node: Node<'_>) -> TypeAtPosition {
        let original_node = node;

        loop {
            if let Some(ty) = self.type_of_node(file_id, node) {
                return self.type_result(source, node, ty);
            }

            let Some(parent) = node.parent() else {
                return unknown_type_result(source, original_node);
            };
            if Stmt::try_from_node(parent).is_ok() {
                return unknown_type_result(source, original_node);
            }

            node = parent;
        }
    }

    fn type_of_node(&self, file_id: FileId, node: Node<'_>) -> Option<TyId> {
        self.inferred_type_of_node(file_id, node)
            .or_else(|| {
                self.resolved_target_at(file_id, node.start_byte())
                    .and_then(|target| self.type_of_resolved(&target.resolved))
            })
            .filter(|ty| *ty != self.type_interner.ty_undefined)
    }

    fn type_result(&self, source: &str, node: Node<'_>, ty: TyId) -> TypeAtPosition {
        let range = range_for_span(source, Span::from_syntax(&node));
        TypeAtPosition::new(self.type_interner.format(ty), range)
    }
}

fn unknown_type_result(source: &str, node: Node<'_>) -> TypeAtPosition {
    let range = range_for_span(source, Span::from_syntax(&node));
    TypeAtPosition::new(UNKNOWN_TYPE, range)
}

fn node_at_offset(root: Node<'_>, offset: usize) -> Option<Node<'_>> {
    [offset, offset.saturating_sub(1)]
        .into_iter()
        .find_map(|offset| root.descendant_for_byte_range(offset, offset))
}
