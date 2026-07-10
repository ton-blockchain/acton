use super::super::schema::format_json;
use crate::{CompletionItem, CompletionItemKind, InsertTextFormat, Range};
use serde_json::Value;
use std::collections::HashSet;
use ton_json_schema::{CompletionProperty, SchemaDoc};

pub(super) fn property_item(
    property: CompletionProperty,
    is_top_level: bool,
    in_table_header: bool,
    key_only: bool,
    replacement_range: Range,
) -> CompletionItem {
    let detail = match (property.required, property.doc.schema_type.as_deref()) {
        (true, Some(schema_type)) => format!("Required, {schema_type}"),
        (true, None) => "Required".to_owned(),
        (false, Some(schema_type)) => schema_type.to_owned(),
        (false, None) => String::new(),
    };
    let insertion = key_insertion(&property, is_top_level, in_table_header, key_only);
    let mut item = CompletionItem::new(&property.name, CompletionItemKind::Field)
        .with_replacement(replacement_range, &insertion.text);
    item.insert_text = Some(insertion.text);
    item.insert_text_format = insertion.format;
    item.sort_text = Some(format!(
        "{}_{}",
        if property.required { 0 } else { 1 },
        property.name
    ));
    if !detail.is_empty() {
        item.detail = Some(detail);
    }
    item.documentation = property_documentation(&property.doc);
    item
}

struct Insertion {
    text: String,
    format: InsertTextFormat,
}

fn key_insertion(
    property: &CompletionProperty,
    is_top_level: bool,
    in_table_header: bool,
    key_only: bool,
) -> Insertion {
    if in_table_header || key_only {
        return Insertion {
            text: property.name.clone(),
            format: InsertTextFormat::PlainText,
        };
    }
    if is_top_level && property.doc.schema_type.as_deref() == Some("object") {
        return Insertion {
            text: format!("[{}]\n$0", property.name),
            format: InsertTextFormat::Snippet,
        };
    }

    let value = property
        .doc
        .const_value
        .as_ref()
        .and_then(json_to_toml)
        .map(|value| escape_snippet(&value))
        .or_else(|| {
            property
                .doc
                .default_value
                .as_ref()
                .and_then(selected_literal)
        })
        .or_else(|| property.doc.enum_values.first().and_then(selected_literal))
        .unwrap_or_else(|| match property.doc.schema_type.as_deref() {
            Some("string") => "\"$1\"".to_owned(),
            Some("array") => "[${1}]".to_owned(),
            Some("object") => "{ $1 }".to_owned(),
            Some("boolean") => "${1|true,false|}".to_owned(),
            Some("integer") => "${1:0}".to_owned(),
            Some("number") => "${1:0.0}".to_owned(),
            _ => "$1".to_owned(),
        });

    Insertion {
        text: format!("{} = {value}", property.name),
        format: InsertTextFormat::Snippet,
    }
}

fn property_documentation(doc: &SchemaDoc) -> Option<String> {
    let mut lines = Vec::new();
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
    (!lines.is_empty()).then(|| lines.join("\n"))
}

pub(super) fn value_items(doc: SchemaDoc, in_string: bool, range: Range) -> Vec<CompletionItem> {
    let mut labels = HashSet::new();
    let mut items = Vec::new();

    if let Some(value) = &doc.const_value {
        push_value_item(
            &mut items,
            &mut labels,
            value,
            in_string,
            range,
            CompletionItemKind::Constant,
            "Const value",
        );
    }
    for value in &doc.enum_values {
        push_value_item(
            &mut items,
            &mut labels,
            value,
            in_string,
            range,
            CompletionItemKind::EnumMember,
            "Enum value",
        );
    }
    if let Some(value) = &doc.default_value {
        push_value_item(
            &mut items,
            &mut labels,
            value,
            in_string,
            range,
            CompletionItemKind::Value,
            "Default value",
        );
    }
    for value in &doc.examples {
        push_value_item(
            &mut items,
            &mut labels,
            value,
            in_string,
            range,
            CompletionItemKind::Value,
            "Example value",
        );
    }

    if items.is_empty() && doc.schema_type.as_deref() == Some("boolean") {
        for value in ["true", "false"] {
            items.push(
                CompletionItem::new(value, CompletionItemKind::Value)
                    .with_replacement(range, value),
            );
        }
    }
    items
}

fn push_value_item(
    items: &mut Vec<CompletionItem>,
    labels: &mut HashSet<String>,
    value: &Value,
    in_string: bool,
    range: Range,
    kind: CompletionItemKind,
    detail: &str,
) {
    let Some(label) = json_to_toml(value) else {
        return;
    };
    if !labels.insert(label.clone()) {
        return;
    }
    let replacement = if in_string {
        let Value::String(value) = value else {
            return;
        };
        escape_toml_string(value)
    } else {
        label.clone()
    };
    items.push(
        CompletionItem::new(label, kind)
            .with_detail(detail)
            .with_replacement(range, replacement),
    );
}

fn selected_literal(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(format!("\"${{1:{}}}\"", escape_snippet(value))),
        _ => json_to_toml(value).map(|value| format!("${{1:{}}}", escape_snippet(&value))),
    }
}

fn json_to_toml(value: &Value) -> Option<String> {
    match value {
        Value::Null | Value::Object(_) => None,
        Value::Bool(_) | Value::Number(_) | Value::String(_) => Some(value.to_string()),
        Value::Array(values) => values
            .iter()
            .map(json_to_toml)
            .collect::<Option<Vec<_>>>()
            .map(|values| format!("[{}]", values.join(", "))),
    }
}

fn escape_snippet(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('$', "\\$")
        .replace('}', "\\}")
}

fn escape_toml_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
