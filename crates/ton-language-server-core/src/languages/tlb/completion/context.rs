use crate::completion::{CompletionCategory, CompletionRank, identifier_prefix};
use crate::{DocumentSnapshot, Position, Range};
use tree_sitter::{Node, Point};

pub(super) const DUMMY_IDENTIFIER: &str = "DummyIdentifier";

pub(super) struct TlbCompletionContext {
    source_file: tlb_syntax::SourceFile,
    cursor_point: Point,
    pub(super) is_type: bool,
    pub(super) prefix: String,
    pub(super) replacement_range: Range,
}

impl TlbCompletionContext {
    pub(super) fn new(
        document: &DocumentSnapshot,
        position: Position,
    ) -> anyhow::Result<Option<Self>> {
        let (prefix, replacement_range) = identifier_prefix(document, position);
        let offset = document
            .text_index()
            .position_to_offset(document.text(), position)
            .min(document.text().len());
        let (left, right) = document.text().split_at(offset);
        let source_file = tlb_syntax::parse(&format!("{left}{DUMMY_IDENTIFIER}{right}"))?;
        let cursor_point = document
            .text_index()
            .position_to_point(document.text(), position);
        let Some(node) = source_file
            .tree
            .root_node()
            .descendant_for_point_range(cursor_point, cursor_point)
        else {
            return Ok(None);
        };
        if !matches!(node.kind(), "identifier" | "type_identifier") {
            return Ok(None);
        }

        Ok(Some(Self {
            is_type: node.kind() == "type_identifier",
            source_file,
            cursor_point,
            prefix: prefix.to_owned(),
            replacement_range,
        }))
    }

    pub(super) fn source(&self) -> &str {
        self.source_file.source.as_ref()
    }

    pub(super) const fn source_file(&self) -> &tlb_syntax::SourceFile {
        &self.source_file
    }

    pub(super) fn cursor_node(&self) -> Option<Node<'_>> {
        self.source_file
            .tree
            .root_node()
            .descendant_for_point_range(self.cursor_point, self.cursor_point)
    }

    pub(super) fn rank_for(&self, category: CompletionCategory, label: &str) -> CompletionRank {
        CompletionRank::new(category).with_prefix(&self.prefix, label)
    }
}
