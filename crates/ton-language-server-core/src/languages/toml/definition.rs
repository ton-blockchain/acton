use super::TomlParsedDocument;
use super::paths::resolve_path;
use super::schema::schema_path;
use crate::{DocumentSnapshot, Location, Position, Range};
use toml_syntax::{AstNode, StringLit};

pub(super) fn definition(
    document: &DocumentSnapshot,
    parsed: &TomlParsedDocument,
    position: Position,
) -> Vec<Location> {
    let point = document
        .text_index()
        .position_to_point(document.text(), position);
    let Some(node) = parsed
        .source_file
        .tree
        .root_node()
        .descendant_for_point_range(point, point)
    else {
        return Vec::new();
    };
    let Some(string) = node.ancestor_as::<StringLit<'_>>() else {
        return Vec::new();
    };
    let Some(path) = schema_path(document, parsed, string.syntax()) else {
        return Vec::new();
    };
    let Some(uri) = resolve_path(document.uri(), &path, document.text_of(string)) else {
        return Vec::new();
    };
    let content = string.content_range(document.text());
    let origin_selection_range =
        document
            .text_index()
            .range_for_offsets(document.text(), content.start, content.end);

    vec![
        Location::new(uri, Range::new(Position::new(0, 0), Position::new(0, 0)))
            .with_origin_selection_range(origin_selection_range),
    ]
}
