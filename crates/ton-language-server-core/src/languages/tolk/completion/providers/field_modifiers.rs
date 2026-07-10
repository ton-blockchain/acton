use super::TolkCompletionProviderContext;
use super::support::{ProviderGroup, provider_group};
use crate::completion::{
    CompletionCategory, CompletionCollector, CompletionProvider, CompletionRank,
};
use crate::{CompletionItem, CompletionItemKind, Range};

pub(crate) struct FieldModifierCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for FieldModifierCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        provider_group(context.syntax) == ProviderGroup::FieldModifier
    }

    fn collect(
        &self,
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) {
        let modifiers = context
            .syntax
            .ancestor("struct_field_declaration")
            .and_then(|field| field.utf8_text(context.syntax.source().as_bytes()).ok())
            .unwrap_or_default();
        let cursor = context
            .document
            .text_index()
            .offset_to_position(context.document.text(), context.syntax.offset);
        let replacement_range = Range::new(context.syntax.replacement_range.start, cursor);
        for modifier in ["private", "readonly"] {
            if modifiers
                .split_whitespace()
                .any(|existing| existing == modifier)
            {
                continue;
            }
            collector.add(
                CompletionItem::new(modifier, CompletionItemKind::Keyword)
                    .with_replacement(replacement_range, format!("{modifier} ")),
                CompletionRank::new(CompletionCategory::Keyword)
                    .with_prefix(&context.syntax.prefix, modifier),
            );
        }
    }
}
