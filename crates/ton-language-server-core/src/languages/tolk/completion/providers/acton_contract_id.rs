use super::TolkCompletionProviderContext;
use crate::completion::{
    CompletionCategory, CompletionCollector, CompletionProvider, CompletionRank,
};
use crate::{CompletionItem, CompletionItemKind};

/// Completes contract identifiers in the first argument of an Acton `build` call.
///
/// Candidates come from the workspace contract catalog and are offered only when
/// the call and argument position match the Acton API.
pub(crate) struct ActonContractIdCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for ActonContractIdCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        super::matches_call(context.syntax, "build", None, 0)
    }

    fn collect(
        &self,
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) -> Option<()> {
        let (prefix, range) = super::string_prefix_and_range(context.syntax, context.document)?;
        for id in context.workspace.contract_ids {
            collector.add(
                CompletionItem::new(id, CompletionItemKind::Class).with_replacement(range, id),
                CompletionRank::new(CompletionCategory::Struct).with_prefix(&prefix, id),
            );
        }
        Some(())
    }
}
