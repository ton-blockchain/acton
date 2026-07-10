use super::TomlParsedDocument;
use super::schema::{acton_schema, hover_markdown, is_acton_manifest, schema_path};
use crate::{DocumentSnapshot, Hover, Position};

pub(super) fn hover(
    document: &DocumentSnapshot,
    parsed: &TomlParsedDocument,
    position: Position,
) -> Option<Hover> {
    if !is_acton_manifest(document.uri()) {
        return None;
    }

    let point = document
        .text_index()
        .position_to_point(document.text(), position);
    let node = parsed
        .source_file
        .root_node()
        .descendant_for_point_range(point, point)?;
    let path = schema_path(document, parsed, node)?;
    let doc = acton_schema()?.summary_for_path(&path)?;
    let markdown = hover_markdown(&path, &doc)?;

    Some(Hover::new(
        markdown,
        Some(document.text_index().range_of_node(document.text(), node)),
    ))
}
