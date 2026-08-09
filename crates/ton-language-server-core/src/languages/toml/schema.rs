use super::TomlParsedDocument;
use crate::{DocumentSnapshot, DocumentUri};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;
use toml_syntax::{AstNode, Key, Pair, TopLevel, Value as TomlValue};
use ton_json_schema::{SchemaDoc, SchemaPathSegment, SchemaStore};
use tree_sitter::Node;

static ACTON_SCHEMA: OnceLock<Option<SchemaStore>> = OnceLock::new();
const ACTON_SCHEMA_JSON: &str = include_str!("../../../../acton-config/schemas/acton.schema.json");

pub(super) fn is_acton_manifest(uri: &DocumentUri) -> bool {
    uri.logical_path()
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("Acton.toml"))
}

pub(super) fn acton_schema() -> Option<&'static SchemaStore> {
    ACTON_SCHEMA
        .get_or_init(|| SchemaStore::from_json_str(ACTON_SCHEMA_JSON).ok())
        .as_ref()
}

pub(super) fn schema_path(
    document: &DocumentSnapshot,
    parsed: &TomlParsedDocument,
    target: Node<'_>,
) -> Option<Vec<SchemaPathSegment>> {
    let mut current_table = Vec::new();
    let mut table_array_indices = HashMap::new();

    for top_level in parsed.source_file.top_levels() {
        let contains_target = contains_node(top_level.syntax(), target);

        match top_level {
            TopLevel::Table(table) => {
                let table_path = key_path(document, table.key()?);
                if contains_target {
                    for pair in table.pairs() {
                        if !contains_node(pair.syntax(), target) {
                            continue;
                        }
                        let mut path = table_path;
                        path.extend(pair_path(document, pair, target)?);
                        return Some(path);
                    }
                    return Some(table_path);
                }
                current_table = table_path;
            }
            TopLevel::TableArrayElement(table) => {
                let table_path =
                    indexed_table_array_path(document, table.key()?, &mut table_array_indices);
                if contains_target {
                    for pair in table.pairs() {
                        if !contains_node(pair.syntax(), target) {
                            continue;
                        }
                        let mut path = table_path;
                        path.extend(pair_path(document, pair, target)?);
                        return Some(path);
                    }
                    return Some(table_path);
                }
                current_table = table_path;
            }
            TopLevel::Pair(pair) if contains_target => {
                let mut path = current_table;
                path.extend(pair_path(document, pair, target)?);
                return Some(path);
            }
            TopLevel::Pair(_) | TopLevel::Unmapped(_) => {}
        }
    }

    None
}

pub(super) fn table_path_at_offset(
    document: &DocumentSnapshot,
    parsed: &TomlParsedDocument,
    offset: usize,
) -> Vec<SchemaPathSegment> {
    let mut current_table = Vec::new();
    let mut table_array_indices = HashMap::new();

    for top_level in parsed.source_file.top_levels() {
        if top_level.syntax().start_byte() > offset {
            break;
        }

        match top_level {
            TopLevel::Table(table) => {
                if let Some(key) = table.key() {
                    current_table = key_path(document, key);
                }
            }
            TopLevel::TableArrayElement(table) => {
                if let Some(key) = table.key() {
                    current_table =
                        indexed_table_array_path(document, key, &mut table_array_indices);
                }
            }
            TopLevel::Pair(_) | TopLevel::Unmapped(_) => {}
        }
    }

    current_table
}

pub(super) fn parse_key_path(text: &str) -> Option<Vec<SchemaPathSegment>> {
    let mut table = toml::from_str::<toml::Table>(&format!("{text} = false")).ok()?;
    let mut result = Vec::new();

    loop {
        if table.len() != 1 {
            return None;
        }
        let (key, value) = table.into_iter().next()?;
        result.push(SchemaPathSegment::Key(key));

        let toml::Value::Table(nested) = value else {
            return Some(result);
        };
        table = nested;
    }
}

fn pair_path(
    document: &DocumentSnapshot,
    pair: Pair<'_>,
    target: Node<'_>,
) -> Option<Vec<SchemaPathSegment>> {
    let mut path = key_path(document, pair.key()?);
    if let Some(value) = pair.value()
        && contains_node(value.syntax(), target)
        && let Some(nested) = value_path(document, value, target)
    {
        path.extend(nested);
    }
    Some(path)
}

fn value_path(
    document: &DocumentSnapshot,
    value: TomlValue<'_>,
    target: Node<'_>,
) -> Option<Vec<SchemaPathSegment>> {
    if !contains_node(value.syntax(), target) {
        return None;
    }

    match value {
        TomlValue::Array(array) => {
            for (index, item) in array.values().enumerate() {
                if !contains_node(item.syntax(), target) {
                    continue;
                }

                let mut path = vec![SchemaPathSegment::Index(index)];
                if let Some(nested) = value_path(document, item, target) {
                    path.extend(nested);
                }
                return Some(path);
            }
            Some(Vec::new())
        }
        TomlValue::InlineTable(table) => table
            .pairs()
            .find(|pair| contains_node(pair.syntax(), target))
            .and_then(|pair| pair_path(document, pair, target))
            .or_else(|| Some(Vec::new())),
        TomlValue::Unmapped(raw) => (!document.text_of(raw).is_empty()).then(Vec::new),
        TomlValue::String(_)
        | TomlValue::Integer(_)
        | TomlValue::Float(_)
        | TomlValue::Boolean(_)
        | TomlValue::OffsetDateTime(_)
        | TomlValue::LocalDateTime(_)
        | TomlValue::LocalDate(_)
        | TomlValue::LocalTime(_) => Some(Vec::new()),
    }
}

fn key_path(document: &DocumentSnapshot, key: Key<'_>) -> Vec<SchemaPathSegment> {
    parse_key_path(document.text_of(key)).unwrap_or_default()
}

fn indexed_table_array_path(
    document: &DocumentSnapshot,
    key: Key<'_>,
    indices: &mut HashMap<Vec<SchemaPathSegment>, usize>,
) -> Vec<SchemaPathSegment> {
    let mut path = key_path(document, key);
    let index = indices.entry(path.clone()).or_default();

    path.push(SchemaPathSegment::Index(*index));
    *index += 1;

    path
}

pub(super) fn hover_markdown(path: &[SchemaPathSegment], doc: &SchemaDoc) -> Option<String> {
    if doc.is_empty() {
        return None;
    }

    let mut lines = vec![
        "```toml".to_owned(),
        format_path(path),
        "```".to_owned(),
        String::new(),
    ];
    if let Some(title) = &doc.title {
        lines.push(format!("**{title}**"));
        lines.push(String::new());
    }
    if let Some(description) = &doc.description {
        lines.push(description.clone());
        lines.push(String::new());
    }
    if let Some(schema_type) = &doc.schema_type {
        lines.push(format!("- Type: `{schema_type}`"));
    }
    if let Some(value) = &doc.default_value {
        lines.push(format!("- Default: `{}`", format_json(value)));
    }
    if let Some(value) = &doc.const_value {
        lines.push(format!("- Const: `{}`", format_json(value)));
    }
    if !doc.enum_values.is_empty() {
        let values = doc
            .enum_values
            .iter()
            .map(format_json)
            .collect::<Vec<_>>()
            .join(" | ");
        lines.push(format!("- Enum: `{values}`"));
    }
    if !doc.examples.is_empty() {
        let values = doc
            .examples
            .iter()
            .map(format_json)
            .collect::<Vec<_>>()
            .join(" | ");
        lines.push(format!("- Examples: `{values}`"));
    }

    Some(lines.join("\n"))
}

pub(super) fn format_path(path: &[SchemaPathSegment]) -> String {
    let mut result = String::new();
    for segment in path {
        match segment {
            SchemaPathSegment::Key(key) => {
                if !result.is_empty() {
                    result.push('.');
                }
                if !key.is_empty()
                    && key.chars().all(|character| {
                        character.is_ascii_alphanumeric() || "_-".contains(character)
                    })
                {
                    result.push_str(key);
                } else {
                    result.push_str(&format_json(&Value::String(key.clone())));
                }
            }
            SchemaPathSegment::Index(index) => {
                result.push('[');
                result.push_str(&index.to_string());
                result.push(']');
            }
        }
    }
    result
}

pub(super) fn contains_node(container: Node<'_>, node: Node<'_>) -> bool {
    node.start_byte() >= container.start_byte() && node.end_byte() <= container.end_byte()
}

pub(super) fn ancestor<'tree>(mut node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    loop {
        if node.kind() == kind {
            return Some(node);
        }
        node = node.parent()?;
    }
}

pub(super) fn format_json(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "null".to_owned())
}
