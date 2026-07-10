use super::FiftParsedDocument;
use super::reference::FiftReference;
use crate::{DocumentSnapshot, Location, Position};

pub(super) fn definition(
    document: &DocumentSnapshot,
    parsed: &FiftParsedDocument,
    position: Position,
) -> Vec<Location> {
    let point = document
        .text_index()
        .position_to_point(document.text(), position);
    let Some(node) = parsed
        .source_file
        .root_node()
        .descendant_for_point_range(point, point)
    else {
        return Vec::new();
    };
    let Some(definition) =
        FiftReference::new(node, &parsed.source_file).and_then(|reference| reference.resolve())
    else {
        return Vec::new();
    };

    vec![Location::new(
        document.uri().clone(),
        document
            .text_index()
            .range_of_node(document.text(), definition),
    )]
}
