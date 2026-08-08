use super::TolkCompletionProviderContext;
use super::support::{ProviderGroup, add_snippet, provider_group};
use crate::completion::{CompletionCategory, CompletionCollector, CompletionProvider};

/// Completes expression-oriented snippets such as a `match` expression skeleton.
///
/// The provider is restricted to expression contexts so statement snippets are
/// not offered where an expression is required.
pub(crate) struct ExpressionSnippetCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for ExpressionSnippetCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        provider_group(context.syntax) == ProviderGroup::General && context.syntax.expression()
    }

    fn collect(
        &self,
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) -> Option<()> {
        add_snippet(
            context.syntax,
            collector,
            "match",
            "match (${1:condition}) {\n\t$0\n}",
            CompletionCategory::Snippet,
        );
        Some(())
    }
}
