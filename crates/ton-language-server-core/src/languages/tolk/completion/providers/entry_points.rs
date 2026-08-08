use super::TolkCompletionProviderContext;
use super::support::{ProviderGroup, add_snippet, provider_group};
use crate::completion::{CompletionCategory, CompletionCollector, CompletionProvider};

/// Completes standard Tolk contract entry-point templates at the top level.
///
/// Templates cover internal, external, bounced, and tick-tock handlers and place
/// the cursor in the generated handler body.
pub(crate) struct EntryPointCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for EntryPointCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        provider_group(context.syntax) == ProviderGroup::TopLevel
    }

    fn collect(
        &self,
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) -> Option<()> {
        for &(label, snippet) in ENTRY_POINTS {
            add_snippet(
                context.syntax,
                collector,
                label,
                snippet,
                CompletionCategory::ContextElement,
            );
        }
        Some(())
    }
}

const ENTRY_POINTS: &[(&str, &str)] = &[
    (
        "onInternalMessage",
        "fun onInternalMessage(in: InMessage) {\n    $0\n}",
    ),
    (
        "onBouncedMessage",
        "fun onBouncedMessage(in: InMessageBounced) {\n    $0\n}",
    ),
    (
        "onExternalMessage",
        "fun onExternalMessage(inMsg: slice) {\n    $0\n}",
    ),
    ("onTickTock", "fun onTickTock(isTock: bool) {\n    $0\n}"),
];
