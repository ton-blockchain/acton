use super::{TolkWorkspaceEngine, logical_path_for_uri};
use crate::{DocumentSnapshot, DocumentSymbol, DocumentSymbolKind, Range, TextIndex};
use tolk_resolver::{FileInfo, Span, Symbol, SymbolKind};
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
        let path = logical_path_for_uri(document.uri());
        let Some(file_id) = snapshot.project_index.get_file_by_path(&path) else {
            return Vec::new();
        };
        let Some(file) = snapshot.file_db.get_by_id(file_id) else {
            return Vec::new();
        };
        let source = file.source().source.as_ref();
        let index = TextIndex::new(source);
        let mut symbols = Vec::new();

        if let Some(imports) = snapshot.project_index.imports_of(file_id) {
            symbols.extend(imports.into_iter().map(|import| {
                let range = range_for_span(&index, source, import.import().span);
                let name = source
                    .get(import.import().span.start()..import.import().span.end())
                    .unwrap_or("import")
                    .trim_end_matches(';');
                DocumentSymbol::new(name, DocumentSymbolKind::Module, range, range)
            }));
        }

        if let Some(file_index) = snapshot.project_index.get_file_index(file_id) {
            symbols.extend(
                file_index
                    .decls
                    .iter()
                    .map(|symbol| symbol_to_document_symbol(symbol, file.as_ref(), &index, source)),
            );
        }
        symbols.sort_by_key(|symbol| symbol.range.start);
        symbols
    }
}

fn symbol_to_document_symbol(
    symbol: &Symbol,
    file: &FileInfo,
    index: &TextIndex,
    source: &str,
) -> DocumentSymbol {
    let range = range_for_span(index, source, symbol.body_span);
    let selection_range = range_for_span(index, source, symbol.name_span);
    let name = match symbol.kind {
        SymbolKind::Method { .. } => symbol.fqn.to_string(),
        SymbolKind::GetMethod { .. } => format!("get {}", symbol.name),
        _ => symbol.name.to_string(),
    };
    let children = match &symbol.kind {
        SymbolKind::Struct { fields, .. } => fields
            .iter()
            .map(|field| symbol_to_document_symbol(field, file, index, source))
            .collect(),
        SymbolKind::Enum { members } => members
            .iter()
            .map(|member| symbol_to_document_symbol(member, file, index, source))
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
    let node = file
        .source()
        .tree
        .root_node()
        .descendant_for_byte_range(symbol.name_span.start(), symbol.name_span.end())?;

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

fn range_for_span(index: &TextIndex, source: &str, span: Span) -> Range {
    index.range_for_offsets(source, span.start(), span.end())
}
