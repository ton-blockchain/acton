use anyhow::{anyhow, bail};
use lsp_types::Position;

#[derive(Clone, Debug)]
pub(crate) struct ParsedSource {
    pub(crate) source: String,
    pub(crate) carets: Vec<Caret>,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub(crate) struct Caret {
    pub(crate) name: Option<String>,
    pub(crate) offset: usize,
    pub(crate) position: Position,
}

#[derive(Clone, Debug)]
struct RawCaret {
    name: Option<String>,
    offset: usize,
}

pub(crate) fn parse_source(source: &str) -> anyhow::Result<ParsedSource> {
    let dedented = dedent_block(source);
    let (clean_source, raw_carets) = strip_carets(&dedented)?;
    let carets = raw_carets
        .into_iter()
        .map(|raw| Caret {
            name: raw.name,
            offset: raw.offset,
            position: offset_to_position_utf16(&clean_source, raw.offset),
        })
        .collect();

    Ok(ParsedSource {
        source: clean_source,
        carets,
    })
}

pub(crate) fn normalize_case_name(test_fn_name: &str, expected_prefix: &str) -> String {
    let short_name = test_fn_name
        .rsplit("::")
        .next()
        .unwrap_or(test_fn_name)
        .to_owned();
    let stripped = short_name
        .strip_prefix(expected_prefix)
        .unwrap_or(&short_name)
        .to_owned();
    let normalized = stripped.replace('_', " ");
    if normalized.is_empty() {
        return "unnamed".to_owned();
    }
    normalized
}

fn strip_carets(input: &str) -> anyhow::Result<(String, Vec<RawCaret>)> {
    let mut clean = String::with_capacity(input.len());
    let mut carets = Vec::new();
    let mut cursor = 0usize;

    while let Some(rel_start) = input[cursor..].find("<caret") {
        let start = cursor + rel_start;
        clean.push_str(&input[cursor..start]);

        let rel_end = input[start..]
            .find('>')
            .ok_or_else(|| anyhow!("invalid caret marker: missing closing `>`"))?;
        let end = start + rel_end;
        let marker = &input[start + 1..end];

        let name = if marker == "caret" {
            None
        } else if let Some(name) = marker.strip_prefix("caret:") {
            if name.is_empty() {
                bail!("invalid caret marker: name after `caret:` cannot be empty");
            }
            Some(name.to_owned())
        } else {
            bail!("invalid caret marker `{marker}`");
        };

        carets.push(RawCaret {
            name,
            offset: clean.len(),
        });
        cursor = end + 1;
    }

    clean.push_str(&input[cursor..]);
    Ok((clean, carets))
}

fn dedent_block(input: &str) -> String {
    let without_leading_newline = input.strip_prefix('\n').unwrap_or(input);
    let mut lines = without_leading_newline.lines().collect::<Vec<_>>();
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }

    let indent = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.chars().take_while(|ch| ch.is_whitespace()).count())
        .min()
        .unwrap_or(0);

    let total = lines.len();
    let mut out = String::new();
    for (idx, line) in lines.into_iter().enumerate() {
        let dedented = line
            .char_indices()
            .nth(indent)
            .map(|(byte_idx, _)| &line[byte_idx..])
            .unwrap_or("");
        out.push_str(dedented);
        if idx + 1 < total {
            out.push('\n');
        }
    }
    out
}

fn offset_to_position_utf16(text: &str, offset: usize) -> Position {
    let mut line = 0u32;
    let mut character = 0u32;

    for (byte_idx, ch) in text.char_indices() {
        if byte_idx >= offset {
            break;
        }

        if ch == '\n' {
            line += 1;
            character = 0;
        } else {
            character += ch.len_utf16() as u32;
        }
    }

    Position::new(line, character)
}
