use super::TolkCompletionProviderContext;
use crate::completion::{
    CompletionCategory, CompletionCollector, CompletionProvider, CompletionRank,
};
use crate::{CompletionItem, CompletionItemKind};
use tolk_resolver::SymbolKind;

/// Completes non-test get-method names in the method-name argument of
/// `net.runGetMethod`.
///
/// Test-only methods are excluded because they are not callable through this
/// Acton runtime API.
pub(crate) struct ActonGetMethodCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for ActonGetMethodCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        super::matches_call(context.syntax, "runGetMethod", Some("net"), 1)
    }

    fn collect(
        &self,
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) -> Option<()> {
        let (prefix, range) = super::string_prefix_and_range(context.syntax, context.document)?;
        for (file, symbol) in context
            .snapshot
            .project_index
            .files()
            .values()
            .filter(|file| !file.path.to_string_lossy().contains(".acton"))
            .flat_map(|file| file.decls.iter().map(move |symbol| (file, symbol)))
            .filter(|(_, symbol)| {
                matches!(symbol.kind, SymbolKind::GetMethod { .. })
                    && !tolk_syntax::is_test_get_method_name(symbol.name.as_ref())
            })
        {
            collector.add(
                CompletionItem::new(symbol.name.as_ref(), CompletionItemKind::Method)
                    .with_label_detail(format!(
                        " {}",
                        file.path
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or_default()
                    ))
                    .with_replacement(range, symbol.name.as_ref()),
                CompletionRank::new(CompletionCategory::Function)
                    .with_prefix(&prefix, symbol.name.as_ref()),
            );
        }
        Some(())
    }
}
