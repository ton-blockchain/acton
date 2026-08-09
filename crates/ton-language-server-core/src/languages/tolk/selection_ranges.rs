use crate::{DocumentSnapshot, Position, Range, SelectionRange};
use tree_sitter::Node;

pub(super) fn selection_ranges(
    document: &DocumentSnapshot,
    root: Node<'_>,
    positions: &[Position],
) -> Vec<SelectionRange> {
    positions
        .iter()
        .copied()
        .map(|position| selection_range(document, root, position))
        .collect()
}

fn selection_range(
    document: &DocumentSnapshot,
    root: Node<'_>,
    position: Position,
) -> SelectionRange {
    let offset = document
        .text_index()
        .position_to_offset(document.text(), position);
    let mut node = root.named_descendant_for_byte_range(offset, offset);
    let mut ranges = Vec::new();

    while let Some(current) = node {
        if current.is_named() && current.start_byte() < current.end_byte() {
            let range = document.text_index().range_for_offsets(
                document.text(),
                current.start_byte(),
                current.end_byte(),
            );
            if ranges.last() != Some(&range) {
                ranges.push(range);
            }
        }
        node = current.parent();
    }

    ranges
        .into_iter()
        .rev()
        .fold(None, |parent, range| {
            Some(Box::new(SelectionRange::new(range, parent)))
        })
        .map_or_else(
            || SelectionRange::new(Range::new(position, position), None),
            |range| *range,
        )
}
