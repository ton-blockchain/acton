use super::TomlParsedDocument;
use super::schema::{is_acton_manifest, parse_key_path};
use crate::{DocumentSnapshot, InlayHint, Range};
use toml_syntax::{AstNode, Pair, TopLevel, Value};
use ton_json_schema::SchemaPathSegment;

pub(super) fn inlay_hints(
    document: &DocumentSnapshot,
    parsed: &TomlParsedDocument,
    range: Range,
    acton_version: &str,
) -> Vec<InlayHint> {
    if !is_acton_manifest(document.uri()) {
        return Vec::new();
    }

    let mut hints = Vec::new();
    for top_level in parsed.source_file.top_levels() {
        match top_level {
            TopLevel::Pair(pair) => {
                collect_acton_version_hint(document, pair, &[], range, acton_version, &mut hints);
            }
            TopLevel::Table(table) => {
                let Some(key) = table.key() else {
                    continue;
                };
                let Some(table_path) = parse_key_path(document.text_of(key)) else {
                    continue;
                };
                for pair in table.pairs() {
                    collect_acton_version_hint(
                        document,
                        pair,
                        &table_path,
                        range,
                        acton_version,
                        &mut hints,
                    );
                }
            }
            TopLevel::TableArrayElement(_) | TopLevel::Unmapped(_) => {}
        }
    }
    hints
}

fn collect_acton_version_hint(
    document: &DocumentSnapshot,
    pair: Pair<'_>,
    table_path: &[SchemaPathSegment],
    range: Range,
    acton_version: &str,
    hints: &mut Vec<InlayHint>,
) {
    let Some(key) = pair.key() else {
        return;
    };
    let Some(mut path) = parse_key_path(document.text_of(key)) else {
        return;
    };
    path.splice(0..0, table_path.iter().cloned());
    if !matches!(
        path.as_slice(),
        [SchemaPathSegment::Key(table), SchemaPathSegment::Key(field)]
            if table == "toolchain" && field == "acton"
    ) {
        return;
    }

    let Some(Value::String(value)) = pair.value() else {
        return;
    };
    let position = document
        .text_index()
        .offset_to_position(document.text(), value.syntax().end_byte());
    if position < range.start || position > range.end {
        return;
    }

    let mut hint = InlayHint::plain(position, format!("installed: {acton_version}"));
    hint.tooltip = Some("Version of the Acton CLI running the language server".to_owned());
    hint.padding_left = true;
    hints.push(hint);
}
