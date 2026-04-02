use crate::backend::Backend;
use crate::languages::engine::cache::ParsedSnapshot;
use crate::languages::tlb::psi::TlbReference;
use lsp_types::{GotoDefinitionParams, GotoDefinitionResponse, Location, Position, Range};

impl Backend {
    pub async fn handle_tlb_goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Option<GotoDefinitionResponse> {
        crate::profile!(self, "tlb: goto_definition");
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let file = self.registry.find_tlb_file(&uri)?;

        let ranges = find_definition_ranges(&file, position)?;
        if ranges.len() == 1 {
            return Some(GotoDefinitionResponse::Scalar(Location::new(
                uri, ranges[0],
            )));
        }

        Some(GotoDefinitionResponse::Array(
            ranges
                .into_iter()
                .map(|range| Location::new(uri.clone(), range))
                .collect(),
        ))
    }
}

fn find_definition_ranges(
    file: &ParsedSnapshot<tlb_syntax::SourceFile>,
    position: Position,
) -> Option<Vec<Range>> {
    let node = file.node_at(position)?;
    let reference = TlbReference::new(node, file.syntax())?;
    let definitions = reference.multi_resolve();
    if definitions.is_empty() {
        return None;
    }

    let mut ranges = Vec::with_capacity(definitions.len());
    for definition in definitions {
        let range = file.range_of(definition.node);
        if !ranges.contains(&range) {
            ranges.push(range);
        }
    }

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
    fn resolves_field_definition_from_field_expr_usage() {
        let source = "foo$0 a:# b:a = Bar;\n";
        let snapshot = parse_snapshot(source);

        let usage_offset = source.find(" b:a").expect("usage should exist") + 3;
        let usage_position = snapshot.position(usage_offset);

        let ranges = find_definition_ranges(&snapshot, usage_position)
            .expect("definition range should resolve");
        assert_eq!(ranges.len(), 1);
        let range = ranges[0];

        let definition_offset = source.find(" a:#").expect("definition should exist") + 1;
        let definition_start = snapshot.position(definition_offset);
        assert_eq!(range.start, definition_start);
    }

    #[test]
    fn resolves_combinator_reference_to_declaration_combinator_name() {
        let source = "foo$0 a:# = CommonMsgInfo;\nbar$1 b:CommonMsgInfo = Wrap;\n";
        let snapshot = parse_snapshot(source);

        let usage_offset = source.find("b:CommonMsgInfo").expect("usage should exist") + 2;
        let usage_position = snapshot.position(usage_offset);

        let ranges = find_definition_ranges(&snapshot, usage_position)
            .expect("definition range should resolve");
        assert_eq!(ranges.len(), 1);
        let range = ranges[0];

        let definition_offset = source
            .find("= CommonMsgInfo")
            .expect("definition should exist")
            + 2;
        let definition_start = snapshot.position(definition_offset);
        assert_eq!(range.start, definition_start);
    }

    #[test]
    fn resolves_multiple_declarations_for_same_type() {
        let source = "foo$0 a:# = CommonMsgInfo;\nbar$1 b:# = CommonMsgInfo;\nbaz$2 x:CommonMsgInfo = Wrap;\n";
        let snapshot = parse_snapshot(source);

        let usage_offset = source.find("x:CommonMsgInfo").expect("usage should exist") + 2;
        let usage_position = snapshot.position(usage_offset);

        let ranges = find_definition_ranges(&snapshot, usage_position)
            .expect("definition ranges should resolve");
        assert_eq!(ranges.len(), 2);

        let first_definition_offset = source
            .find("= CommonMsgInfo")
            .expect("first definition should exist")
            + 2;
        let second_definition_offset = source
            .rfind("= CommonMsgInfo")
            .expect("second definition should exist")
            + 2;
        let first_definition_start = snapshot.position(first_definition_offset);
        let second_definition_start = snapshot.position(second_definition_offset);

        assert_eq!(ranges[0].start, first_definition_start);
        assert_eq!(ranges[1].start, second_definition_start);
    }
}
