use super::TolkCompletionProviderContext;
use super::support::{ProviderGroup, provider_group};
use crate::completion::{
    CompletionCategory, CompletionCollector, CompletionProvider, CompletionRank,
};
use crate::{CompletionItem, CompletionItemKind};

pub(crate) struct ContractFieldCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for ContractFieldCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        provider_group(context.syntax) == ProviderGroup::Contract
    }

    fn collect(
        &self,
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) {
        let existing = context
            .syntax
            .ancestor("contract_body")
            .map(|body| {
                let mut cursor = body.walk();
                body.children(&mut cursor)
                    .filter(|node| node.kind() == "contract_field")
                    .filter_map(|node| node.child_by_field_name("name"))
                    .filter_map(|node| node.utf8_text(context.syntax.source().as_bytes()).ok())
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for &(label, detail) in CONTRACT_FIELDS {
            if existing.iter().any(|field| field == label) {
                continue;
            }
            let insertion = format!("{label}: $0");
            let item = CompletionItem::new(label, CompletionItemKind::Field)
                .with_detail(detail)
                .with_snippet_replacement(context.syntax.replacement_range, insertion);
            collector.add(
                item,
                CompletionRank::new(CompletionCategory::ContextElement)
                    .with_prefix(&context.syntax.prefix, label),
            );
        }
    }
}

const CONTRACT_FIELDS: &[(&str, &str)] = &[
    ("author", "Author of the contract"),
    ("version", "Version of the contract"),
    ("description", "Description of the contract"),
    ("incomingMessages", "Allowed incoming messages type"),
    (
        "incomingExternal",
        "Allowed incoming external messages type",
    ),
    ("storage", "Persistent storage structure"),
    ("storageAtDeployment", "Storage structure at deployment"),
    ("forceAbiExport", "Symbols additionally exported to ABI"),
];
