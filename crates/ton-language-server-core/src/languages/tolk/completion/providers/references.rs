use super::TolkCompletionProviderContext;
use crate::completion::{
    CompletionCategory, CompletionCollector, CompletionProvider, CompletionRank,
};
use crate::languages::tolk::completion::{items, semantics};
use crate::languages::tolk::import_edits;
use crate::{CompletionItem, CompletionItemKind};
use rustc_hash::{FxHashMap, FxHashSet};
use tolk_resolver::resolve_index::Resolved;
use tolk_resolver::symbol_resolver::GlobalEnv;
use tolk_resolver::{Symbol, SymbolKind};
use tolk_syntax::{DotAccess, Expr, HasName, ObjectLit};
use tolk_ty::{TyData, TyId, TypeDb, method_ids_for_completion};

/// Completes names visible in the current expression context.
/// This includes locals, globals, fields, enum members, methods, and struct fields.
/// Unique symbols from other files also receive an auto-import edit.
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
    ) -> Option<()> {
        if context.syntax.after_dot {
            Self::collect_member_completions(context, collector)?;
            return Some(());
        }
        if context.syntax.in_name_of_field_init() {
            Self::collect_struct_initializer_fields(context, collector)?;
            return Some(());
        }
        for local in context.visible_locals() {
            if let Some(candidate) = items::local(context, local) {
                collector.add(candidate.item, candidate.rank);
            }
        }
        let mut visible_symbols = FxHashSet::default();
        for symbol in context.visible_globals() {
            visible_symbols.insert(symbol.id);
            if allowed_global(symbol, context.syntax.is_type())
                && let Some(candidate) = items::symbol(context, symbol, false)
            {
                collector.add(candidate.item, candidate.rank);
            }
        }
        Self::collect_auto_imports(context, &visible_symbols, collector);
        Some(())
    }
}

impl ReferenceCompletionProvider {
    fn collect_auto_imports(
        context: &TolkCompletionProviderContext<'_>,
        visible_symbols: &FxHashSet<tolk_resolver::SymbolId>,
        collector: &mut CompletionCollector,
    ) {
        let project_index = &context.snapshot.project_index;
        let is_type = context.syntax.is_type();
        let mut import_edits = FxHashMap::default();

        for symbol_ids in project_index.global_symbols().values() {
            let mut symbols = symbol_ids.iter().filter_map(|symbol_id| {
                let symbol = project_index.resolve_symbol(*symbol_id)?;
                (!visible_symbols.contains(symbol_id) && allowed_global(symbol, is_type))
                    .then_some(symbol)
            });

            let Some(symbol) = symbols.next() else {
                continue;
            };
            let Some(mut candidate) = items::symbol(context, symbol, false) else {
                continue;
            };
            if symbols.next().is_none()
                && let Some(edit) = import_edits
                    .entry(symbol.id.file_id)
                    .or_insert_with(|| Self::auto_import_edit(context, symbol))
                    .clone()
            {
                candidate.item = candidate.item.with_additional_text_edit(edit);
            }
            collector.add(candidate.item, candidate.rank);
        }
    }

    fn with_auto_import(
        context: &TolkCompletionProviderContext<'_>,
        symbol: &Symbol,
        item: CompletionItem,
    ) -> CompletionItem {
        let Some(edit) = Self::auto_import_edit(context, symbol) else {
            return item;
        };

        item.with_additional_text_edit(edit)
    }

    fn auto_import_edit(
        context: &TolkCompletionProviderContext<'_>,
        symbol: &Symbol,
    ) -> Option<crate::TextEdit> {
        if symbol.id.file_id == context.file_id
            || context
                .snapshot
                .project_index
                .imports()
                .get(&context.file_id)
                .into_iter()
                .flatten()
                .filter_map(tolk_resolver::ResolvedImport::target)
                .any(|target| target == symbol.id.file_id)
        {
            return None;
        }

        let target = context
            .snapshot
            .project_index
            .files()
            .get(&symbol.id.file_id)?;
        if target.is_stdlib_prelude() {
            return None;
        }
        let import_path = import_edits::import_path_for(
            context.snapshot,
            context.file_id,
            &target.path,
            context.workspace.stdlib_path,
            context.workspace.mappings,
        )?;

        Some(import_edits::import_edit(
            context.document,
            context.syntax.root(),
            &import_path,
        ))
    }

    fn collect_member_completions(
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) -> Option<()> {
        let dot_access = context.syntax.parent_as::<DotAccess>()?;
        let qualifier = dot_access.obj()?;
        let qualifier_node = qualifier.syntax();

        let static_lookup = match qualifier {
            Expr::Instantiation(instantiation) => instantiation
                .expr()
                .map_or(qualifier_node, |expression| expression.syntax()),
            _ => qualifier_node,
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
            if matches!(qualifier, Expr::Instantiation(_)) {
                return None;
            }
            let name = context.syntax.text_of(qualifier);
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
            if let Some(receiver_ty) = context
                .type_of_node(qualifier)
                .or_else(|| context.snapshot.type_db_cache.top_level_type(symbol.id))
            {
                Self::collect_methods(context, receiver_ty, false, collector);
            }
            return Some(());
        }

        let receiver_ty = context.type_of_node(qualifier)?;
        if matches!(qualifier, Expr::Instantiation(_)) {
            Self::collect_methods(context, receiver_ty, false, collector);
            return Some(());
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
        Some(())
    }

    fn collect_type_members(
        context: &TolkCompletionProviderContext<'_>,
        symbol: &Symbol,
        collector: &mut CompletionCollector,
    ) {
        match &symbol.kind {
            SymbolKind::Struct { fields, .. } => {
                for field in fields {
                    if field.is_private {
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
        let clone_started_at = context.profiler.start();
        let mut interner = context.snapshot.type_interner.as_ref().clone();
        context
            .profiler
            .finish("tolk.completion.methods.clone_interner", clone_started_at);

        let query_db_started_at = context.profiler.start();
        let mut type_db = TypeDb::new_for_query(
            &mut interner,
            &context.snapshot.file_db,
            &context.snapshot.project_index,
            &context.snapshot.type_db_cache,
        );
        context
            .profiler
            .finish("tolk.completion.methods.query_db", query_db_started_at);

        let resolve_started_at = context.profiler.start();
        let method_ids = method_ids_for_completion(receiver_ty, instance, &mut type_db);
        context
            .profiler
            .finish("tolk.completion.methods.resolve", resolve_started_at);

        let items_started_at = context.profiler.start();
        for method_id in method_ids {
            if let Some(symbol) = context.snapshot.project_index.resolve_symbol(method_id)
                && let Some(mut candidate) = items::symbol(context, symbol, true)
            {
                tracing::trace!(
                    target: crate::logging::TOLK_TARGET,
                    operation = "tolk.completion.method_candidate",
                    method = symbol.fqn.as_ref(),
                    provided_receiver = %context.snapshot.type_interner.display(receiver_ty),
                    declared_receiver = context
                        .snapshot
                        .type_db_cache
                        .method_receiver_type(method_id)
                        .map(|ty| context.snapshot.type_interner.format(ty)),
                    "method completion candidate accepted"
                );
                candidate.item = Self::with_auto_import(context, symbol, candidate.item);
                collector.add(candidate.item, candidate.rank);
            }
        }
        context
            .profiler
            .finish("tolk.completion.methods.items", items_started_at);
    }

    fn collect_struct_initializer_fields(
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) -> Option<()> {
        let object = context.syntax.ancestor_as::<ObjectLit>()?;
        let struct_symbol = Self::object_struct(context, object)?;
        let SymbolKind::Struct { fields, .. } = &struct_symbol.kind else {
            return None;
        };
        let initialized = object
            .arguments()
            .filter_map(|argument| argument.name())
            .map(|name| context.syntax.text_of(name))
            .collect::<Vec<_>>();
        let field_names = fields
            .iter()
            .map(|field| field.name.as_ref())
            .collect::<FxHashSet<_>>();
        for field in fields {
            if initialized.iter().any(|name| *name == field.name.as_ref()) || field.is_private {
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
        for local in context.visible_locals() {
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
        }
        Some(())
    }

    fn object_struct<'a>(
        context: &'a TolkCompletionProviderContext<'_>,
        object: ObjectLit<'_>,
    ) -> Option<&'a Symbol> {
        if let Some(ty) = context.type_of_node(object) {
            let ty = context.snapshot.type_interner.unwrap_alias(ty);
            if let TyData::Struct { def, .. } = context.snapshot.type_interner.data(ty) {
                return context.snapshot.project_index.resolve_symbol(*def);
            }
        }

        let type_name = context.syntax.text_of(object.typ()?);
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
