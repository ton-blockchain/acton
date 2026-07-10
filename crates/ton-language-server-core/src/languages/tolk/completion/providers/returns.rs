use super::TolkCompletionProviderContext;
use super::support::{ProviderGroup, provider_group};
use crate::completion::{
    CompletionCategory, CompletionCollector, CompletionProvider, CompletionRank,
};
use crate::{CompletionItem, CompletionItemKind};
use tolk_ty::{TyData, TyId};

pub(crate) struct ReturnCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for ReturnCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        provider_group(context.syntax) == ProviderGroup::General && context.syntax.is_statement()
    }

    fn collect(
        &self,
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) {
        let return_ty = enclosing_return_type(context);
        if return_ty
            .is_some_and(|ty| matches!(context.snapshot.type_interner.data(ty), TyData::Void))
        {
            add_return(context, collector, "return;", "return;");
            return;
        }

        add_return(context, collector, "return <expr>;", "return $0;");
        let Some(return_ty) = return_ty else {
            return;
        };
        let return_ty = context.snapshot.type_interner.unwrap_alias(return_ty);
        match context.snapshot.type_interner.data(return_ty) {
            TyData::Bool { .. } => {
                add_return(context, collector, "return true;", "return true;");
                add_return(context, collector, "return false;", "return false;");
            }
            TyData::Int(_) => add_return(context, collector, "return 0;", "return 0;"),
            _ if contains_null(context, return_ty) => {
                add_return(context, collector, "return null;", "return null;");
            }
            _ => {}
        }
    }
}

fn enclosing_return_type(context: &TolkCompletionProviderContext<'_>) -> Option<TyId> {
    let file = context.snapshot.file_db.get_by_id(context.file_id)?;
    let symbol = file.find_symbol_at(context.syntax.offset)?;
    let function_ty = context.snapshot.type_db_cache.top_level_type(symbol.id)?;
    match context.snapshot.type_interner.data(function_ty) {
        TyData::Func { return_ty, .. } => Some(*return_ty),
        _ => None,
    }
}

fn contains_null(context: &TolkCompletionProviderContext<'_>, ty: TyId) -> bool {
    let ty = context.snapshot.type_interner.unwrap_alias(ty);
    match context.snapshot.type_interner.data(ty) {
        TyData::Null => true,
        TyData::Union(types) => types.iter().any(|ty| contains_null(context, *ty)),
        _ => false,
    }
}

fn add_return(
    context: &TolkCompletionProviderContext<'_>,
    collector: &mut CompletionCollector,
    label: &str,
    snippet: &str,
) {
    collector.add(
        CompletionItem::new(label, CompletionItemKind::Keyword)
            .with_snippet_replacement(context.syntax.replacement_range, snippet),
        CompletionRank::new(CompletionCategory::Keyword).with_prefix(&context.syntax.prefix, label),
    );
}
