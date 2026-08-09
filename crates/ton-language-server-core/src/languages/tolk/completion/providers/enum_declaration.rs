use super::TolkCompletionProviderContext;
use super::support::{ProviderGroup, add_snippet, provider_group};
use crate::completion::{CompletionCategory, CompletionCollector, CompletionProvider};

/// Completes an enum member declaration inside an enum body.
///
/// The snippet supplies editable member name and value placeholders while keeping
/// the declaration syntax valid for immediate continued editing.
pub(crate) struct EnumDeclarationCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for EnumDeclarationCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        provider_group(context.syntax) == ProviderGroup::Enum
    }

    fn collect(
        &self,
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) -> Option<()> {
        add_snippet(
            context.syntax,
            collector,
            "member",
            "${1:MEMBER} = ${2:0}$0",
            CompletionCategory::ContextElement,
        );
        Some(())
    }
}
