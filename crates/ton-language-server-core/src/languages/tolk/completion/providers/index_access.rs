use super::TolkCompletionProviderContext;
use crate::completion::{
    CompletionCategory, CompletionCollector, CompletionProvider, CompletionRank,
};
use crate::languages::tolk::completion::semantics;
use crate::{CompletionItem, CompletionItemKind};
use tolk_ty::TyData;

pub(crate) struct IndexAccessCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for IndexAccessCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        context.syntax.after_dot
    }

    fn collect(
        &self,
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) {
        let Some(cursor) = context.syntax.cursor_node() else {
            return;
        };
        let Some(dot_access) = cursor
            .parent()
            .filter(|parent| parent.kind() == "dot_access")
        else {
            return;
        };
        let Some(qualifier) = dot_access.child_by_field_name("obj") else {
            return;
        };
        let Some(ty) = semantics::type_of_node(context, qualifier) else {
            return;
        };
        let ty = context.snapshot.type_interner.unwrap_alias(ty);
        let (TyData::Tuple(elements) | TyData::Tensor(elements)) =
            context.snapshot.type_interner.data(ty)
        else {
            return;
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
    }
}
