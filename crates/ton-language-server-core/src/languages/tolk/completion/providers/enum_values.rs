use super::TolkCompletionProviderContext;
use crate::completion::{CompletionCollector, CompletionProvider};
use crate::languages::tolk::completion::items;
use tolk_resolver::SymbolKind;

/// Completes qualified enum members in expression contexts.
///
/// Candidates are restricted to the selected enum, and insertion preserves the
/// source spelling required for backticked member names.
pub(crate) struct EnumCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for EnumCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        context.syntax.expression()
    }

    fn collect(
        &self,
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) -> Option<()> {
        for symbol in context.visible_globals() {
            let SymbolKind::Enum { members } = &symbol.kind else {
                continue;
            };

            for member in members.iter() {
                let candidate = items::prefixed_enum_member(context, symbol, member);
                collector.add(candidate.item, candidate.rank);
            }
        }
        Some(())
    }
}
