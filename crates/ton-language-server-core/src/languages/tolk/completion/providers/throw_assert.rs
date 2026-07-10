use super::TolkCompletionProviderContext;
use super::support::{ProviderGroup, add_snippet, provider_group};
use crate::completion::{CompletionCategory, CompletionCollector, CompletionProvider};

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
        for (label, snippet) in [
            ("throw", "throw ${1:5};$0"),
            ("assert", "assert (${1:cond}) throw ${2:5};$0"),
        ] {
            add_snippet(
                context.syntax,
                collector,
                label,
                snippet,
                CompletionCategory::Keyword,
            );
        }
        Some(())
    }
}
