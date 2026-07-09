use std::fmt::Write as _;
use ton_language_server_core::{Location, Position};

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
                clean.push_str(&rest[..=marker_end]);
                rest = &rest[marker_end + 1..];
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
    targets.dedup();

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

fn dedent_block(source: &str) -> String {
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
