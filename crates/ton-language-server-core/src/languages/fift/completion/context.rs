use super::super::FiftParsedDocument;
use crate::completion::{CompletionCategory, CompletionRank, identifier_prefix};
use crate::{DocumentSnapshot, Position, Range};

pub(super) struct FiftCompletionContext<'a> {
    pub(super) document: &'a DocumentSnapshot,
    pub(super) parsed: &'a FiftParsedDocument,
    pub(super) prefix: &'a str,
    pub(super) replacement_range: Range,
}

impl<'a> FiftCompletionContext<'a> {
    pub(super) fn new(
        document: &'a DocumentSnapshot,
        parsed: &'a FiftParsedDocument,
        position: Position,
    ) -> Self {
        let (prefix, replacement_range) = identifier_prefix(document, position);
        Self {
            document,
            parsed,
            prefix,
            replacement_range,
        }
    }

    pub(super) fn rank_for(&self, category: CompletionCategory, label: &str) -> CompletionRank {
        CompletionRank::new(category).with_prefix(self.prefix, label)
    }
}
