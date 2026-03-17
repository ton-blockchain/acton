use crate::backend::Backend;
use crate::languages::engine::cache::ParsedSnapshot;
use lsp_types::{Hover, HoverContents, HoverParams, MarkupContent, MarkupKind};
use serde_json::Value as JsonValue;
use std::path::Path;
use std::sync::OnceLock;
use toml_syntax::{Key, Pair, TopLevel, Value as TomlValue};
use ton_json_schema::{SchemaDoc, SchemaPathSegment, SchemaStore};
use tree_sitter::Node;

static ACTON_SCHEMA_STORE: OnceLock<Option<SchemaStore>> = OnceLock::new();
const ACTON_SCHEMA_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../acton.schema.json"
));

impl Backend {
    pub async fn handle_toml_hover(&self, params: HoverParams) -> Option<Hover> {
        crate::profile!(self, "toml: hover");
        let uri = params.text_document_position_params.text_document.uri;

        let path = uri.to_file_path().ok()?;
        if !is_acton_toml(&path) {
            return None;
        }

        let file = self.registry.find_toml_file(&uri)?;

        let node = file.node_at(params.text_document_position_params.position)?;
        let schema_path = find_schema_path(&file, node)?;

        let schema = get_acton_schema_store()?;
        let doc = schema.summary_for_path(&schema_path)?;

        let markdown = build_hover_markdown(&schema_path, &doc)?;
        let range = file.range_of(node);

        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: markdown,
            }),
            range: Some(range),
        })
    }
}

pub(super) fn is_acton_toml(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("Acton.toml"))
}

pub(super) fn get_acton_schema_store() -> Option<&'static SchemaStore> {
    ACTON_SCHEMA_STORE
        .get_or_init(|| SchemaStore::from_json_str(ACTON_SCHEMA_JSON).ok())
        .as_ref()
}

pub(super) fn find_schema_path(
    file: &ParsedSnapshot<toml_syntax::SourceFile>,
    target: Node<'_>,
) -> Option<Vec<SchemaPathSegment>> {
    let source = file.source();
    let mut current_table = Vec::new();
    let source_file = file.syntax();

    for top_level in source_file.top_levels() {
        let syntax = top_level.syntax();
        let contains_target = contains_node(syntax, target);

        match top_level {
            TopLevel::Table(table) => {
                let key = table.key()?;
                let table_path = key_to_path_segments(key, source);
                if contains_target {
                    for pair in table.pairs() {
                        if !contains_node(pair.0, target) {
                            continue;
                        }
                        let Some(mut pair_path) = pair_to_path_segments(pair, target, source)
                        else {
                            continue;
                        };
                        let mut full_path = table_path.clone();
                        full_path.append(&mut pair_path);
                        return Some(full_path);
                    }
                    return Some(table_path);
                }
                current_table = table_path;
            }
            TopLevel::TableArrayElement(table) => {
                let key = table.key()?;
                let mut table_path = key_to_path_segments(key, source);
                table_path.push(SchemaPathSegment::Index(0));
                if contains_target {
                    for pair in table.pairs() {
                        if !contains_node(pair.0, target) {
                            continue;
                        }
                        let Some(mut pair_path) = pair_to_path_segments(pair, target, source)
                        else {
                            continue;
                        };
                        let mut full_path = table_path.clone();
                        full_path.append(&mut pair_path);
                        return Some(full_path);
                    }
                    return Some(table_path);
                }
                current_table = table_path;
            }
            TopLevel::Pair(pair) => {
                if !contains_target {
                    continue;
                }

                let mut full_path = current_table.clone();
                let mut pair_path = pair_to_path_segments(pair, target, source)?;
                full_path.append(&mut pair_path);
                return Some(full_path);
            }
            TopLevel::Unmapped(_) => {}
        }
    }

    None
}

fn contains_node(container: Node<'_>, node: Node<'_>) -> bool {
    node.start_byte() >= container.start_byte() && node.end_byte() <= container.end_byte()
}

fn key_to_path_segments(key: Key<'_>, source: &str) -> Vec<SchemaPathSegment> {
    let mut result = Vec::new();
    push_key_segments(key, source, &mut result);
    result
}

fn pair_to_path_segments(
    pair: Pair<'_>,
    target: Node<'_>,
    source: &str,
) -> Option<Vec<SchemaPathSegment>> {
    let key = pair.key()?;
    let mut path = key_to_path_segments(key, source);

    if let Some(value) = pair.value()
        && contains_node(value.syntax(), target)
        && let Some(mut nested_path) = value_to_path_segments(value, target, source)
    {
        path.append(&mut nested_path);
    }

    Some(path)
}

fn value_to_path_segments(
    value: TomlValue<'_>,
    target: Node<'_>,
    source: &str,
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
                if let Some(mut nested_path) = value_to_path_segments(item, target, source) {
                    path.append(&mut nested_path);
                }
                return Some(path);
            }

            Some(Vec::new())
        }
        TomlValue::InlineTable(table) => {
            for pair in table.pairs() {
                if !contains_node(pair.0, target) {
                    continue;
                }
                return pair_to_path_segments(pair, target, source);
            }

            Some(Vec::new())
        }
        TomlValue::Unmapped(raw) => {
            let text = raw.text(source);
            if text.is_empty() {
                return None;
            }
            Some(Vec::new())
        }
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

fn push_key_segments(key: Key<'_>, source: &str, result: &mut Vec<SchemaPathSegment>) {
    match key {
        Key::Bare(_) | Key::Quoted(_) => {
            let value = normalize_key_text(key.text(source));
            if !value.is_empty() {
                result.push(SchemaPathSegment::Key(value));
            }
        }
        Key::Dotted(dotted) => {
            for part in dotted.parts() {
                push_key_segments(part, source, result);
            }
        }
        Key::Unmapped(raw) => {
            let text = raw.text(source);
            for part in text.split('.') {
                let value = normalize_key_text(part);
                if !value.is_empty() {
                    result.push(SchemaPathSegment::Key(value));
                }
            }
        }
    }
}

fn normalize_key_text(text: &str) -> String {
    let trimmed = text.trim();
    if let Some(value) = trimmed
        .strip_prefix('"')
        .and_then(|it| it.strip_suffix('"'))
    {
        return value.to_string();
    }
    if let Some(value) = trimmed
        .strip_prefix('\'')
        .and_then(|it| it.strip_suffix('\''))
    {
        return value.to_string();
    }
    trimmed.to_string()
}

fn build_hover_markdown(path: &[SchemaPathSegment], doc: &SchemaDoc) -> Option<String> {
    if doc.is_empty() {
        return None;
    }

    let mut lines = vec![
        "```toml".to_string(),
        format_path(path),
        "```".to_string(),
        String::new(),
    ];

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

    if !doc.enum_values.is_empty() {
        let values = doc
            .enum_values
            .iter()
            .map(format_json_value)
            .collect::<Vec<_>>()
            .join(" | ");
        lines.push(format!("- Enum: `{values}`"));
    }

    if !doc.examples.is_empty() {
        let values = doc
            .examples
            .iter()
            .map(format_json_value)
            .collect::<Vec<_>>()
            .join(" | ");
        lines.push(format!("- Examples: `{values}`"));
    }

    Some(lines.join("\n"))
}

fn format_path(path: &[SchemaPathSegment]) -> String {
    let mut result = String::new();

    for segment in path {
        match segment {
            SchemaPathSegment::Key(key) => {
                if !result.is_empty() {
                    result.push('.');
                }
                result.push_str(key);
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

fn format_json_value(value: &JsonValue) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "null".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn parse_snapshot(source: &str) -> ParsedSnapshot<toml_syntax::SourceFile> {
        ParsedSnapshot::new(
            lsp_types::Url::parse("file:///tmp/Acton.toml").expect("snapshot uri should parse"),
            1,
            Arc::from(source),
            Arc::new(toml_syntax::parse(source).expect("toml should parse")),
        )
    }

    #[test]
    fn resolves_pair_path_inside_table() {
        let source = "[package]\nname = \"Acton\"\n";
        let snapshot = parse_snapshot(source);
        let offset = source.find("name").expect("name key exists");
        let node = snapshot
            .node_at(snapshot.position(offset))
            .expect("node at offset must exist");

        let path = find_schema_path(&snapshot, node).expect("schema path should resolve");

        assert_eq!(
            path,
            vec![
                SchemaPathSegment::Key("package".to_string()),
                SchemaPathSegment::Key("name".to_string())
            ]
        );
    }

    #[test]
    fn resolves_dotted_table_path() {
        let source = "[networks.localnet]\napi = { v2 = \"http://localhost\" }\n";
        let snapshot = parse_snapshot(source);
        let offset = source.find("v2 =").expect("key exists");
        let node = snapshot
            .node_at(snapshot.position(offset))
            .expect("node at offset must exist");

        let path = find_schema_path(&snapshot, node).expect("schema path should resolve");

        assert_eq!(
            path,
            vec![
                SchemaPathSegment::Key("networks".to_string()),
                SchemaPathSegment::Key("localnet".to_string()),
                SchemaPathSegment::Key("api".to_string()),
                SchemaPathSegment::Key("v2".to_string())
            ]
        );
    }

    #[test]
    fn resolves_inline_table_path_inside_array() {
        let source = "[contracts.acton_jetton_minter]\ndepends = [{ name = \"wallet\", function = \"actonJettonWalletCompiledCode\" }]\n";
        let snapshot = parse_snapshot(source);
        let offset = source.find("function").expect("inline table key exists");
        let node = snapshot
            .node_at(snapshot.position(offset))
            .expect("node at offset must exist");

        let path = find_schema_path(&snapshot, node).expect("schema path should resolve");

        assert_eq!(
            path,
            vec![
                SchemaPathSegment::Key("contracts".to_string()),
                SchemaPathSegment::Key("acton_jetton_minter".to_string()),
                SchemaPathSegment::Key("depends".to_string()),
                SchemaPathSegment::Index(0),
                SchemaPathSegment::Key("function".to_string())
            ]
        );
    }
}
