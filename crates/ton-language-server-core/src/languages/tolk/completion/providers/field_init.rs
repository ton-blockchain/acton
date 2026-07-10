use super::TolkCompletionProviderContext;
use crate::completion::{
    CompletionCategory, CompletionCollector, CompletionProvider, CompletionRank,
};
use crate::{CompletionItem, CompletionItemKind};
use tolk_resolver::symbol_resolver::GlobalEnv;
use tolk_resolver::{Symbol, SymbolKind};
use tolk_syntax::{HasName, InstanceArg, ObjectLit, StructField, TryFromNode, Type};

/// Completes `Struct {}.toCell()` for fields expecting a `Cell<Struct>` value.
///
/// The struct type is inferred from the initialized field, so the snippet is only
/// offered when the expected type is a compatible cell-backed struct.
pub(crate) struct FieldInitCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for FieldInitCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        context.syntax.in_field_init_value()
    }

    fn collect(
        &self,
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) -> Option<()> {
        let struct_field = resolve_initialized_field(context)?;
        let inner_struct_name = cell_struct_name(context, struct_field)?;
        let label = format!("{inner_struct_name} {{}}.toCell()");
        let snippet = format!("{inner_struct_name} {{$0}}.toCell()");
        collector.add(
            CompletionItem::new(&label, CompletionItemKind::Snippet)
                .with_snippet_replacement(context.syntax.replacement_range, snippet),
            CompletionRank::new(CompletionCategory::ContextElement)
                .with_prefix(&context.syntax.prefix, &label),
        );
        Some(())
    }
}

fn resolve_initialized_field<'a>(
    context: &'a TolkCompletionProviderContext<'_>,
) -> Option<&'a Symbol> {
    let argument = context.syntax.parent_as::<InstanceArg>()?;
    let field_name = context.syntax.text_of(argument.name()?);
    let object = context.syntax.ancestor_as::<ObjectLit>()?;
    let type_name = context.syntax.text_of(object.typ()?);

    let env = GlobalEnv::new(&context.snapshot.project_index, context.file_id);
    let owner = env.visible.get(type_name)?.iter().find_map(|id| {
        context
            .snapshot
            .project_index
            .resolve_symbol(*id)
            .filter(|symbol| matches!(symbol.kind, SymbolKind::Struct { .. }))
    })?;

    let SymbolKind::Struct { fields, .. } = &owner.kind else {
        return None;
    };
    fields
        .iter()
        .find(|field| field.name.as_ref() == field_name)
}

fn cell_struct_name(context: &TolkCompletionProviderContext<'_>, field: &Symbol) -> Option<String> {
    let field_file_id = field.id.file_id;
    let file = context.snapshot.file_db.get_by_id(field_file_id)?;
    let name = file.find_node_at_span(field.name_span)?;
    let node = name.parent()?;
    let field = StructField::try_from_node(node).ok()?;
    let Type::TypeInstantiatedTs(instantiated) = field.typ()? else {
        return None;
    };
    if file.text(&instantiated.name()?) != "Cell" {
        return None;
    }
    let Type::TypeIdent(inner) = instantiated.arguments()?.types().next()? else {
        return None;
    };
    let inner_name = file.text(&inner);
    let env = GlobalEnv::new(&context.snapshot.project_index, field_file_id);
    env.visible.get(inner_name)?.iter().find_map(|id| {
        context
            .snapshot
            .project_index
            .resolve_symbol(*id)
            .filter(|symbol| matches!(symbol.kind, SymbolKind::Struct { .. }))
            .map(|_| inner_name.to_owned())
    })
}
