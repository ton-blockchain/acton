use super::TolkCompletionProviderContext;
use crate::completion::{
    CompletionCategory, CompletionCollector, CompletionProvider, CompletionRank,
};
use crate::languages::tolk::completion::{items, semantics};
use crate::{CompletionItem, CompletionItemKind, Range, TextEdit};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use tolk_resolver::resolve_index::Resolved;
use tolk_resolver::symbol_resolver::GlobalEnv;
use tolk_resolver::{Symbol, SymbolKind};
use tolk_syntax::{HasName, ObjectLit, TryFromNode};
use tolk_ty::{TyData, TyId, TypeDb, method_ids_for_completion};

pub(crate) struct ReferenceCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for ReferenceCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        let syntax = context.syntax;
        (syntax.after_dot && !syntax.is_function_name())
            || !(syntax.inside_import()
                || syntax.is_annotation_name()
                || syntax.is_function_name()
                || syntax.is_declaration_name()
                || syntax.is_catch_variable()
                || syntax.expect_field_modifier()
                || syntax.expect_match_arm()
                || syntax.inside_string()
                || syntax.top_level()
                || (syntax.struct_top_level() && !syntax.is_type())
                || syntax.enum_top_level()
                || syntax.contract_top_level())
    }

    fn collect(
        &self,
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) {
        if context.syntax.after_dot {
            Self::collect_member_completions(context, collector);
            return;
        }
        if context.syntax.in_name_of_field_init() {
            Self::collect_struct_initializer_fields(context, collector);
            return;
        }
        semantics::visit_visible_locals(context, |local| {
            if let Some(candidate) = items::local(context, local) {
                collector.add(candidate.item, candidate.rank);
            }
        });
        let mut visible_symbols = BTreeSet::new();
        semantics::visit_visible_globals(context, |symbol| {
            visible_symbols.insert(symbol.id);
            if allowed_global(symbol, context.syntax.is_type())
                && let Some(candidate) = items::symbol(context, symbol, false)
            {
                collector.add(candidate.item, candidate.rank);
            }
        });
        Self::collect_auto_imports(context, &visible_symbols, collector);
    }
}

impl ReferenceCompletionProvider {
    fn collect_auto_imports(
        context: &TolkCompletionProviderContext<'_>,
        visible_symbols: &BTreeSet<tolk_resolver::SymbolId>,
        collector: &mut CompletionCollector,
    ) {
        let mut by_name = BTreeMap::<_, Vec<_>>::new();
        for file in context.snapshot.project_index.files().values() {
            if file.id == context.file_id {
                continue;
            }
            for symbol in &file.decls {
                if !visible_symbols.contains(&symbol.id)
                    && allowed_global(symbol, context.syntax.is_type())
                {
                    by_name.entry(symbol.name.clone()).or_default().push(symbol);
                }
            }
        }

        let imported_targets = context
            .snapshot
            .project_index
            .imports()
            .get(&context.file_id)
            .into_iter()
            .flatten()
            .filter_map(tolk_resolver::ResolvedImport::target)
            .collect::<BTreeSet<_>>();
        for symbols in by_name.into_values() {
            let Some(symbol) = symbols.first() else {
                continue;
            };
            let Some(mut candidate) = items::symbol(context, symbol, false) else {
                continue;
            };
            if symbols.len() == 1
                && !imported_targets.contains(&symbol.id.file_id)
                && let Some(target) = context
                    .snapshot
                    .project_index
                    .files()
                    .get(&symbol.id.file_id)
                && let Some(import_path) = import_path_for(context, &target.path)
            {
                candidate.item = candidate
                    .item
                    .with_additional_text_edit(import_edit(context, &import_path));
            }
            collector.add(candidate.item, candidate.rank);
        }
    }

    fn collect_member_completions(
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

        let static_lookup = if qualifier.kind() == "generic_instantiation" {
            qualifier
                .child_by_field_name("expr")
                .or_else(|| qualifier.named_child(0))
                .unwrap_or(qualifier)
        } else {
            qualifier
        };
        let resolved = context
            .snapshot
            .resolved_at(context.file_id, static_lookup.start_byte());
        let static_symbol = resolved.as_ref().and_then(|resolved| match resolved {
            Resolved::Global(symbol_id) => {
                context.snapshot.project_index.resolve_symbol(*symbol_id)
            }
            Resolved::Local(_) | Resolved::Unresolved => None,
        });
        let static_type = static_symbol.filter(|symbol| symbol.is_type()).or_else(|| {
            if qualifier.kind() == "generic_instantiation" {
                return None;
            }
            let name = qualifier
                .utf8_text(context.syntax.source().as_bytes())
                .ok()?
                .trim();
            GlobalEnv::new(&context.snapshot.project_index, context.file_id)
                .visible
                .get(name)?
                .iter()
                .find_map(|id| {
                    context
                        .snapshot
                        .project_index
                        .resolve_symbol(*id)
                        .filter(|symbol| symbol.is_type())
                })
        });
        if let Some(symbol) = static_type {
            if matches!(symbol.kind, SymbolKind::Enum { .. }) {
                Self::collect_type_members(context, symbol, collector);
            }
            if let Some(receiver_ty) = semantics::type_of_node(context, qualifier)
                .or_else(|| context.snapshot.type_db_cache.top_level_type(symbol.id))
            {
                Self::collect_methods(context, receiver_ty, false, collector);
            }
            return;
        }

        let Some(receiver_ty) = semantics::type_of_node(context, qualifier) else {
            return;
        };
        if qualifier.kind() == "generic_instantiation" {
            Self::collect_methods(context, receiver_ty, false, collector);
            return;
        }
        let ty = context.snapshot.type_interner.unwrap_alias(receiver_ty);
        match context.snapshot.type_interner.data(ty) {
            TyData::Struct { def, .. } | TyData::Enum { def, .. } => {
                if let Some(symbol) = context.snapshot.project_index.resolve_symbol(*def) {
                    Self::collect_type_members(context, symbol, collector);
                }
            }
            _ => {}
        }
        Self::collect_methods(context, receiver_ty, true, collector);
    }

    fn collect_type_members(
        context: &TolkCompletionProviderContext<'_>,
        symbol: &Symbol,
        collector: &mut CompletionCollector,
    ) {
        match &symbol.kind {
            SymbolKind::Struct { fields, .. } => {
                for field in fields {
                    if semantics::struct_field_is_private(context.snapshot, field) {
                        continue;
                    }
                    if let Some(candidate) = items::symbol(context, field, true) {
                        collector.add(candidate.item, candidate.rank);
                    }
                }
            }
            SymbolKind::Enum { members } => {
                for member in members {
                    if let Some(candidate) = items::symbol(context, member, true) {
                        collector.add(candidate.item, candidate.rank);
                    }
                }
            }
            _ => {}
        }
    }

    fn collect_methods(
        context: &TolkCompletionProviderContext<'_>,
        receiver_ty: TyId,
        instance: bool,
        collector: &mut CompletionCollector,
    ) {
        let mut interner = context.snapshot.type_interner.clone();
        let mut type_db = TypeDb::new_with_cache(
            &mut interner,
            &context.snapshot.file_db,
            &context.snapshot.project_index,
            context.snapshot.type_db_cache.clone(),
            std::iter::empty(),
        );
        for method_id in method_ids_for_completion(receiver_ty, instance, &mut type_db) {
            if let Some(symbol) = context.snapshot.project_index.resolve_symbol(method_id)
                && let Some(candidate) = items::symbol(context, symbol, true)
            {
                collector.add(candidate.item, candidate.rank);
            }
        }
    }

    fn collect_struct_initializer_fields(
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) {
        let Some(object) = context.syntax.ancestor("object_literal") else {
            return;
        };
        let Ok(object) = ObjectLit::try_from_node(object) else {
            return;
        };
        let Some(struct_symbol) = Self::object_struct(context, object) else {
            return;
        };
        let SymbolKind::Struct { fields, .. } = &struct_symbol.kind else {
            return;
        };
        let initialized = object
            .arguments()
            .filter_map(|argument| argument.name())
            .filter_map(|name| name.0.utf8_text(context.syntax.source().as_bytes()).ok())
            .map(str::trim)
            .collect::<Vec<_>>();
        let field_names = fields
            .iter()
            .map(|field| field.name.as_ref())
            .collect::<BTreeSet<_>>();
        for field in fields {
            if initialized.iter().any(|name| *name == field.name.as_ref())
                || semantics::struct_field_is_private(context.snapshot, field)
            {
                continue;
            }
            let raw_name = semantics::raw_text(context.snapshot, field.id.file_id, field.name_span)
                .unwrap_or_else(|| field.name.to_string());
            let comma = if context.syntax.in_multiline_struct_init() {
                ","
            } else {
                ""
            };
            let snippet = format!("{raw_name}: $1{comma}$0");
            collector.add(
                CompletionItem::new(&raw_name, CompletionItemKind::Property)
                    .with_snippet_replacement(context.syntax.replacement_range, snippet),
                CompletionRank::new(CompletionCategory::ContextElement)
                    .with_prefix(&context.syntax.prefix, field.name.as_ref()),
            );
        }
        semantics::visit_visible_locals(context, |local| {
            if field_names.contains(local.name.as_ref())
                && !initialized.iter().any(|name| *name == local.name.as_ref())
                && let Some(mut candidate) = items::local(context, local)
            {
                if context.syntax.in_multiline_struct_init()
                    && let Some(edit) = &mut candidate.item.text_edit
                {
                    edit.new_text.push(',');
                }
                collector.add(candidate.item, candidate.rank);
            }
        });
    }

    fn object_struct<'a>(
        context: &'a TolkCompletionProviderContext<'_>,
        object: ObjectLit<'_>,
    ) -> Option<&'a Symbol> {
        if let Some(ty) = semantics::type_of_node(context, object.0) {
            let ty = context.snapshot.type_interner.unwrap_alias(ty);
            if let TyData::Struct { def, .. } = context.snapshot.type_interner.data(ty) {
                return context.snapshot.project_index.resolve_symbol(*def);
            }
        }

        let type_name = object.typ()?.text(context.syntax.source()).trim();
        GlobalEnv::new(&context.snapshot.project_index, context.file_id)
            .visible
            .get(type_name)?
            .iter()
            .find_map(|id| {
                context
                    .snapshot
                    .project_index
                    .resolve_symbol(*id)
                    .filter(|symbol| matches!(symbol.kind, SymbolKind::Struct { .. }))
            })
    }
}

fn import_path_for(context: &TolkCompletionProviderContext<'_>, target: &Path) -> Option<String> {
    if let Ok(relative) = target.strip_prefix(context.workspace.stdlib_path) {
        return Some(format!("@stdlib/{}", path_without_tolk(relative)));
    }
    if let Some(mappings) = context.workspace.mappings {
        for (mapping, root) in mappings {
            if let Ok(relative) = target.strip_prefix(root) {
                let relative = path_without_tolk(relative);
                return Some(if relative.is_empty() {
                    mapping.clone()
                } else {
                    format!("{mapping}/{relative}")
                });
            }
        }
    }

    let current = context
        .snapshot
        .project_index
        .files()
        .get(&context.file_id)?;
    Some(path_without_tolk(&relative_path(
        current.path.parent()?,
        target,
    )?))
}

fn path_without_tolk(path: &Path) -> String {
    let mut path = path.to_path_buf();
    if path
        .extension()
        .is_some_and(|extension| extension == "tolk")
    {
        path.set_extension("");
    }
    path.to_string_lossy().replace('\\', "/")
}

fn relative_path(from: &Path, to: &Path) -> Option<PathBuf> {
    let from = from.components().collect::<Vec<_>>();
    let to = to.components().collect::<Vec<_>>();
    let common = from
        .iter()
        .zip(&to)
        .take_while(|(left, right)| left == right)
        .count();
    if common == 0 {
        return None;
    }
    let mut result = PathBuf::new();
    for component in &from[common..] {
        if !matches!(component, Component::CurDir) {
            result.push("..");
        }
    }
    for component in &to[common..] {
        result.push(component.as_os_str());
    }
    Some(result)
}

fn import_edit(context: &TolkCompletionProviderContext<'_>, import_path: &str) -> TextEdit {
    let root = context.syntax.root();
    let mut cursor = root.walk();
    let leading = root
        .named_children(&mut cursor)
        .take_while(|node| matches!(node.kind(), "tolk_required_version" | "import_directive"))
        .last();
    let (offset, text) = match leading {
        Some(node) if node.kind() == "import_directive" => {
            (node.end_byte(), format!("\nimport \"{import_path}\"\n"))
        }
        Some(node) => (node.end_byte(), format!("\n\nimport \"{import_path}\"\n")),
        None => (0, format!("import \"{import_path}\"\n\n")),
    };
    let position = context
        .document
        .text_index()
        .offset_to_position(context.document.text(), offset);
    TextEdit::new(Range::new(position, position), text)
}

fn allowed_global(symbol: &Symbol, is_type: bool) -> bool {
    if symbol.name.starts_with("__") {
        return false;
    }
    if is_type {
        return symbol.is_type();
    }
    if matches!(symbol.kind, SymbolKind::GetMethod { .. })
        && tolk_syntax::is_test_get_method_name(symbol.name.as_ref())
    {
        return false;
    }
    matches!(
        symbol.kind,
        SymbolKind::Function { .. }
            | SymbolKind::GetMethod { .. }
            | SymbolKind::Constant
            | SymbolKind::GlobalVariable
            | SymbolKind::Struct { .. }
            | SymbolKind::Enum { .. }
            | SymbolKind::TypeAlias { .. }
    )
}
