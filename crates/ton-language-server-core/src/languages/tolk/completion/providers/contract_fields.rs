use super::TolkCompletionProviderContext;
use super::support::{ProviderGroup, provider_group};
use crate::completion::{
    CompletionCategory, CompletionCollector, CompletionProvider, CompletionRank,
};
use crate::languages::tolk::completion::contract::CONTRACT_FIELDS;
use crate::{CompletionItem, CompletionItemKind};
use tolk_syntax::{ContractBody, HasName};

/// Completes Acton contract metadata fields at the top level of a contract body.
///
/// The provider reads existing typed contract fields and omits fields already
/// declared in the same body.
pub(crate) struct ContractFieldCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for ContractFieldCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        provider_group(context.syntax) == ProviderGroup::Contract
    }

    fn collect(
        &self,
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) -> Option<()> {
        let existing = context
            .syntax
            .ancestor_as::<ContractBody>()
            .map(|body| {
                body.fields()
                    .filter_map(|field| field.name())
                    .map(|name| context.syntax.text_of(name).to_owned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for field in CONTRACT_FIELDS {
            let label = field.name;
            if existing.iter().any(|field| field == label) {
                continue;
            }
            let insertion = format!("{label}: $0");
            let item = CompletionItem::new(label, CompletionItemKind::Field)
                .with_detail(field.detail)
                .with_snippet_replacement(context.syntax.replacement_range, insertion);
            collector.add(
                item,
                CompletionRank::new(CompletionCategory::ContextElement)
                    .with_prefix(&context.syntax.prefix, label),
            );
        }
        Some(())
    }
}
