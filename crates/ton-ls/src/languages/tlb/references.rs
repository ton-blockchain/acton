use crate::backend::Backend;
use crate::languages::engine::cache::ParsedSnapshot;
use crate::languages::tlb::psi::TlbReferent;
use lsp_types::{Location, Position, Range, ReferenceParams};

impl Backend {
    pub async fn handle_tlb_references(&self, params: ReferenceParams) -> Option<Vec<Location>> {
        crate::profile!(self, "tlb: references");
        let uri = params.text_document_position.text_document.uri;
        let file = self.registry.find_tlb_file(&uri)?;

        let ranges = find_reference_ranges(
            &file,
            params.text_document_position.position,
            params.context.include_declaration,
        )?;

        Some(
            ranges
                .into_iter()
                .map(|range| Location::new(uri.clone(), range))
                .collect(),
        )
    }
}

fn find_reference_ranges(
    file: &ParsedSnapshot<tlb_syntax::SourceFile>,
    position: Position,
    include_definition: bool,
) -> Option<Vec<Range>> {
    let node = file.node_at(position)?;
    let referent = TlbReferent::new(node, file.syntax());
    referent.resolved()?;

    let ranges = referent
        .find_references(include_definition)
        .into_iter()
        .map(|node| file.range_of(node))
        .collect::<Vec<_>>();

    Some(ranges)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_snapshot(source: &str) -> ParsedSnapshot<tlb_syntax::SourceFile> {
        ParsedSnapshot::new(
            lsp_types::Url::parse("file:///tmp/test.tlb").expect("snapshot uri should parse"),
            1,
            std::sync::Arc::from(source),
            std::sync::Arc::new(tlb_syntax::parse(source).expect("failed to parse fixture")),
        )
    }

    #[test]
    fn finds_references_for_field_symbol() {
        let source = "foo$0 a:# b:a c:a = Bar;\n";
        let snapshot = parse_snapshot(source);

        let usage_offset = source.find(" b:a").expect("usage should exist") + 3;
        let usage_position = snapshot.position(usage_offset);

        let without_def =
            find_reference_ranges(&snapshot, usage_position, false).expect("ranges should resolve");
        assert_eq!(without_def.len(), 2);

        let with_def =
            find_reference_ranges(&snapshot, usage_position, true).expect("ranges should resolve");
        assert_eq!(with_def.len(), 3);
    }

    #[test]
    fn returns_none_for_unresolved_symbol() {
        let source = "foo$0 a:Type = Bar missing;\n";
        let snapshot = parse_snapshot(source);

        let offset = source.find("missing").expect("reference should exist");
        let position = snapshot.position(offset);

        assert!(find_reference_ranges(&snapshot, position, false).is_none());
    }
}
