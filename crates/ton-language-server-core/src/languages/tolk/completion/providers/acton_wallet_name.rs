use super::TolkCompletionProviderContext;
use crate::completion::{
    CompletionCategory, CompletionCollector, CompletionProvider, CompletionRank,
};
use crate::{CompletionItem, CompletionItemKind};

/// Completes configured wallet names in the first argument of `scripts.wallet`.
///
/// Values are taken from the current workspace configuration and are inserted as
/// string contents without replacing the surrounding quotes.
pub(crate) struct ActonWalletNameCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for ActonWalletNameCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        super::matches_call(context.syntax, "wallet", Some("scripts"), 0)
    }

    fn collect(
        &self,
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) -> Option<()> {
        let (prefix, range) = super::string_prefix_and_range(context.syntax, context.document)?;
        for wallet in context.workspace.wallet_names {
            collector.add(
                CompletionItem::new(wallet, CompletionItemKind::Value)
                    .with_label_detail(" (local)")
                    .with_replacement(range, wallet),
                CompletionRank::new(CompletionCategory::Variable).with_prefix(&prefix, wallet),
            );
        }
        Some(())
    }
}
