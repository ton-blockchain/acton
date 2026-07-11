use super::TolkCompletionContext;
use crate::completion::{CompletionCategory, CompletionCollector, CompletionRank};
use crate::{CompletionItem, CompletionItemKind};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProviderGroup {
    None,
    Annotation,
    Contract,
    Enum,
    FieldModifier,
    FunctionName,
    TopLevel,
    General,
}

pub(crate) fn provider_group(context: &TolkCompletionContext) -> ProviderGroup {
    if context.inside_import() || context.inside_string() {
        ProviderGroup::None
    } else if context.is_annotation_name() {
        ProviderGroup::Annotation
    } else if context.in_contract_field_value() {
        ProviderGroup::General
    } else if context.contract_top_level() {
        ProviderGroup::Contract
    } else if context.enum_top_level() {
        ProviderGroup::Enum
    } else if context.expect_field_modifier() {
        ProviderGroup::FieldModifier
    } else if context.is_function_name() {
        ProviderGroup::FunctionName
    } else if context.after_dot {
        ProviderGroup::None
    } else if context.top_level() {
        ProviderGroup::TopLevel
    } else {
        ProviderGroup::General
    }
}

pub(super) fn add_keyword(
    context: &TolkCompletionContext,
    collector: &mut CompletionCollector,
    label: &str,
    insertion: &str,
) {
    collector.add(
        CompletionItem::new(label, CompletionItemKind::Keyword)
            .with_replacement(context.replacement_range, insertion),
        CompletionRank::new(CompletionCategory::Keyword).with_prefix(&context.prefix, label),
    );
}

pub(super) fn add_snippet(
    context: &TolkCompletionContext,
    collector: &mut CompletionCollector,
    label: &str,
    snippet: &str,
    category: CompletionCategory,
) {
    collector.add(
        CompletionItem::new(label, CompletionItemKind::Snippet)
            .with_snippet_replacement(context.replacement_range, snippet),
        CompletionRank::new(category).with_prefix(&context.prefix, label),
    );
}
