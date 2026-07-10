use super::{CodeActionProvider, TolkCodeActionContext};
use crate::CodeAction;
use crate::languages::tolk::import_edits;
use std::collections::BTreeSet;
use tolk_resolver::{Resolved, Symbol, SymbolKind};
use tolk_syntax::{AstNode, Ident, TypeIdent};

/// Adds an import for one uniquely named, non-stdlib declaration from another workspace file.
pub(super) struct AddImportProvider;

impl CodeActionProvider for AddImportProvider {
    fn collect(
        &self,
        context: &TolkCodeActionContext<'_>,
        actions: &mut Vec<CodeAction>,
    ) -> Option<()> {
        let identifier = context
            .ancestor_as::<Ident>()
            .map(|identifier| identifier.syntax())
            .or_else(|| {
                context
                    .ancestor_as::<TypeIdent>()
                    .map(|identifier| identifier.syntax())
            })?;
        let imported_targets = context
            .snapshot
            .project_index
            .imports()
            .get(&context.file_id)
            .into_iter()
            .flatten()
            .filter_map(tolk_resolver::ResolvedImport::target)
            .collect::<BTreeSet<_>>();
        let already_visible = match context
            .snapshot
            .resolved_at(context.file_id, identifier.start_byte())
        {
            Some(Resolved::Local(_)) => true,
            Some(Resolved::Global(symbol_id)) => {
                symbol_id.file_id == context.file_id
                    || imported_targets.contains(&symbol_id.file_id)
                    || context.snapshot.file_db.is_stdlib_file(symbol_id.file_id)
            }
            Some(Resolved::Unresolved) | None => false,
        };
        if already_visible {
            return None;
        }
        let name = context.text_of(identifier);
        let candidates = context
            .snapshot
            .project_index
            .files()
            .values()
            .flat_map(|file| file.decls.iter())
            .filter(|symbol| symbol.name.as_ref() == name && importable(symbol))
            .collect::<Vec<_>>();
        let [symbol] = candidates.as_slice() else {
            return None;
        };
        if symbol.id.file_id == context.file_id
            || context.snapshot.file_db.is_stdlib_file(symbol.id.file_id)
        {
            return None;
        }
        if imported_targets.contains(&symbol.id.file_id) {
            return None;
        }
        let target = context
            .snapshot
            .project_index
            .files()
            .get(&symbol.id.file_id)?;
        let import_path = import_edits::import_path_for(
            context.snapshot,
            context.file_id,
            &target.path,
            &context.config.stdlib_path,
            context.config.import_mappings.as_ref(),
        )?;
        let edit = import_edits::import_edit(
            context.document,
            context.file.source().tree.root_node(),
            &import_path,
        );
        actions.push(context.action("Import symbol from other file", edit));
        Some(())
    }
}

fn importable(symbol: &Symbol) -> bool {
    matches!(
        symbol.kind,
        SymbolKind::GlobalVariable
            | SymbolKind::Function { .. }
            | SymbolKind::Struct { .. }
            | SymbolKind::Enum { .. }
            | SymbolKind::Constant
            | SymbolKind::TypeAlias { .. }
    )
}
