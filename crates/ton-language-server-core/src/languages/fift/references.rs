use super::FiftParsedDocument;
use super::reference::{FiftReference, is_definition_name};
use crate::{DocumentSnapshot, Location, Position};
use ton_syntax::ast::PreorderTraverse;

pub(super) fn references(
    document: &DocumentSnapshot,
    parsed: &FiftParsedDocument,
    position: Position,
    include_declaration: bool,
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
    let Some(reference) = FiftReference::new(node, &parsed.source_file) else {
        return Vec::new();
    };
    let Some(definition) = reference.resolve() else {
        return Vec::new();
    };
    let name = document.text_of(definition).trim();
    let mut locations = Vec::new();

    if include_declaration {
        locations.push(location(document, definition));
    }

    for candidate in PreorderTraverse::new(parsed.source_file.root_node().walk()) {
        if candidate.kind() != "identifier" || document.text_of(candidate).trim() != name {
            continue;
        }
        let Some(parent) = candidate.parent() else {
            continue;
        };
        if is_definition_name(parent, candidate) {
            continue;
        }

        let resolved = FiftReference::new(candidate, &parsed.source_file)
            .and_then(|reference| reference.resolve());
        if resolved == Some(definition) {
            locations.push(location(document, candidate));
        }
    }

    locations
}

fn location(document: &DocumentSnapshot, node: tree_sitter::Node<'_>) -> Location {
    Location::new(
        document.uri().clone(),
        document.text_index().range_of_node(document.text(), node),
    )
}
