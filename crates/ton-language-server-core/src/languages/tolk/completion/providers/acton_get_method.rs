use super::TolkCompletionProviderContext;
use crate::completion::{
    CompletionCategory, CompletionCollector, CompletionProvider, CompletionRank,
};
use crate::{CompletionItem, CompletionItemKind};
use tolk_resolver::SymbolKind;

pub(crate) struct ActonGetMethodCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for ActonGetMethodCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        super::matches_call(context.syntax, "runGetMethod", Some("net"), 1)
    }

    fn collect(
        &self,
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) {
        let Some((prefix, range)) =
            super::string_prefix_and_range(context.document, context.syntax.offset)
        else {
            return;
        };
        for symbol in context
            .snapshot
            .project_index
            .files()
            .values()
            .filter(|file| !file.path.to_string_lossy().contains(".acton"))
            .flat_map(|file| &file.decls)
            .filter(|symbol| {
                matches!(symbol.kind, SymbolKind::GetMethod { .. })
                    && !tolk_syntax::is_test_get_method_name(symbol.name.as_ref())
            })
        {
            collector.add(
                CompletionItem::new(symbol.name.as_ref(), CompletionItemKind::Method)
                    .with_replacement(range, symbol.name.as_ref()),
                CompletionRank::new(CompletionCategory::Function)
                    .with_prefix(&prefix, symbol.name.as_ref()),
            );
        }
    }
}
