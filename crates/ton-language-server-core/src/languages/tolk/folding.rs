use crate::{DocumentSnapshot, FoldingRange, Position};
use tree_sitter::Node;

pub(super) fn folding_ranges(document: &DocumentSnapshot, root: Node<'_>) -> Vec<FoldingRange> {
    let mut ranges = Vec::new();
    collect_ranges(document, root, &mut ranges);
    ranges.sort_by_key(|range| {
        (
            range.start_line,
            range.start_character,
            range.end_line,
            range.end_character,
        )
    });
    ranges.dedup();
    ranges
}

fn collect_ranges(document: &DocumentSnapshot, node: Node<'_>, ranges: &mut Vec<FoldingRange>) {
    if matches!(
        node.kind(),
        "block_statement" | "object_literal_body" | "match_body" | "struct_body" | "enum_body"
    ) {
        let _ = push_body_range(document, node, ranges);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_ranges(document, child, ranges);
    }
}

fn push_body_range(
    document: &DocumentSnapshot,
    node: Node<'_>,
    ranges: &mut Vec<FoldingRange>,
) -> Option<()> {
    let open = node.child(0)?;
    let last_child = u32::try_from(node.child_count().checked_sub(1)?).ok()?;
    let close = node.child(last_child)?;
    if open.kind() != "{"
        || close.kind() != "}"
        || open.end_position().row >= close.start_position().row
    {
        return None;
    }

    let start = position(document, open.end_byte());
    let end = position(document, close.start_byte());
    ranges.push(FoldingRange::new(
        start.line,
        Some(start.character),
        end.line,
        Some(end.character),
    ));
    Some(())
}

fn position(document: &DocumentSnapshot, offset: usize) -> Position {
    document
        .text_index()
        .offset_to_position(document.text(), offset)
}
