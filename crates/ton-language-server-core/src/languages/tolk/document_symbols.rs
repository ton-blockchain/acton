use super::{TolkWorkspaceEngine, file_info::FileInfoExt};
use crate::{DocumentSnapshot, DocumentSymbol, DocumentSymbolKind};
use tolk_resolver::{FileInfo, Symbol, SymbolKind};
use tolk_syntax::{
    AstNode, EnumMember, FunctionLike, HasGenericParams, StructField, TopLevel, TryFromNode,
};

impl TolkWorkspaceEngine {
    pub(super) fn document_symbols(&self, document: &DocumentSnapshot) -> Vec<DocumentSymbol> {
        let snapshot = {
            let state = self.state.read().expect("Tolk workspace lock poisoned");
            state.latest_snapshot.clone()
        };
        let Some(snapshot) = snapshot else {
            return Vec::new();
        };
        let Some(file_id) = snapshot.find_document_file(document) else {
            return Vec::new();
        };
        let Some(file) = snapshot.file_db.get_by_id(file_id) else {
            return Vec::new();
        };
        let source = file.source().source.as_ref();
        let mut symbols = Vec::new();

        if let Some(imports) = snapshot.project_index.imports_of(file_id) {
            symbols.extend(imports.into_iter().map(|import| {
                let range = file.range_for_span(import.import().span);
                let name = file.text_at(import.import().span).trim_end_matches(';');
                DocumentSymbol::new(name, DocumentSymbolKind::Module, range, range)
            }));
        }

        if let Some(file_index) = snapshot.project_index.get_file_index(file_id) {
            symbols.extend(
                file_index
                    .decls
                    .iter()
                    .map(|symbol| symbol_to_document_symbol(symbol, file.as_ref(), source)),
            );
        }
        symbols.sort_by_key(|symbol| symbol.range.start);
        symbols
    }
}

fn symbol_to_document_symbol(symbol: &Symbol, file: &FileInfo, source: &str) -> DocumentSymbol {
    let range = file.range_for_span(symbol.body_span);
    let selection_range = file.range_for_span(symbol.name_span);
    let name = match symbol.kind {
        SymbolKind::Method { .. } => symbol.fqn.to_string(),
        SymbolKind::GetMethod { .. } => format!("get {}", symbol.name),
        _ => symbol.name.to_string(),
    };
    let children = match &symbol.kind {
        SymbolKind::Struct { fields, .. } => fields
            .iter()
            .map(|field| symbol_to_document_symbol(field, file, source))
            .collect(),
        SymbolKind::Enum { members } => members
            .iter()
            .map(|member| symbol_to_document_symbol(member, file, source))
            .collect(),
        _ => Vec::new(),
    };
    let mut result = DocumentSymbol::new(name, symbol_kind(&symbol.kind), range, selection_range)
        .with_children(children);
    if let Some(detail) = symbol_detail(symbol, file, source) {
        result = result.with_detail(detail);
    }

    result
}

fn symbol_detail(symbol: &Symbol, file: &FileInfo, source: &str) -> Option<String> {
    match symbol.kind {
        SymbolKind::StructField => {
            let field = child_declaration::<StructField<'_>>(symbol, file)?;
            Some(format!(": {}", field.typ()?.text(source)))
        }
        SymbolKind::EnumMember => {
            let member = child_declaration::<EnumMember<'_>>(symbol, file)?;
            Some(
                member
                    .default()
                    .map(|value| format!(" = {}", value.text(source)))
                    .unwrap_or_default(),
            )
        }
        _ => match file.find_syntax_declaration(symbol.id)? {
            TopLevel::Func(function) => Some(function_detail(function, source)),
            TopLevel::Method(function) => Some(function_detail(function, source)),
            TopLevel::GetMethod(function) => Some(function_detail(function, source)),
            TopLevel::Constant(constant) => Some(format!(
                ": {} = {}",
                constant.typ().map_or("unknown", |typ| typ.text(source)),
                constant.value()?.text(source),
            )),
            TopLevel::GlobalVar(variable) => Some(format!(
                ": {}",
                variable.typ().map_or("unknown", |typ| typ.text(source)),
            )),
            _ => None,
        },
    }
}

fn function_detail<'tree, F>(function: F, source: &'tree str) -> String
where
    F: FunctionLike<'tree> + HasGenericParams<'tree>,
{
    let type_parameters = function
        .type_parameters()
        .map(|parameters| parameters.text(source))
        .unwrap_or_default();
    let parameters = function
        .parameters()
        .map(|parameter| parameter.text(source))
        .collect::<Vec<_>>()
        .join(", ");
    let return_type = function
        .return_type()
        .map(|typ| format!(": {}", typ.text(source)))
        .unwrap_or_default();

    format!("{type_parameters}({parameters}){return_type}")
}

fn child_declaration<'tree, N>(symbol: &Symbol, file: &'tree FileInfo) -> Option<N>
where
    N: TryFromNode<'tree>,
{
    let node = file.find_node_at_span(symbol.name_span)?;

    N::try_from_node(node.parent()?).ok()
}

pub(super) const fn symbol_kind(kind: &SymbolKind) -> DocumentSymbolKind {
    match kind {
        SymbolKind::GlobalVariable => DocumentSymbolKind::Variable,
        SymbolKind::Function { .. } => DocumentSymbolKind::Function,
        SymbolKind::Method { .. } => DocumentSymbolKind::Method,
        SymbolKind::GetMethod { .. } => DocumentSymbolKind::Event,
        SymbolKind::Struct { .. } => DocumentSymbolKind::Struct,
        SymbolKind::StructField => DocumentSymbolKind::Field,
        SymbolKind::Enum { .. } => DocumentSymbolKind::Enum,
        SymbolKind::EnumMember => DocumentSymbolKind::EnumMember,
        SymbolKind::Constant => DocumentSymbolKind::Constant,
        SymbolKind::TypeAlias { .. } => DocumentSymbolKind::TypeParameter,
    }
}
