use super::TolkCompletionProviderContext;
use super::support::{ProviderGroup, add_snippet, provider_group};
use crate::completion::{CompletionCategory, CompletionCollector, CompletionProvider};

pub(crate) struct StorageCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for StorageCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        provider_group(context.syntax) == ProviderGroup::TopLevel
    }

    fn collect(
        &self,
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) {
        add_snippet(
            context.syntax,
            collector,
            "storage",
            "struct ${1:Storage} {\n    $0\n}\n\nfun ${1:Storage}.load() {\n    return ${1:Storage}.fromCell(contract.getData());\n}\n\nfun ${1:Storage}.save(self) {\n    contract.setData(self.toCell());\n}",
            CompletionCategory::Snippet,
        );
    }
}
