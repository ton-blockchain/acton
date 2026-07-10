use super::TolkCompletionProviderContext;
use crate::completion::{CompletionCollector, CompletionProvider};
use crate::languages::tolk::completion::{items, semantics};
use tolk_resolver::SymbolKind;

pub(crate) struct EnumCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for EnumCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        context.syntax.expression()
    }

    fn collect(
        &self,
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) {
        semantics::visit_visible_globals(context, |symbol| {
            if matches!(symbol.kind, SymbolKind::Enum { .. }) {
                let SymbolKind::Enum { members } = &symbol.kind else {
                    return;
                };
                for member in members {
                    let candidate = items::prefixed_enum_member(context, symbol, member);
                    collector.add(candidate.item, candidate.rank);
                }
            }
        });
    }
}
