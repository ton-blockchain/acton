use super::TolkCompletionProviderContext;
use super::support::{ProviderGroup, add_snippet, provider_group};
use crate::completion::{CompletionCategory, CompletionCollector, CompletionProvider};

pub(crate) struct EntryPointCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for EntryPointCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        provider_group(context.syntax) == ProviderGroup::TopLevel
    }

    fn collect(
        &self,
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) {
        for &(label, snippet) in ENTRY_POINTS {
            add_snippet(
                context.syntax,
                collector,
                label,
                snippet,
                CompletionCategory::ContextElement,
            );
        }
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
