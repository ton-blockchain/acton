use super::TolkCompletionProviderContext;
use super::support::{ProviderGroup, provider_group};
use crate::completion::{
    CompletionCategory, CompletionCollector, CompletionProvider, CompletionRank,
};
use crate::{CompletionItem, CompletionItemKind, Range};
use tolk_syntax::StructField;

/// Completes missing `private` and `readonly` modifiers on struct fields.
///
/// Existing modifiers are inspected from the typed field declaration and are not
/// suggested a second time.
pub(crate) struct FieldModifierCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for FieldModifierCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        provider_group(context.syntax) == ProviderGroup::FieldModifier
    }

    fn collect(
        &self,
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) -> Option<()> {
        let field = context.syntax.ancestor_as::<StructField>();
        let cursor = context
            .document
            .text_index()
            .offset_to_position(context.document.text(), context.syntax.offset);
        let replacement_range = Range::new(context.syntax.replacement_range.start, cursor);
        for (modifier, already_present) in [
            ("private", field.is_some_and(|field| field.has_private())),
            ("readonly", field.is_some_and(|field| field.has_readonly())),
        ] {
            if already_present {
                continue;
            }
            collector.add(
                CompletionItem::new(modifier, CompletionItemKind::Keyword)
                    .with_replacement(replacement_range, format!("{modifier} ")),
                CompletionRank::new(CompletionCategory::Keyword)
                    .with_prefix(&context.syntax.prefix, modifier),
            );
        }
        Some(())
    }
}
