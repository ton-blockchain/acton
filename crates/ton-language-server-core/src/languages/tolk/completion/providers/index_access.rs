use super::TolkCompletionProviderContext;
use crate::completion::{
    CompletionCategory, CompletionCollector, CompletionProvider, CompletionRank,
};
use crate::{CompletionItem, CompletionItemKind};
use tolk_syntax::DotAccess;
use tolk_ty::TyData;

/// Completes numeric tuple and tensor indexes after a dot access.
///
/// The receiver must have a known tuple or tensor type; an empty tuple and unknown
/// receiver do not produce speculative index suggestions.
pub(crate) struct IndexAccessCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for IndexAccessCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        context.syntax.after_dot
    }

    fn collect(
        &self,
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) -> Option<()> {
        let dot_access = context.syntax.parent_as::<DotAccess>()?;
        let qualifier = dot_access.obj()?;

        let ty = context.type_of_node(qualifier)?;
        let ty = context.snapshot.type_interner.unwrap_alias(ty);

        let (TyData::Tuple(elements) | TyData::Tensor(elements)) =
            context.snapshot.type_interner.data(ty)
        else {
            return None;
        };

        for index in 0..elements.len() {
            let label = index.to_string();
            collector.add(
                CompletionItem::new(&label, CompletionItemKind::Field)
                    .with_replacement(context.syntax.replacement_range, &label),
                CompletionRank::new(CompletionCategory::Field)
                    .with_prefix(&context.syntax.prefix, &label),
            );
        }
        Some(())
    }
}
