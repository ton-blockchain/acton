use lsp_types::{CompletionParams, CompletionTriggerKind, Position, Url};

#[derive(Debug, Clone)]
pub(crate) struct CompletionRequestContext<'a> {
    pub(crate) uri: &'a Url,
    pub(crate) position: Position,
    pub(crate) trigger_kind: CompletionTriggerKind,
    pub(crate) trigger_character: Option<&'a str>,
}

impl<'a> CompletionRequestContext<'a> {
    #[must_use]
    pub(crate) fn from_params(params: &'a CompletionParams) -> Self {
        let completion_context = params.context.as_ref();
        Self {
            uri: &params.text_document_position.text_document.uri,
            position: params.text_document_position.position,
            trigger_kind: completion_context
                .map(|ctx| ctx.trigger_kind)
                .unwrap_or(CompletionTriggerKind::INVOKED),
            trigger_character: completion_context.and_then(|ctx| ctx.trigger_character.as_deref()),
        }
    }
}
