use super::super::TomlParsedDocument;
use super::super::schema::{
    ancestor, contains_node, parse_key_path, schema_path, table_path_at_offset,
};
use crate::{DocumentSnapshot, Position, Range};
use std::collections::HashSet;
use toml_syntax::{AstNode, TopLevel};
use ton_json_schema::{CompletionProperty, SchemaPathSegment};
use tree_sitter::Node;

pub(super) enum CompletionContext {
    Keys {
        object_path: Vec<SchemaPathSegment>,
        existing_keys: HashSet<String>,
        header_kind: HeaderKind,
        key_only: bool,
        replacement_range: Range,
    },
    Values {
        value_path: Vec<SchemaPathSegment>,
        in_string: bool,
        replacement_range: Range,
    },
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) enum HeaderKind {
    #[default]
    None,
    Table,
    TableArray,
}

impl HeaderKind {
    pub(super) const fn is_header(self) -> bool {
        !matches!(self, Self::None)
    }

    pub(super) fn allows(
        self,
        schema: &ton_json_schema::SchemaStore,
        object_path: &[SchemaPathSegment],
        property: &CompletionProperty,
    ) -> bool {
        match self {
            Self::None => true,
            Self::Table => property.doc.schema_type.as_deref() == Some("object"),
            Self::TableArray => {
                let mut item_path = object_path.to_vec();
                item_path.push(SchemaPathSegment::Key(property.name.clone()));
                item_path.push(SchemaPathSegment::Index(0));

                schema
                    .completion_for_path(&item_path)
                    .is_some_and(|completion| !completion.properties.is_empty())
            }
        }
    }
}

impl CompletionContext {
    pub(super) fn new(
        document: &DocumentSnapshot,
        parsed: &TomlParsedDocument,
        node: Option<Node<'_>>,
        offset: usize,
        position: Position,
    ) -> Option<Self> {
        if node.is_some_and(|node| ancestor(node, "comment").is_some()) {
            return None;
        }

        let string = node.and_then(|node| ancestor(node, "string"));
        if string.is_none()
            && let Some(header) = header_context(document, offset)
        {
            let existing_keys = if matches!(header.kind, HeaderKind::TableArray) {
                HashSet::new()
            } else {
                existing_keys(document, parsed, &header.object_path, node)
            };

            return Some(Self::Keys {
                existing_keys,
                object_path: header.object_path,
                header_kind: header.kind,
                key_only: true,
                replacement_range: header.replacement_range,
            });
        }

        let Some(node) = node else {
            return Some(Self::root_keys(document, parsed, position));
        };
        let full_path = schema_path(document, parsed, node)
            .unwrap_or_else(|| table_path_at_offset(document, parsed, offset));
        let collection_context = collection_key_context(node, offset);

        if !collection_context && is_in_pair_value(node, offset) {
            return Some(Self::Values {
                value_path: full_path,
                in_string: string.is_some(),
                replacement_range: string.map_or_else(
                    || node_replacement_range(document, node, position),
                    |string| string_content_range(document, string),
                ),
            });
        }

        if !collection_context
            && let Some(assignment) = assignment_context(document, parsed, offset)
        {
            return Some(Self::Values {
                value_path: assignment.value_path,
                in_string: assignment.in_string,
                replacement_range: assignment.replacement_range,
            });
        }

        let object_path = if collection_context {
            full_path
        } else if ancestor(node, "pair").is_some() {
            without_last(full_path)
        } else {
            full_path
        };
        let assignment_key_range = assignment_key_range(document, offset);
        Some(Self::Keys {
            existing_keys: existing_keys(document, parsed, &object_path, Some(node)),
            object_path,
            header_kind: HeaderKind::None,
            key_only: assignment_key_range.is_some(),
            replacement_range: assignment_key_range
                .unwrap_or_else(|| node_replacement_range(document, node, position)),
        })
    }

    fn root_keys(
        document: &DocumentSnapshot,
        parsed: &TomlParsedDocument,
        position: Position,
    ) -> Self {
        Self::Keys {
            object_path: Vec::new(),
            existing_keys: existing_keys(document, parsed, &[], None),
            header_kind: HeaderKind::None,
            key_only: false,
            replacement_range: Range::new(position, position),
        }
    }
}

fn collection_key_context(node: Node<'_>, offset: usize) -> bool {
    matches!(node.kind(), "array" | "inline_table")
        && offset >= node.start_byte()
        && offset <= node.end_byte()
}

fn without_last(mut path: Vec<SchemaPathSegment>) -> Vec<SchemaPathSegment> {
    path.pop();
    path
}

fn is_in_pair_value(node: Node<'_>, offset: usize) -> bool {
    let Some(pair) = ancestor(node, "pair") else {
        return false;
    };
    let mut cursor = pair.walk();
    let mut children = pair.named_children(&mut cursor);
    children.next();
    children.next().is_some_and(|value| {
        contains_node(value, node) && offset >= value.start_byte() && offset <= value.end_byte()
    })
}

struct HeaderContext {
    object_path: Vec<SchemaPathSegment>,
    kind: HeaderKind,
    replacement_range: Range,
}

fn header_context(document: &DocumentSnapshot, offset: usize) -> Option<HeaderContext> {
    let source = document.text();
    let offset = offset.min(source.len());
    let line_start = source[..offset].rfind('\n').map_or(0, |index| index + 1);
    let line_end = source[offset..]
        .find('\n')
        .map_or(source.len(), |index| offset + index);
    let line = &source[line_start..line_end];
    let first = line.find(|character: char| !character.is_whitespace())?;
    let rest = &line[first..];
    let (kind, opening_len, closing) = if rest.starts_with("[[") {
        (HeaderKind::TableArray, 2, "]]")
    } else if rest.starts_with('[') {
        (HeaderKind::Table, 1, "]")
    } else {
        return None;
    };

    let column = offset.saturating_sub(line_start);
    let content_start = first + opening_len;
    let content_end = line[content_start..]
        .find(closing)
        .map_or(line.len(), |index| content_start + index);
    if column < content_start || column > content_end {
        return None;
    }

    let prefix = &line[content_start..column];
    let separator = last_key_separator(prefix);
    let segment_start = separator.map_or(content_start, |index| content_start + index + 1);
    let object_text = separator.map_or("", |index| &prefix[..index]);
    let object_path = if object_text.trim().is_empty() {
        Vec::new()
    } else {
        parse_key_path(object_text.trim())?
    };

    let replacement_start = line_start
        + segment_start
        + line[segment_start..column]
            .len()
            .saturating_sub(line[segment_start..column].trim_start().len());
    let replacement_end = line_start
        + line[column..content_end]
            .find(|character: char| {
                character == '.' || character == ']' || character.is_whitespace()
            })
            .map_or(content_end, |length| column + length);

    Some(HeaderContext {
        object_path,
        kind,
        replacement_range: Range::new(
            document
                .text_index()
                .offset_to_position(source, replacement_start),
            document
                .text_index()
                .offset_to_position(source, replacement_end),
        ),
    })
}

fn last_key_separator(text: &str) -> Option<usize> {
    let mut quote = None;
    let mut escaped = false;
    let mut last = None;

    for (index, character) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if quote == Some('"') && character == '\\' {
            escaped = true;
            continue;
        }
        if matches!(character, '\'' | '"') {
            if quote == Some(character) {
                quote = None;
            } else if quote.is_none() {
                quote = Some(character);
            }
        } else if character == '.' && quote.is_none() {
            last = Some(index);
        }
    }

    last
}

struct AssignmentContext {
    value_path: Vec<SchemaPathSegment>,
    in_string: bool,
    replacement_range: Range,
}

fn assignment_context(
    document: &DocumentSnapshot,
    parsed: &TomlParsedDocument,
    offset: usize,
) -> Option<AssignmentContext> {
    let source = document.text();
    let offset = offset.min(source.len());
    let line_start = source[..offset].rfind('\n').map_or(0, |index| index + 1);
    let line_end = source[offset..]
        .find('\n')
        .map_or(source.len(), |index| offset + index);
    let line = &source[line_start..line_end];
    let column = offset.saturating_sub(line_start);
    let equals = assignment_separator(line)?;
    if column <= equals {
        return None;
    }

    let mut value_path = table_path_at_offset(document, parsed, offset);
    value_path.extend(parse_key_path(line[..equals].trim())?);

    let raw_value = &line[equals + 1..];
    let leading = raw_value.len().saturating_sub(raw_value.trim_start().len());
    let value_start = equals + 1 + leading;
    let value_without_comment = strip_toml_comment(&line[value_start..]);
    let value_end = value_start + value_without_comment.trim_end().len();
    let quoted = line[value_start..value_end]
        .chars()
        .next()
        .filter(|character| matches!(character, '\'' | '"'));
    let closed_string = quoted.is_some_and(|quote| {
        line[value_start..value_end].len() >= quote.len_utf8() * 2
            && line[value_start..value_end].ends_with(quote)
    });
    let replacement_start = value_start + usize::from(closed_string);
    let replacement_end = value_end.saturating_sub(usize::from(closed_string));

    Some(AssignmentContext {
        value_path,
        in_string: closed_string,
        replacement_range: Range::new(
            document
                .text_index()
                .offset_to_position(source, line_start + replacement_start),
            document
                .text_index()
                .offset_to_position(source, line_start + replacement_end),
        ),
    })
}

fn assignment_key_range(document: &DocumentSnapshot, offset: usize) -> Option<Range> {
    let source = document.text();
    let offset = offset.min(source.len());
    let line_start = source[..offset].rfind('\n').map_or(0, |index| index + 1);
    let line_end = source[offset..]
        .find('\n')
        .map_or(source.len(), |index| offset + index);
    let line = &source[line_start..line_end];
    let column = offset.saturating_sub(line_start);
    let equals = assignment_separator(line)?;
    if column > equals {
        return None;
    }

    let key = &line[..equals];
    let start = key.len().saturating_sub(key.trim_start().len());
    let end = key.trim_end().len();

    Some(Range::new(
        document
            .text_index()
            .offset_to_position(source, line_start + start),
        document
            .text_index()
            .offset_to_position(source, line_start + end),
    ))
}

fn assignment_separator(line: &str) -> Option<usize> {
    let mut quote = None;
    let mut escaped = false;

    for (index, character) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if quote == Some('"') && character == '\\' {
            escaped = true;
            continue;
        }
        if matches!(character, '\'' | '"') {
            if quote == Some(character) {
                quote = None;
            } else if quote.is_none() {
                quote = Some(character);
            }
        } else if character == '=' && quote.is_none() {
            return Some(index);
        } else if character == '#' && quote.is_none() {
            return None;
        }
    }

    None
}

fn strip_toml_comment(text: &str) -> &str {
    let mut quote = None;
    let mut escaped = false;

    for (index, character) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if quote == Some('"') && character == '\\' {
            escaped = true;
            continue;
        }
        if matches!(character, '\'' | '"') {
            if quote == Some(character) {
                quote = None;
            } else if quote.is_none() {
                quote = Some(character);
            }
        } else if character == '#' && quote.is_none() {
            return &text[..index];
        }
    }

    text
}

fn existing_keys(
    document: &DocumentSnapshot,
    parsed: &TomlParsedDocument,
    object_path: &[SchemaPathSegment],
    cursor_node: Option<Node<'_>>,
) -> HashSet<String> {
    let mut result = HashSet::new();

    for top_level in parsed.source_file.top_levels() {
        match top_level {
            TopLevel::Pair(pair) => collect_pair_key(
                document,
                parsed,
                pair,
                object_path,
                cursor_node,
                &mut result,
            ),
            TopLevel::Table(table) => {
                collect_key_at_node(
                    document,
                    parsed,
                    table.syntax(),
                    object_path,
                    cursor_node,
                    &mut result,
                );
                for pair in table.pairs() {
                    collect_pair_key(
                        document,
                        parsed,
                        pair,
                        object_path,
                        cursor_node,
                        &mut result,
                    );
                }
            }
            TopLevel::TableArrayElement(table) => {
                collect_key_at_node(
                    document,
                    parsed,
                    table.syntax(),
                    object_path,
                    cursor_node,
                    &mut result,
                );
                for pair in table.pairs() {
                    collect_pair_key(
                        document,
                        parsed,
                        pair,
                        object_path,
                        cursor_node,
                        &mut result,
                    );
                }
            }
            TopLevel::Unmapped(_) => {}
        }
    }
    result
}

fn collect_pair_key(
    document: &DocumentSnapshot,
    parsed: &TomlParsedDocument,
    pair: toml_syntax::Pair<'_>,
    object_path: &[SchemaPathSegment],
    cursor_node: Option<Node<'_>>,
    result: &mut HashSet<String>,
) {
    collect_key_at_node(
        document,
        parsed,
        pair.syntax(),
        object_path,
        cursor_node,
        result,
    );

    let Some(value) = pair.value() else {
        return;
    };
    match value {
        toml_syntax::Value::Array(array) => {
            for value in array.values() {
                if let toml_syntax::Value::InlineTable(table) = value {
                    for pair in table.pairs() {
                        collect_pair_key(document, parsed, pair, object_path, cursor_node, result);
                    }
                }
            }
        }
        toml_syntax::Value::InlineTable(table) => {
            for pair in table.pairs() {
                collect_pair_key(document, parsed, pair, object_path, cursor_node, result);
            }
        }
        toml_syntax::Value::String(_)
        | toml_syntax::Value::Integer(_)
        | toml_syntax::Value::Float(_)
        | toml_syntax::Value::Boolean(_)
        | toml_syntax::Value::OffsetDateTime(_)
        | toml_syntax::Value::LocalDateTime(_)
        | toml_syntax::Value::LocalDate(_)
        | toml_syntax::Value::LocalTime(_)
        | toml_syntax::Value::Unmapped(_) => {}
    }
}

fn collect_key_at_node(
    document: &DocumentSnapshot,
    parsed: &TomlParsedDocument,
    candidate: Node<'_>,
    object_path: &[SchemaPathSegment],
    cursor_node: Option<Node<'_>>,
    result: &mut HashSet<String>,
) {
    if cursor_node.is_some_and(|cursor| contains_node(candidate, cursor)) {
        return;
    }
    let Some(path) = schema_path(document, parsed, candidate) else {
        return;
    };
    if path.len() <= object_path.len() || !path.starts_with(object_path) {
        return;
    }
    if let SchemaPathSegment::Key(key) = &path[object_path.len()] {
        result.insert(key.clone());
    }
}

fn node_replacement_range(
    document: &DocumentSnapshot,
    node: Node<'_>,
    position: Position,
) -> Range {
    let replaceable = ancestor(node, "bare_key")
        .or_else(|| ancestor(node, "quoted_key"))
        .or_else(|| ancestor(node, "boolean"))
        .or_else(|| ancestor(node, "integer"))
        .or_else(|| ancestor(node, "float"));
    replaceable.map_or_else(
        || Range::new(position, position),
        |node| document.text_index().range_of_node(document.text(), node),
    )
}

fn string_content_range(document: &DocumentSnapshot, string: Node<'_>) -> Range {
    let mut range = document.text_index().range_of_node(document.text(), string);
    let text = document.text_of(string);
    if (text.starts_with('"') && text.ends_with('"'))
        || (text.starts_with('\'') && text.ends_with('\''))
    {
        range.start.character += 1;
        range.end.character = range.end.character.saturating_sub(1);
    }
    range
}
