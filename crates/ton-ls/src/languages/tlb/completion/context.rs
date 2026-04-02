use crate::completion::ranking::{CompletionCategory, CompletionRank};
use crate::languages::engine::cache::ParsedSnapshot;
use lsp_types::Position;
use std::sync::Arc;
use tree_sitter::Node;

const DUMMY_IDENTIFIER: &str = "DummyIdentifier";

#[derive(Clone)]
pub(super) struct TlbCompletionContext {
    pub(super) file: ParsedSnapshot<tlb_syntax::SourceFile>,
    pub(super) position: Position,
    pub(super) is_type: bool,
    typed_prefix: String,
}

impl TlbCompletionContext {
    pub(super) fn new(
        file: ParsedSnapshot<tlb_syntax::SourceFile>,
        position: Position,
    ) -> Option<Self> {
        let source = file.source();
        let offset = file.position_to_offset(position);
        let typed_prefix = identifier_prefix(source, offset);

        let (left, right) = source.split_at(offset.min(source.len()));
        let rewritten_source = format!("{left}{DUMMY_IDENTIFIER}{right}");
        let parsed = tlb_syntax::parse(&rewritten_source).ok()?;

        let rewritten = ParsedSnapshot::new(
            file.uri.clone(),
            file.version,
            rewritten_source,
            Arc::new(parsed),
        );

        let node = rewritten.node_at(position)?;
        if !matches!(node.kind(), "identifier" | "type_identifier") {
            return None;
        }
        let is_type = node.kind() == "type_identifier";

        Some(Self {
            file: rewritten,
            position,
            is_type,
            typed_prefix,
        })
    }

    pub(super) fn cursor_node(&self) -> Option<Node<'_>> {
        self.file.node_at(self.position)
    }

    pub(super) fn rank_for(&self, category: CompletionCategory, label: &str) -> CompletionRank {
        let context_match = self.typed_prefix.is_empty() || label.starts_with(&self.typed_prefix);
        CompletionRank {
            category,
            context_match,
            prefix_score: if context_match { 0 } else { 1000 },
            locality_boost: 0,
        }
    }
}

fn identifier_prefix(source: &str, offset: usize) -> String {
    let bytes = source.as_bytes();
    let mut start = offset.min(bytes.len());
    while start > 0 && is_identifier_char(bytes[start - 1]) {
        start -= 1;
    }

    source[start..offset.min(source.len())].to_string()
}

fn is_identifier_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}
