use super::TlbParsedDocument;
use crate::{DocumentSnapshot, DocumentSymbol, DocumentSymbolKind};
use tlb_syntax::{AstNode, TopLevel};

pub(super) fn document_symbols(
    document: &DocumentSnapshot,
    parsed: &TlbParsedDocument,
) -> Vec<DocumentSymbol> {
    let source = document.text();
    let index = document.text_index();
    let mut symbols = parsed
        .source_file
        .top_levels()
        .filter_map(|top_level| {
            let TopLevel::Declaration(declaration) = top_level else {
                return None;
            };
            let name = declaration.combinator()?.name()?;
            let range = index.range_of_node(source, declaration.syntax());
            let selection_range = index.range_of_node(source, name.syntax());

            Some(
                DocumentSymbol::new(
                    name.text(source),
                    DocumentSymbolKind::Class,
                    range,
                    selection_range,
                )
                .with_detail(declaration.text(source)),
            )
        })
        .collect::<Vec<_>>();
    symbols.sort_by_key(|symbol| symbol.range.start);
    symbols
}
