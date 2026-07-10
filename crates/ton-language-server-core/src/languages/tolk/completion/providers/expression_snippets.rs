use super::TolkCompletionProviderContext;
use super::support::{ProviderGroup, add_snippet, provider_group};
use crate::completion::{CompletionCategory, CompletionCollector, CompletionProvider};

pub(crate) struct ExpressionSnippetCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for ExpressionSnippetCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        provider_group(context.syntax) == ProviderGroup::General && context.syntax.expression()
    }

    fn collect(
        &self,
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) {
        add_snippet(
            context.syntax,
            collector,
            "match",
            "match (${1:condition}) {\n\t$0\n}",
            CompletionCategory::Snippet,
        );
    }
}
