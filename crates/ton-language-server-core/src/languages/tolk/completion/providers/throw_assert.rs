use super::TolkCompletionProviderContext;
use super::support::{ProviderGroup, provider_group};
use crate::completion::{
    CompletionCategory, CompletionCollector, CompletionProvider, CompletionRank,
};
use crate::{CompletionItem, CompletionItemKind};

/// Completes `throw` and `assert` statement snippets in statement contexts.
///
/// Both snippets expose editable exception values or conditions and leave the
/// cursor after the generated statement.
pub(crate) struct ThrowAssertCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for ThrowAssertCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        provider_group(context.syntax) == ProviderGroup::General && context.syntax.is_statement()
    }

    fn collect(
        &self,
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) -> Option<()> {
        for (label, detail, snippet) in [
            ("throw", " EXIT_CODE", "throw ${1:5};$0"),
            (
                "assert",
                " (cond) throw EXIT_CODE",
                "assert (${1:cond}) throw ${2:5};$0",
            ),
        ] {
            collector.add(
                CompletionItem::new(label, CompletionItemKind::Keyword)
                    .with_label_detail(detail)
                    .with_snippet_replacement(context.syntax.replacement_range, snippet),
                CompletionRank::new(CompletionCategory::Keyword)
                    .with_prefix(&context.syntax.prefix, label),
            );
        }
        Some(())
    }
}
