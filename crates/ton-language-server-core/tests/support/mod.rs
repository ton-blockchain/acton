use std::fmt::Write as _;
use ton_language_server_core::{
    CompletionItem, InlayHint, InlayHintKind, Location, Position, SEMANTIC_TOKEN_MODIFIER_NAMES,
    SEMANTIC_TOKEN_TYPE_NAMES, SemanticToken, TextIndex,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Marker {
    pub(crate) name: String,
    pub(crate) position: Position,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MarkedSource {
    source: String,
    markers: Vec<Marker>,
}

impl MarkedSource {
    #[must_use]
    pub(crate) fn parse(source: &str) -> Self {
        let dedented = dedent_block(source);
        let mut clean = String::with_capacity(dedented.len());
        let mut markers = Vec::new();
        let mut rest = dedented.as_str();

        while let Some(marker_start) = rest.find('<') {
            clean.push_str(&rest[..marker_start]);
            rest = &rest[marker_start..];
            let Some(marker_end) = rest.find('>') else {
                clean.push('<');
                rest = &rest[1..];
                continue;
            };
            let marker_name = &rest[1..marker_end];
            if !is_marker_name(marker_name) {
                clean.push('<');
                rest = &rest[1..];
                continue;
            }
            markers.push(Marker {
                name: marker_name.to_owned(),
                position: offset_to_position_utf16(&clean, clean.len()),
            });
            rest = &rest[marker_end + 1..];
        }
        clean.push_str(rest);

        Self {
            source: clean,
            markers,
        }
    }

    #[must_use]
    pub(crate) fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    #[allow(dead_code)]
    pub(crate) fn marker(&self, name: &str) -> &Marker {
        self.markers
            .iter()
            .find(|marker| marker.name == name)
            .unwrap_or_else(|| panic!("missing marker '{name}'"))
    }

    #[allow(dead_code)]
    #[must_use]
    pub(crate) fn markers(&self) -> &[Marker] {
        &self.markers
    }
}

#[allow(dead_code)]
#[must_use]
pub(crate) fn render_definition(caret_position: Position, locations: &[Location]) -> String {
    if locations.is_empty() {
        return format!("{} unresolved", format_position(caret_position));
    }

    let mut targets = locations
        .iter()
        .map(|location| location.range.start)
        .collect::<Vec<_>>();
    targets.sort();

    let mut output = String::new();
    for target in targets {
        if !output.is_empty() {
            output.push('\n');
        }
        let _ = write!(
            output,
            "{} -> {} resolved",
            format_position(caret_position),
            format_position(target)
        );
    }
    output
}

#[allow(dead_code)]
#[must_use]
pub(crate) fn render_semantic_tokens(source: &str, tokens: &[SemanticToken]) -> String {
    if tokens.is_empty() {
        return "<none>".to_owned();
    }

    let text_index = TextIndex::new(source);
    let mut line = 0;
    let mut start = 0;
    let mut output = String::new();
    for token in tokens {
        line += token.delta_line;
        if token.delta_line == 0 {
            start += token.delta_start;
        } else {
            start = token.delta_start;
        }
        let end = start + token.length;
        let token_text = text_for_range(source, &text_index, line, start, end);
        let token_type = SEMANTIC_TOKEN_TYPE_NAMES
            .get(token.token_type as usize)
            .copied()
            .unwrap_or("<unknown>");
        let modifiers = render_token_modifiers(token.token_modifiers_bitset);

        if !output.is_empty() {
            output.push('\n');
        }
        let _ = write!(
            output,
            "{line}:{start} {end} kind={token_type:<13} modifiers={modifiers:<12} text={token_text}"
        );
    }
    output
}

#[allow(dead_code)]
#[must_use]
pub(crate) fn render_inlay_hints(hints: &[InlayHint]) -> String {
    if hints.is_empty() {
        return "<none>".to_owned();
    }

    let mut output = String::new();
    for hint in hints {
        if !output.is_empty() {
            output.push('\n');
        }
        let kind = match hint.kind {
            Some(InlayHintKind::Type) => "type",
            Some(InlayHintKind::Parameter) => "parameter",
            None => "none",
        };
        let _ = write!(
            output,
            "{}:{} kind={kind:<9} label={}",
            hint.position.line, hint.position.character, hint.label
        );
        if let Some(tooltip) = &hint.tooltip {
            let _ = write!(output, " tooltip={tooltip}");
        }
    }
    output
}

#[allow(dead_code)]
#[must_use]
pub(crate) fn render_completion(items: &[CompletionItem]) -> String {
    if items.is_empty() {
        return "<none>".to_owned();
    }
    let mut output = String::new();
    for item in items {
        if !output.is_empty() {
            output.push('\n');
        }
        let _ = write!(
            output,
            "{} kind={:?}",
            escape_completion_text(&item.label),
            item.kind
        );
        if let Some(detail) = &item.detail {
            let _ = write!(output, " detail={}", escape_completion_text(detail));
        }
        if let Some(edit) = &item.text_edit {
            let _ = write!(
                output,
                " edit={}:{}-{}:{}:{}",
                edit.range.start.line,
                edit.range.start.character,
                edit.range.end.line,
                edit.range.end.character,
                escape_completion_text(&edit.new_text)
            );
        } else if let Some(insert_text) = &item.insert_text {
            let _ = write!(output, " insert={}", escape_completion_text(insert_text));
        }
    }
    output
}

fn escape_completion_text(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

#[allow(dead_code)]
fn is_marker_name(name: &str) -> bool {
    name == "caret" || name == "target" || name.starts_with("caret:") || name.starts_with("target:")
}

#[allow(dead_code)]
fn format_position(position: Position) -> String {
    format!("{}:{}", position.line, position.character)
}

fn offset_to_position_utf16(source: &str, offset: usize) -> Position {
    let mut line = 0;
    let mut line_start = 0;
    for (idx, byte) in source.bytes().enumerate() {
        if idx >= offset {
            break;
        }
        if byte == b'\n' {
            line += 1;
            line_start = idx + 1;
        }
    }
    let character = source[line_start..offset]
        .chars()
        .map(char::len_utf16)
        .sum::<usize>();
    Position::new(line, character as u32)
}

fn text_for_range<'a>(
    source: &'a str,
    text_index: &TextIndex,
    line: u32,
    start: u32,
    end: u32,
) -> &'a str {
    let start = text_index.position_to_offset(source, Position::new(line, start));
    let end = text_index.position_to_offset(source, Position::new(line, end));
    source.get(start..end).unwrap_or("")
}

fn render_token_modifiers(bitset: u32) -> String {
    let modifiers = SEMANTIC_TOKEN_MODIFIER_NAMES
        .iter()
        .enumerate()
        .filter_map(|(idx, name)| (bitset & (1 << idx) != 0).then_some(*name))
        .collect::<Vec<_>>();
    if modifiers.is_empty() {
        "-".to_owned()
    } else {
        modifiers.join("|")
    }
}

pub(crate) fn dedent_block(source: &str) -> String {
    let mut lines = source.lines().collect::<Vec<_>>();
    while lines.first().is_some_and(|line| line.trim().is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    let indent = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.chars().take_while(|ch| *ch == ' ').count())
        .min()
        .unwrap_or(0);
    let mut result = String::new();
    for (idx, line) in lines.into_iter().enumerate() {
        if idx > 0 {
            result.push('\n');
        }
        result.push_str(line.get(indent..).unwrap_or(line));
    }
    result
}
