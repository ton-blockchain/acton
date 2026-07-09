use crate::logging;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TlbSymbol {
    pub(super) name: String,
    pub(super) start_byte: usize,
    pub(super) end_byte: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct TlbSymbolIndex {
    declarations: Vec<TlbSymbol>,
}

impl TlbSymbolIndex {
    pub(super) fn build(source_file: &tlb_syntax::SourceFile) -> Self {
        let source = source_file.source.as_ref();
        let mut declarations = Vec::new();
        for top_level in source_file.top_levels() {
            let tlb_syntax::TopLevel::Declaration(declaration) = top_level else {
                continue;
            };
            let Some(name_node) = declaration
                .combinator()
                .and_then(|combinator| combinator.name())
            else {
                continue;
            };
            let Ok(name) = name_node.0.utf8_text(source.as_bytes()) else {
                continue;
            };
            declarations.push(TlbSymbol {
                name: name.trim().to_owned(),
                start_byte: name_node.0.start_byte(),
                end_byte: name_node.0.end_byte(),
            });
            tracing::trace!(
                target: logging::TLB_TARGET,
                operation = "tlb.index.symbol",
                name = name.trim(),
                start_byte = name_node.0.start_byte(),
                end_byte = name_node.0.end_byte(),
                "indexed TL-B symbol"
            );
        }
        Self { declarations }
    }

    pub(super) fn declaration_count(&self) -> usize {
        self.declarations.len()
    }

    pub(super) fn declarations_named<'a>(
        &'a self,
        name: &'a str,
    ) -> impl Iterator<Item = &'a TlbSymbol> {
        self.declarations
            .iter()
            .filter(move |symbol| symbol.name == name)
    }
}
