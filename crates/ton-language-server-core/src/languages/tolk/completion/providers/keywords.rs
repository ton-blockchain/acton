use super::TolkCompletionProviderContext;
use super::support::{ProviderGroup, add_keyword, provider_group};
use crate::completion::{CompletionCollector, CompletionProvider};

pub(crate) struct KeywordCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for KeywordCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        provider_group(context.syntax) == ProviderGroup::General
            && context.syntax.expression()
            && !context.syntax.in_name_of_field_init()
    }

    fn collect(
        &self,
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) {
        for keyword in ["true", "false"] {
            add_keyword(context.syntax, collector, keyword, keyword);
        }
        for keyword in ["lazy", "as", "is", "mutate"] {
            add_keyword(context.syntax, collector, keyword, &format!("{keyword} "));
        }
    }
}
