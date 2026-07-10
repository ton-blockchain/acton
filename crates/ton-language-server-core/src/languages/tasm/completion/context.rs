use super::super::TasmSpec;
use crate::completion::{CompletionCategory, CompletionRank, identifier_prefix};
use crate::{DocumentSnapshot, Position, Range};

pub(super) struct TasmCompletionContext<'a> {
    pub(super) spec: &'a TasmSpec,
    pub(super) prefix: &'a str,
    pub(super) replacement_range: Range,
}

impl<'a> TasmCompletionContext<'a> {
    pub(super) fn new(
        spec: &'a TasmSpec,
        document: &'a DocumentSnapshot,
        position: Position,
    ) -> Self {
        let (prefix, replacement_range) = identifier_prefix(document, position);
        Self {
            spec,
            prefix,
            replacement_range,
        }
    }

    pub(super) fn rank_for(&self, category: CompletionCategory, label: &str) -> CompletionRank {
        CompletionRank::new(category).with_prefix(self.prefix, label)
    }
}
