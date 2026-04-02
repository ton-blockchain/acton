use crate::backend::Backend;
use crate::languages::engine::cache::ParsedSnapshot;
use crate::languages::fift::psi::FiftReferent;
use lsp_types::{Location, Position, Range, ReferenceParams};

impl Backend {
    pub async fn handle_fift_references(&self, params: ReferenceParams) -> Option<Vec<Location>> {
        crate::profile!(self, "fift: references");
        let uri = params.text_document_position.text_document.uri;

        let file = self.registry.find_fift_file(&uri)?;

        let ranges = find_reference_ranges(
            &file,
            params.text_document_position.position,
            params.context.include_declaration,
        )?;

        let locations = ranges
            .into_iter()
            .map(|range| Location::new(uri.clone(), range))
            .collect::<Vec<_>>();
        Some(locations)
    }
}

fn find_reference_ranges(
    file: &ParsedSnapshot<fift_syntax::SourceFile>,
    position: Position,
    include_definition: bool,
) -> Option<Vec<Range>> {
    let node = file.node_at(position)?;
    let referent = FiftReferent::new(node, file.syntax());
    referent.resolved()?;

    let ranges = referent
        .find_references(include_definition)
        .into_iter()
        .map(|node| {
            let target = node.child_by_field_name("name").unwrap_or(node);
            file.range_of(target)
        })
        .collect::<Vec<_>>();

    Some(ranges)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn parse_snapshot(source: &str) -> ParsedSnapshot<fift_syntax::SourceFile> {
        ParsedSnapshot::new(
            lsp_types::Url::parse("file:///tmp/test.fift").expect("snapshot uri should parse"),
            1,
            Arc::from(source),
            Arc::new(fift_syntax::parse(source).expect("failed to parse fixture")),
        )
    }

    #[test]
    fn finds_reference_ranges() {
        let source = r#"PROGRAM{
DECLPROC entry
entry PROC:<{
  foo
  foo
}>
foo PROC:<{
  foo
}>
END>c
"#;

        let snapshot = parse_snapshot(source);
        let offset = source.find("  foo").expect("reference must exist") + 2;
        let position = offset_to_position(source, offset);

        let without_def =
            find_reference_ranges(&snapshot, position, false).expect("ranges must resolve");
        assert_eq!(without_def.len(), 3);

        let with_def =
            find_reference_ranges(&snapshot, position, true).expect("ranges must resolve");
        assert_eq!(with_def.len(), 4);
    }

    #[test]
    fn returns_none_for_unresolved_symbol() {
        let source = r#"PROGRAM{
DECLPROC entry
entry PROC:<{
  missing
}>
foo PROC:<{ }>
END>c
"#;

        let snapshot = parse_snapshot(source);
        let offset = source.find("missing").expect("reference must exist");
        let position = offset_to_position(source, offset);
        assert!(find_reference_ranges(&snapshot, position, false).is_none());
    }

    fn offset_to_position(source: &str, byte_offset: usize) -> Position {
        let mut line = 0usize;
        let mut character = 0usize;

        for (index, byte) in source.bytes().enumerate() {
            if index >= byte_offset {
                break;
            }
            if byte == b'\n' {
                line += 1;
                character = 0;
            } else {
                character += 1;
            }
        }

        Position::new(line as u32, character as u32)
    }
}
