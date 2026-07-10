use super::FiftParsedDocument;
use crate::languages::instruction_docs::InstructionSpec;
use crate::{DocumentSnapshot, InlayHint, InlayHintKind, Position, Range};
use ton_syntax::ast::PreorderTraverse;

pub(super) fn inlay_hints(
    spec: &InstructionSpec,
    document: &DocumentSnapshot,
    parsed: &FiftParsedDocument,
    requested_range: Range,
) -> Vec<InlayHint> {
    let mut hints = Vec::new();

    for node in PreorderTraverse::new(parsed.source_file.root_node().walk()) {
        if node.kind() != "identifier" {
            continue;
        }
        let position = document
            .text_index()
            .range_of_node(document.text(), node)
            .end;
        if !contains_position(requested_range, position) {
            continue;
        }

        let Some(instruction) = spec.instruction(document.text_of(node).trim()) else {
            continue;
        };
        hints.push(InlayHint::new(
            position,
            instruction.gas(),
            InlayHintKind::Type,
        ));
    }

    hints
}

fn contains_position(range: Range, position: Position) -> bool {
    range.start <= position && position <= range.end
}
