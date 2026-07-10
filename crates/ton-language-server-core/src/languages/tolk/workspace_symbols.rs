use super::{TolkWorkspaceEngine, document_symbols::symbol_kind, file_info::FileInfoExt};
use crate::{Location, WorkspaceSymbol};
use tolk_resolver::{FileInfo, Symbol, SymbolKind};

impl TolkWorkspaceEngine {
    pub(super) fn workspace_symbols(&self, query: &str) -> Vec<WorkspaceSymbol> {
        let (files, file_db) = {
            let state = self.state.read().expect("Tolk workspace lock poisoned");
            (state.files.clone(), state.file_db.clone())
        };
        let query = query.to_ascii_lowercase();
        let mut symbols = Vec::new();

        for (path, workspace_file) in &files {
            let Some(uri) = workspace_file.active_uri() else {
                continue;
            };
            let Some(file) = file_db.get_by_path(path) else {
                continue;
            };
            for symbol in &file.index().decls {
                collect_symbol(symbol, file.as_ref(), &uri, &query, &mut symbols);
            }
        }
        symbols
    }
}

fn collect_symbol(
    symbol: &Symbol,
    file: &FileInfo,
    uri: &crate::DocumentUri,
    query: &str,
    symbols: &mut Vec<WorkspaceSymbol>,
) {
    let name = workspace_symbol_name(symbol);
    if query.is_empty() || name.to_ascii_lowercase().contains(query) {
        symbols.push(WorkspaceSymbol::new(
            name,
            symbol_kind(&symbol.kind),
            Location::new(uri.clone(), file.range_for_span(symbol.name_span)),
        ));
    }

    if let SymbolKind::Enum { members } = &symbol.kind {
        for member in members {
            let member_name = format!("{}.{}", symbol.name, member.name);
            if query.is_empty() || member_name.to_ascii_lowercase().contains(query) {
                symbols.push(WorkspaceSymbol::new(
                    member_name,
                    symbol_kind(&member.kind),
                    Location::new(uri.clone(), file.range_for_span(member.name_span)),
                ));
            }
        }
    }
}

fn workspace_symbol_name(symbol: &Symbol) -> String {
    match symbol.kind {
        SymbolKind::Method { .. } => symbol.fqn.to_string(),
        SymbolKind::GetMethod { .. } => format!("get {}", symbol.name),
        _ => symbol.name.to_string(),
    }
}
