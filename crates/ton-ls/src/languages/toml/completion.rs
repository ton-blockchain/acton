use crate::backend::Backend;
use crate::languages::engine::cache::ParsedSnapshot;
use crate::languages::toml::hover::{find_schema_path, get_acton_schema_store, is_acton_toml};
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionParams, CompletionResponse, Documentation,
    InsertTextFormat, MarkupContent, MarkupKind,
};
use serde_json::Value;
use std::collections::HashSet;
use toml_syntax::TopLevel;
use ton_json_schema::{CompletionProperty, SchemaDoc, SchemaPathSegment};
use tree_sitter::Node;

impl Backend {
    pub async fn handle_toml_completion(
        &self,
        params: CompletionParams,
    ) -> Option<CompletionResponse> {
        crate::profile!(self, "toml: completion");
        let uri = params.text_document_position.text_document.uri;

        let path = uri.to_file_path().ok()?;
        if !is_acton_toml(&path) {
            return None;
        }

        let file = self.registry.find_toml_file(&uri)?;

        let position = params.text_document_position.position;
        let cursor_offset = file.position_to_offset(position);
        let node = file.node_at(position);

        let schema = get_acton_schema_store()?;

        let context = build_completion_context(&file, node, cursor_offset);
        let items = match context {
            TomlCompletionContext::Keys {
                object_path,
                existing_keys,
                in_table_header,
            } => schema
                .completion_for_path(&object_path)
                .map(|completion| {
                    let is_top_level = object_path.is_empty();
                    completion
                        .properties
                        .into_iter()
                        .filter(|property| !existing_keys.contains(&property.name))
                        .map(|property| {
                            property_completion_item(property, is_top_level, in_table_header)
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            TomlCompletionContext::Values {
                value_path,
                in_string_literal,
            } => schema
                .summary_for_path(&value_path)
                .map(|doc| value_completion_items(doc, in_string_literal))
                .unwrap_or_default(),
        };

        if items.is_empty() {
            return None;
        }

        Some(CompletionResponse::Array(items))
    }
}

enum TomlCompletionContext {
    Keys {
        object_path: Vec<SchemaPathSegment>,
        existing_keys: HashSet<String>,
        in_table_header: bool,
    },
    Values {
        value_path: Vec<SchemaPathSegment>,
        in_string_literal: bool,
    },
}

fn build_completion_context(
    file: &ParsedSnapshot<toml_syntax::SourceFile>,
    node: Option<Node<'_>>,
    cursor_offset: usize,
) -> TomlCompletionContext {
    let source = file.source();
    let Some(node) = node else {
        let in_table_header = is_in_table_header_brackets(source, cursor_offset);
        return TomlCompletionContext::Keys {
            object_path: Vec::new(),
            existing_keys: collect_existing_property_keys(file, &[], None),
            in_table_header,
        };
    };

    let full_path = find_schema_path(file, node).unwrap_or_default();

    if is_in_pair_value(node) {
        return TomlCompletionContext::Values {
            value_path: full_path,
            in_string_literal: enclosing_node_of_kind(node, "string").is_some(),
        };
    }

    if is_in_table_header_context(node, source)
        || is_in_table_header_brackets(source, cursor_offset)
    {
        let object_path = truncate_last_path_segment(full_path);
        let existing_keys = collect_existing_property_keys(file, &object_path, Some(node));
        return TomlCompletionContext::Keys {
            object_path,
            existing_keys,
            in_table_header: true,
        };
    }

    let object_path = if enclosing_node_of_kind(node, "pair").is_some() {
        truncate_last_path_segment(full_path)
    } else {
        full_path
    };

    let existing_keys = collect_existing_property_keys(file, &object_path, Some(node));
    TomlCompletionContext::Keys {
        object_path,
        existing_keys,
        in_table_header: false,
    }
}

fn truncate_last_path_segment(mut path: Vec<SchemaPathSegment>) -> Vec<SchemaPathSegment> {
    path.pop();
    path
}

fn enclosing_node_of_kind<'tree>(mut node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    loop {
        if node.kind() == kind {
            return Some(node);
        }
        node = node.parent()?;
    }
}

fn is_in_pair_value(node: Node<'_>) -> bool {
    let Some(pair) = enclosing_node_of_kind(node, "pair") else {
        return false;
    };

    let mut cursor = pair.walk();
    let mut named = pair.named_children(&mut cursor);
    let _ = named.next(); // key
    let value = named.next();

    let Some(value) = value else {
        return false;
    };

    contains_node(value, node)
}

fn is_in_table_header_context(node: Node<'_>, source: &str) -> bool {
    let Some(table) = enclosing_node_of_kind(node, "table")
        .or_else(|| enclosing_node_of_kind(node, "table_array_element"))
    else {
        return false;
    };

    let table_start = table.start_byte();
    let table_end = table.end_byte().min(source.len());
    if table_start >= table_end {
        return false;
    }

    let header_end = source[table_start..table_end]
        .find('\n')
        .map_or(table_end, |newline| table_start + newline);

    node.start_byte() <= header_end
}

fn is_in_table_header_brackets(source: &str, cursor_offset: usize) -> bool {
    if source.is_empty() {
        return false;
    }

    let clamped = cursor_offset.min(source.len());
    let line_start = source[..clamped].rfind('\n').map_or(0, |index| index + 1);
    let line_end = source[clamped..]
        .find('\n')
        .map_or(source.len(), |index| clamped + index);
    let line = &source[line_start..line_end];

    let Some(first_non_ws) = line.find(|ch: char| !ch.is_whitespace()) else {
        return false;
    };
    if line.as_bytes().get(first_non_ws) != Some(&b'[') {
        return false;
    }

    let after_open = first_non_ws + 1;
    let Some(close_rel) = line[after_open..].find(']') else {
        return false;
    };
    let close = after_open + close_rel;

    let cursor_col = clamped.saturating_sub(line_start);
    cursor_col >= after_open && cursor_col <= close
}

fn contains_node(container: Node<'_>, node: Node<'_>) -> bool {
    node.start_byte() >= container.start_byte() && node.end_byte() <= container.end_byte()
}

fn collect_existing_property_keys(
    file: &ParsedSnapshot<toml_syntax::SourceFile>,
    object_path: &[SchemaPathSegment],
    cursor_node: Option<Node<'_>>,
) -> HashSet<String> {
    let mut result = HashSet::new();

    let mut push_pair = |pair_node: Node<'_>| {
        if let Some(cursor_node) = cursor_node
            && contains_node(pair_node, cursor_node)
        {
            return;
        }

        let Some(path) = find_schema_path(file, pair_node) else {
            return;
        };

        let Some(key) = immediate_child_key(&path, object_path) else {
            return;
        };
        result.insert(key);
    };

    for top_level in file.syntax().top_levels() {
        match top_level {
            TopLevel::Pair(pair) => push_pair(pair.0),
            TopLevel::Table(table) => {
                for pair in table.pairs() {
                    push_pair(pair.0);
                }
            }
            TopLevel::TableArrayElement(table_array) => {
                for pair in table_array.pairs() {
                    push_pair(pair.0);
                }
            }
            TopLevel::Unmapped(_) => {}
        }
    }

    result
}

fn immediate_child_key(
    path: &[SchemaPathSegment],
    object_path: &[SchemaPathSegment],
) -> Option<String> {
    if !path_starts_with(path, object_path) {
        return None;
    }
    if path.len() <= object_path.len() {
        return None;
    }

    match &path[object_path.len()] {
        SchemaPathSegment::Key(key) => Some(key.clone()),
        SchemaPathSegment::Index(_) => None,
    }
}

fn path_starts_with(path: &[SchemaPathSegment], prefix: &[SchemaPathSegment]) -> bool {
    path.len() >= prefix.len()
        && path
            .iter()
            .zip(prefix.iter())
            .all(|(left, right)| left == right)
}

fn property_completion_item(
    property: CompletionProperty,
    is_top_level: bool,
    in_table_header: bool,
) -> CompletionItem {
    let detail = match (property.required, property.doc.schema_type.as_ref()) {
        (true, Some(schema_type)) => format!("Required, {schema_type}"),
        (true, None) => "Required".to_string(),
        (false, Some(schema_type)) => schema_type.clone(),
        (false, None) => String::new(),
    };
    let insertion = key_value_insertion(&property, is_top_level, in_table_header);

    CompletionItem {
        label: property.name.clone(),
        kind: Some(CompletionItemKind::FIELD),
        detail: (!detail.is_empty()).then_some(detail),
        documentation: completion_doc(&property.doc),
        insert_text: Some(if insertion.is_full_insert {
            insertion.value
        } else {
            format!("{} = {}", property.name, insertion.value)
        }),
        insert_text_format: Some(insertion.format),
        sort_text: Some(if property.required {
            format!("0_{}", property.name)
        } else {
            format!("1_{}", property.name)
        }),
        ..Default::default()
    }
}

struct KeyValueInsertion {
    value: String,
    format: InsertTextFormat,
    is_full_insert: bool,
}

fn key_value_insertion(
    property: &CompletionProperty,
    is_top_level: bool,
    in_table_header: bool,
) -> KeyValueInsertion {
    if in_table_header {
        return KeyValueInsertion {
            value: property.name.clone(),
            format: InsertTextFormat::PLAIN_TEXT,
            is_full_insert: true,
        };
    }

    if is_top_level && property.doc.schema_type.as_deref() == Some("object") {
        return KeyValueInsertion {
            value: format!("[{}]\n$0", property.name),
            format: InsertTextFormat::SNIPPET,
            is_full_insert: true,
        };
    }

    let doc = &property.doc;

    if let Some(value) = doc
        .const_value
        .as_ref()
        .and_then(json_value_to_toml_literal)
    {
        return KeyValueInsertion {
            value: escape_snippet_text(&value),
            format: InsertTextFormat::SNIPPET,
            is_full_insert: false,
        };
    }
    if let Some(value) = doc
        .default_value
        .as_ref()
        .and_then(selected_schema_literal_snippet)
    {
        return KeyValueInsertion {
            value,
            format: InsertTextFormat::SNIPPET,
            is_full_insert: false,
        };
    }
    if let Some(value) = doc
        .enum_values
        .first()
        .and_then(selected_schema_literal_snippet)
    {
        return KeyValueInsertion {
            value,
            format: InsertTextFormat::SNIPPET,
            is_full_insert: false,
        };
    }

    let value = match doc.schema_type.as_deref() {
        Some("string") => "\"$1\"".to_string(),
        Some("array") => "[${1}]".to_string(),
        Some("object") => "{ $1 }".to_string(),
        Some("boolean") => "${1|true,false|}".to_string(),
        Some("integer") => "${1:0}".to_string(),
        Some("number") => "${1:0.0}".to_string(),
        _ => "$1".to_string(),
    };

    KeyValueInsertion {
        value,
        format: InsertTextFormat::SNIPPET,
        is_full_insert: false,
    }
}

fn selected_schema_literal_snippet(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(format!("\"${{1:{}}}\"", escape_snippet_text(text))),
        _ => json_value_to_toml_literal(value)
            .map(|literal| format!("${{1:{}}}", escape_snippet_text(&literal))),
    }
}

fn escape_snippet_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '$' | '}' | '\\' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

fn completion_doc(doc: &SchemaDoc) -> Option<Documentation> {
    let mut lines = Vec::new();

    if let Some(title) = doc.title.as_ref() {
        lines.push(format!("**{title}**"));
        lines.push(String::new());
    }

    if let Some(description) = doc.description.as_ref() {
        lines.push(description.clone());
        lines.push(String::new());
    }

    if let Some(schema_type) = doc.schema_type.as_ref() {
        lines.push(format!("- Type: `{schema_type}`"));
    }

    if let Some(default_value) = doc.default_value.as_ref() {
        lines.push(format!("- Default: `{}`", format_json_value(default_value)));
    }

    if let Some(const_value) = doc.const_value.as_ref() {
        lines.push(format!("- Const: `{}`", format_json_value(const_value)));
    }

    if lines.is_empty() {
        return None;
    }

    Some(Documentation::MarkupContent(MarkupContent {
        kind: MarkupKind::Markdown,
        value: lines.join("\n"),
    }))
}

fn value_completion_items(doc: SchemaDoc, in_string_literal: bool) -> Vec<CompletionItem> {
    let mut seen_labels = HashSet::new();
    let mut items = Vec::new();

    if let Some(const_value) = doc.const_value.as_ref()
        && let Some(label) = json_value_to_toml_literal(const_value)
        && let Some(insert_text) = value_completion_insert_text(const_value, in_string_literal)
    {
        seen_labels.insert(label.clone());
        items.push(CompletionItem {
            label: label.clone(),
            kind: Some(CompletionItemKind::CONSTANT),
            detail: Some("Const value".to_string()),
            insert_text: Some(insert_text),
            insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
            ..Default::default()
        });
    }

    for enum_value in &doc.enum_values {
        let Some(label) = json_value_to_toml_literal(enum_value) else {
            continue;
        };
        if !seen_labels.insert(label.clone()) {
            continue;
        }
        let Some(insert_text) = value_completion_insert_text(enum_value, in_string_literal) else {
            continue;
        };

        items.push(CompletionItem {
            label: label.clone(),
            kind: Some(CompletionItemKind::ENUM_MEMBER),
            detail: Some("Enum value".to_string()),
            insert_text: Some(insert_text),
            insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
            ..Default::default()
        });
    }

    if let Some(default_value) = doc.default_value.as_ref()
        && let Some(label) = json_value_to_toml_literal(default_value)
        && let Some(insert_text) = value_completion_insert_text(default_value, in_string_literal)
        && seen_labels.insert(label.clone())
    {
        items.push(CompletionItem {
            label: label.clone(),
            kind: Some(CompletionItemKind::VALUE),
            detail: Some("Default value".to_string()),
            insert_text: Some(insert_text),
            insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
            ..Default::default()
        });
    }

    for example in &doc.examples {
        let Some(label) = json_value_to_toml_literal(example) else {
            continue;
        };
        if !seen_labels.insert(label.clone()) {
            continue;
        }
        let Some(insert_text) = value_completion_insert_text(example, in_string_literal) else {
            continue;
        };

        items.push(CompletionItem {
            label: label.clone(),
            kind: Some(CompletionItemKind::VALUE),
            detail: Some("Example value".to_string()),
            insert_text: Some(insert_text),
            insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
            ..Default::default()
        });
    }

    if items.is_empty() && doc.schema_type.as_deref() == Some("boolean") {
        items.push(CompletionItem {
            label: "true".to_string(),
            kind: Some(CompletionItemKind::VALUE),
            insert_text: Some("true".to_string()),
            insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
            ..Default::default()
        });
        items.push(CompletionItem {
            label: "false".to_string(),
            kind: Some(CompletionItemKind::VALUE),
            insert_text: Some("false".to_string()),
            insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
            ..Default::default()
        });
    }

    items
}

fn value_completion_insert_text(value: &Value, in_string_literal: bool) -> Option<String> {
    if in_string_literal && let Value::String(text) = value {
        return Some(escape_toml_basic_string_content(text));
    }

    json_value_to_toml_literal(value)
}

fn escape_toml_basic_string_content(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

fn json_value_to_toml_literal(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(value) => Some(value.to_string()),
        Value::String(value) => Some(format!("\"{}\"", value.replace('"', "\\\""))),
        Value::Array(values) => {
            let mut rendered = Vec::with_capacity(values.len());
            for item in values {
                let literal = json_value_to_toml_literal(item)?;
                rendered.push(literal);
            }
            Some(format!("[{}]", rendered.join(", ")))
        }
        Value::Object(_) => None,
    }
}

fn format_json_value(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "null".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_json_values_to_toml_literals() {
        assert_eq!(
            json_value_to_toml_literal(&Value::Bool(true)),
            Some("true".to_string())
        );
        assert_eq!(
            json_value_to_toml_literal(&Value::Number(42.into())),
            Some("42".to_string())
        );
        assert_eq!(
            json_value_to_toml_literal(&Value::String("abc".to_string())),
            Some("\"abc\"".to_string())
        );
    }

    #[test]
    fn builds_boolean_value_completions_when_no_enum_or_default() {
        let items = value_completion_items(
            SchemaDoc {
                schema_type: Some("boolean".to_string()),
                ..Default::default()
            },
            false,
        );

        let labels = items.into_iter().map(|item| item.label).collect::<Vec<_>>();
        assert_eq!(labels, vec!["true".to_string(), "false".to_string()]);
    }

    #[test]
    fn collects_existing_keys_for_object_path() {
        let source = r#"
name = "app"
version = "1.0.0"

[package]
type = "contract"
"#;
        let snapshot = ParsedSnapshot::new(
            lsp_types::Url::parse("file:///tmp/Acton.toml").expect("snapshot uri should parse"),
            1,
            source,
            std::sync::Arc::new(toml_syntax::parse(source).expect("valid toml")),
        );

        let root = collect_existing_property_keys(&snapshot, &[], None);
        assert!(root.contains("name"));
        assert!(root.contains("version"));
        assert!(root.contains("package"));

        let package_path = [SchemaPathSegment::Key("package".to_string())];
        let package = collect_existing_property_keys(&snapshot, &package_path, None);
        assert!(package.contains("type"));
    }

    #[test]
    fn key_value_insertion_uses_typed_snippets() {
        let string = key_value_insertion(
            &CompletionProperty {
                name: "name".to_string(),
                required: false,
                doc: SchemaDoc {
                    schema_type: Some("string".to_string()),
                    ..Default::default()
                },
            },
            false,
            false,
        );
        assert_eq!(string.value, "\"$1\"");
        assert_eq!(string.format, InsertTextFormat::SNIPPET);

        let array = key_value_insertion(
            &CompletionProperty {
                name: "items".to_string(),
                required: false,
                doc: SchemaDoc {
                    schema_type: Some("array".to_string()),
                    ..Default::default()
                },
            },
            false,
            false,
        );
        assert_eq!(array.value, "[${1}]");

        let boolean = key_value_insertion(
            &CompletionProperty {
                name: "enabled".to_string(),
                required: false,
                doc: SchemaDoc {
                    schema_type: Some("boolean".to_string()),
                    ..Default::default()
                },
            },
            false,
            false,
        );
        assert_eq!(boolean.value, "${1|true,false|}");
    }

    #[test]
    fn key_value_insertion_prefers_schema_values() {
        let with_const = key_value_insertion(
            &CompletionProperty {
                name: "mode".to_string(),
                required: false,
                doc: SchemaDoc {
                    schema_type: Some("string".to_string()),
                    const_value: Some(Value::String("fixed".to_string())),
                    ..Default::default()
                },
            },
            false,
            false,
        );
        assert_eq!(with_const.value, "\"fixed\"");

        let with_default = key_value_insertion(
            &CompletionProperty {
                name: "mode".to_string(),
                required: false,
                doc: SchemaDoc {
                    schema_type: Some("string".to_string()),
                    default_value: Some(Value::String("abc".to_string())),
                    ..Default::default()
                },
            },
            false,
            false,
        );
        assert_eq!(with_default.value, "\"${1:abc}\"");

        let with_enum = key_value_insertion(
            &CompletionProperty {
                name: "mode".to_string(),
                required: false,
                doc: SchemaDoc {
                    schema_type: Some("string".to_string()),
                    enum_values: vec![Value::String("dev".to_string())],
                    ..Default::default()
                },
            },
            false,
            false,
        );
        assert_eq!(with_enum.value, "\"${1:dev}\"");
    }

    #[test]
    fn top_level_object_key_inserts_table_header() {
        let insertion = key_value_insertion(
            &CompletionProperty {
                name: "localnet".to_string(),
                required: false,
                doc: SchemaDoc {
                    schema_type: Some("object".to_string()),
                    ..Default::default()
                },
            },
            true,
            false,
        );

        assert_eq!(insertion.value, "[localnet]\n$0");
        assert_eq!(insertion.format, InsertTextFormat::SNIPPET);
    }

    #[test]
    fn top_level_object_completion_item_uses_table_header_without_equals() {
        let item = property_completion_item(
            CompletionProperty {
                name: "localnet".to_string(),
                required: false,
                doc: SchemaDoc {
                    schema_type: Some("object".to_string()),
                    ..Default::default()
                },
            },
            true,
            false,
        );

        assert_eq!(item.insert_text.as_deref(), Some("[localnet]\n$0"));
    }

    #[test]
    fn table_header_completion_item_inserts_only_name() {
        let item = property_completion_item(
            CompletionProperty {
                name: "localnet".to_string(),
                required: false,
                doc: SchemaDoc {
                    schema_type: Some("object".to_string()),
                    ..Default::default()
                },
            },
            true,
            true,
        );

        assert_eq!(item.insert_text.as_deref(), Some("localnet"));
    }

    #[test]
    fn value_completion_in_string_literal_omits_wrapping_quotes() {
        let items = value_completion_items(
            SchemaDoc {
                enum_values: vec![Value::String("github".to_string())],
                ..Default::default()
            },
            true,
        );

        let github = items
            .iter()
            .find(|item| item.label == "\"github\"")
            .expect("enum completion item must exist");
        assert_eq!(github.insert_text.as_deref(), Some("github"));
    }

    #[test]
    fn detects_table_header_brackets_context() {
        let source = "[fmt]\nwidth = 100\nignore = []\n\n[]";
        let cursor = source.rfind(']').expect("closing bracket exists");

        assert!(is_in_table_header_brackets(source, cursor));
        assert!(!is_in_table_header_brackets(
            source,
            source.find("width").unwrap_or(0)
        ));
        assert!(!is_in_table_header_brackets(
            source,
            source.find("ignore = []").unwrap_or(0)
        ));
    }
}
