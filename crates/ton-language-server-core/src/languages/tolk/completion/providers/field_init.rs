use super::TolkCompletionProviderContext;
use crate::completion::{
    CompletionCategory, CompletionCollector, CompletionProvider, CompletionRank,
};
use crate::{CompletionItem, CompletionItemKind};
use tolk_resolver::symbol_resolver::GlobalEnv;
use tolk_resolver::{Symbol, SymbolKind};
use tolk_syntax::{StructField, TryFromNode, Type};

pub(crate) struct FieldInitCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for FieldInitCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        context.syntax.in_field_init_value()
    }

    fn collect(
        &self,
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) {
        let Some(struct_field) = resolve_initialized_field(context) else {
            return;
        };
        let Some(inner_struct_name) = cell_struct_name(context, struct_field) else {
            return;
        };
        let label = format!("{inner_struct_name} {{}}.toCell()");
        let snippet = format!("{inner_struct_name} {{$0}}.toCell()");
        collector.add(
            CompletionItem::new(&label, CompletionItemKind::Snippet)
                .with_snippet_replacement(context.syntax.replacement_range, snippet),
            CompletionRank::new(CompletionCategory::ContextElement)
                .with_prefix(&context.syntax.prefix, &label),
        );
    }
}

fn resolve_initialized_field<'a>(
    context: &'a TolkCompletionProviderContext<'_>,
) -> Option<&'a Symbol> {
    let argument = context.syntax.cursor_node()?.parent()?;
    let field_name = argument
        .child_by_field_name("name")?
        .utf8_text(context.syntax.source().as_bytes())
        .ok()?;
    let object = context.syntax.ancestor("object_literal")?;
    let type_node = object
        .child_by_field_name("type")
        .or_else(|| object.named_child(0))?;
    let type_name = type_node
        .utf8_text(context.syntax.source().as_bytes())
        .ok()?
        .trim();
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
    let source_file = file.source();
    let name = source_file
        .tree
        .root_node()
        .descendant_for_byte_range(field.name_span.start(), field.name_span.end())?;
    let node = name.parent()?;
    let field = StructField::try_from_node(node).ok()?;
    let Type::TypeInstantiatedTs(instantiated) = field.typ()? else {
        return None;
    };
    if instantiated.name()?.text(source_file.source.as_ref()) != "Cell" {
        return None;
    }
    let Type::TypeIdent(inner) = instantiated.arguments()?.types().next()? else {
        return None;
    };
    let inner_name = inner.text(source_file.source.as_ref());
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
