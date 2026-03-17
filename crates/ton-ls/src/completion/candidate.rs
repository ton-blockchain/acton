use crate::completion::data::CompletionItemDataEnvelope;
use crate::completion::ranking::{CompletionRank, encode_sort_text};
use lsp_types::{CompletionItem, CompletionItemKind, Documentation, InsertTextFormat};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompletionResolvePayloadRef {
    pub(crate) language: String,
    pub(crate) provider: String,
    pub(crate) resolve_id: String,
}

#[derive(Debug, Clone)]
pub(crate) struct CompletionCandidate {
    pub(crate) label: String,
    pub(crate) kind: Option<CompletionItemKind>,
    pub(crate) insert_text: Option<String>,
    pub(crate) insert_text_format: Option<InsertTextFormat>,
    pub(crate) detail: Option<String>,
    pub(crate) documentation: Option<Documentation>,
    pub(crate) filter_text: Option<String>,
    pub(crate) commit_characters: Option<Vec<String>>,
    pub(crate) deprecated: bool,
    pub(crate) preselect: bool,
    pub(crate) rank: CompletionRank,
    pub(crate) resolve: Option<CompletionResolvePayloadRef>,
}

impl CompletionCandidate {
    #[must_use]
    pub(crate) fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            kind: None,
            insert_text: None,
            insert_text_format: None,
            detail: None,
            documentation: None,
            filter_text: None,
            commit_characters: None,
            deprecated: false,
            preselect: false,
            rank: CompletionRank::default(),
            resolve: None,
        }
    }

    #[must_use]
    pub(crate) fn convert_to_completion_item(self) -> CompletionItem {
        let sort_text = encode_sort_text(self.rank, &self.label);
        let data = self.resolve.map(|payload_ref| {
            CompletionItemDataEnvelope::new(
                payload_ref.language,
                payload_ref.provider,
                payload_ref.resolve_id,
            )
            .to_json_value()
        });

        CompletionItem {
            label: self.label,
            kind: self.kind,
            detail: self.detail,
            documentation: self.documentation,
            filter_text: self.filter_text,
            sort_text: Some(sort_text),
            preselect: self.preselect.then_some(true),
            insert_text: self.insert_text,
            insert_text_format: self.insert_text_format,
            commit_characters: self.commit_characters,
            deprecated: self.deprecated.then_some(true),
            data,
            ..Default::default()
        }
    }
}
