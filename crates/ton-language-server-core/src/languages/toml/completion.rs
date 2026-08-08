mod context;
mod items;

use self::context::CompletionContext;
use self::items::{property_item, value_items};
use super::TomlParsedDocument;
use super::schema::{acton_schema, is_acton_manifest};
use crate::{CompletionList, DocumentSnapshot, Position};

pub(super) fn completion(
    document: &DocumentSnapshot,
    parsed: &TomlParsedDocument,
    position: Position,
) -> CompletionList {
    if !is_acton_manifest(document.uri()) {
        return CompletionList::default();
    }
    let Some(schema) = acton_schema() else {
        return CompletionList::default();
    };

    let offset = document
        .text_index()
        .position_to_offset(document.text(), position);
    let point = document
        .text_index()
        .position_to_point(document.text(), position);
    let node = parsed
        .source_file
        .root_node()
        .descendant_for_point_range(point, point);
    let Some(context) = CompletionContext::new(document, parsed, node, offset, position) else {
        return CompletionList::default();
    };

    let items = match context {
        CompletionContext::Keys {
            object_path,
            existing_keys,
            header_kind,
            key_only,
            replacement_range,
        } => schema
            .completion_for_path(&object_path)
            .map(|completion| {
                let is_top_level = object_path.is_empty();
                completion
                    .properties
                    .into_iter()
                    .filter(|property| !existing_keys.contains(&property.name))
                    .filter(|property| header_kind.allows(schema, &object_path, property))
                    .map(|property| {
                        property_item(
                            property,
                            is_top_level,
                            header_kind.is_header(),
                            key_only,
                            replacement_range,
                        )
                    })
                    .collect()
            })
            .unwrap_or_default(),
        CompletionContext::Values {
            value_path,
            in_string,
            replacement_range,
        } => schema
            .summary_for_path(&value_path)
            .map(|doc| value_items(doc, in_string, replacement_range))
            .unwrap_or_default(),
    };

    CompletionList::new(items)
}
