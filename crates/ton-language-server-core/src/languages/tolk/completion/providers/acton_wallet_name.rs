use super::TolkCompletionProviderContext;
use crate::completion::{
    CompletionCategory, CompletionCollector, CompletionProvider, CompletionRank,
};
use crate::{CompletionItem, CompletionItemKind};

pub(crate) struct ActonWalletNameCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for ActonWalletNameCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        super::matches_call(context.syntax, "wallet", Some("scripts"), 0)
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
        for wallet in context.workspace.wallet_names {
            collector.add(
                CompletionItem::new(wallet, CompletionItemKind::Value)
                    .with_replacement(range, wallet),
                CompletionRank::new(CompletionCategory::Variable).with_prefix(&prefix, wallet),
            );
        }
    }
}
