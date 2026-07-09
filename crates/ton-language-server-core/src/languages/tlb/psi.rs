use super::index::TlbSymbol;
use super::{TlbParsedDocument, reference};
use crate::{DocumentSnapshot, Position, Range};
use tree_sitter::Node;

pub(super) struct TlbPsiFile<'a> {
    document: &'a DocumentSnapshot,
    parsed: &'a TlbParsedDocument,
}

impl<'a> TlbPsiFile<'a> {
    pub(super) const fn new(document: &'a DocumentSnapshot, parsed: &'a TlbParsedDocument) -> Self {
        Self { document, parsed }
    }

    pub(super) fn definition_ranges_at(&self, position: Position) -> Vec<Range> {
        reference::definition_ranges_at(self, position)
    }

    pub(super) const fn document(&self) -> &'a DocumentSnapshot {
        self.document
    }

    pub(super) fn source(&self) -> &str {
        self.document.text()
    }

    pub(super) fn range_of(&self, node: Node<'_>) -> Range {
        self.document
            .text_index()
            .range_of_node(self.document.text(), node)
    }

    pub(super) fn range_of_symbol(&self, symbol: &TlbSymbol) -> Range {
        self.document.text_index().range_for_offsets(
            self.document.text(),
            symbol.start_byte,
            symbol.end_byte,
        )
    }

    pub(super) fn declarations_named<'b>(
        &'b self,
        name: &'b str,
    ) -> impl Iterator<Item = &'b TlbSymbol> {
        self.parsed.index.declarations_named(name)
    }

    pub(super) fn node_at(&self, position: Position) -> Option<Node<'_>> {
        let point = self
            .document
            .text_index()
            .position_to_point(self.document.text(), position);
        self.parsed
            .source_file
            .tree
            .root_node()
            .descendant_for_point_range(point, point)
    }
}
