use super::TolkCompletionProviderContext;
use super::support::{ProviderGroup, add_snippet, provider_group};
use crate::completion::{CompletionCategory, CompletionCollector, CompletionProvider};

pub(crate) struct EnumDeclarationCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for EnumDeclarationCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        provider_group(context.syntax) == ProviderGroup::Enum
    }

    fn collect(
        &self,
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) {
        add_snippet(
            context.syntax,
            collector,
            "member",
            "${1:MEMBER} = ${2:0}$0",
            CompletionCategory::ContextElement,
        );
    }
}
