use super::TolkCompletionProviderContext;
use super::support::{ProviderGroup, provider_group};
use crate::completion::{
    CompletionCategory, CompletionCollector, CompletionProvider, CompletionRank,
};
use crate::{CompletionItem, CompletionItemKind};

/// Completes variable-width integer, byte, and bit type spellings.
///
/// Each item inserts a width placeholder, allowing the user to choose the size
/// without manually rebuilding the type name.
pub(crate) struct VariableSizeTypeCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for VariableSizeTypeCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        provider_group(context.syntax) == ProviderGroup::General
            && (context.syntax.is_type() || context.syntax.expression())
    }

    fn collect(
        &self,
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) -> Option<()> {
        for &label in VARIABLE_SIZE_TYPES {
            let insertion = label.replace("{X}", "${1:32}");
            collector.add(
                CompletionItem::new(label, CompletionItemKind::TypeParameter)
                    .with_snippet_replacement(context.syntax.replacement_range, insertion),
                CompletionRank::new(CompletionCategory::TypeAlias)
                    .with_prefix(&context.syntax.prefix, label),
            );
        }
        Some(())
    }
}

const VARIABLE_SIZE_TYPES: &[&str] = &[
    "uint8", "uint16", "uint32", "uint64", "uint128", "uint256", "int8", "int16", "int32", "int64",
    "int128", "int256", "int257", "int{X}", "uint{X}", "bytes32", "bytes{X}", "bits256", "bits{X}",
];
