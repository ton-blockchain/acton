use lsp_types::{
    FoldingRange, GotoDefinitionResponse, Hover, HoverContents, LanguageString, Location,
    MarkedString, Position, SemanticToken, SemanticTokensLegend,
};

use crate::self_contained::harness::lsp::slice_line_utf16;

pub(crate) fn render_resolve(
    caret_position: Position,
    response: Option<GotoDefinitionResponse>,
) -> Vec<String> {
    let caret = format_position(caret_position);
    let mut targets = collect_resolve_targets(response);

    if targets.is_empty() {
        return vec![format!("{caret} unresolved")];
    }

    targets.sort_by_key(|pos| (pos.line, pos.character));
    targets.dedup_by_key(|pos| (pos.line, pos.character));

    targets
        .into_iter()
        .map(|target| format!("{caret} -> {} resolved", format_position(target)))
        .collect()
}

pub(crate) fn render_references(
    caret_position: Position,
    response: Option<Vec<Location>>,
) -> String {
    let caret = format_position(caret_position);
    let Some(locations) = response else {
        return format!("{caret} refs=unresolved");
    };

    let mut positions = locations
        .into_iter()
        .map(|location| location.range.start)
        .collect::<Vec<_>>();
    positions.sort_by_key(|pos| (pos.line, pos.character));
    positions.dedup_by_key(|pos| (pos.line, pos.character));

    if positions.is_empty() {
        return format!("{caret} refs=[]");
    }

    let rendered = positions
        .into_iter()
        .map(format_position)
        .collect::<Vec<_>>()
        .join(", ");
    format!("{caret} refs=[{rendered}]")
}

pub(crate) fn render_hover(response: Option<Hover>) -> String {
    let Some(hover) = response else {
        return "<none>".to_owned();
    };

    render_hover_contents(&hover.contents)
}

pub(crate) fn render_folding_ranges(response: Option<Vec<FoldingRange>>) -> String {
    let Some(mut ranges) = response else {
        return "<none>".to_owned();
    };
    if ranges.is_empty() {
        return "<none>".to_owned();
    }

    ranges.sort_by_key(|range| (range.start_line, range.end_line));
    let parts = ranges
        .into_iter()
        .map(|range| format!("[{}, {}]", range.start_line, range.end_line))
        .collect::<Vec<_>>();
    parts.join(", ")
}

pub(crate) fn render_semantic_tokens(
    source: &str,
    tokens: &[SemanticToken],
    legend: &SemanticTokensLegend,
) -> Vec<String> {
    let mut line = 0u32;
    let mut start = 0u32;
    let mut rows = Vec::with_capacity(tokens.len());

    for token in tokens {
        line += token.delta_line;
        if token.delta_line == 0 {
            start += token.delta_start;
        } else {
            start = token.delta_start;
        }
        let end = start + token.length;

        let token_type = legend
            .token_types
            .get(token.token_type as usize)
            .map(|tt| tt.as_str().to_owned())
            .unwrap_or_else(|| format!("unknown#{}", token.token_type));

        let text =
            slice_line_utf16(source, line, start, end).unwrap_or_else(|| "<invalid>".to_owned());
        let modifiers = collect_modifiers(token.token_modifiers_bitset, legend);
        rows.push((line, start, end, token_type, text, modifiers));
    }

    let start_width = rows
        .iter()
        .map(|(_, start, _, _, _, _)| start.to_string().len())
        .max()
        .unwrap_or(1);
    let end_width = rows
        .iter()
        .map(|(_, _, end, _, _, _)| end.to_string().len())
        .max()
        .unwrap_or(1)
        .max(3);
    let kind_width = rows
        .iter()
        .map(|(_, _, _, kind, _, _)| kind.len())
        .max()
        .unwrap_or(1);

    rows.into_iter()
        .map(|(line, start, end, kind, text, modifiers)| {
            let modifiers_part = if modifiers.is_empty() {
                String::new()
            } else {
                format!(" mods={}", modifiers.join(","))
            };
            format!(
                "{line}:{start:<start_width$}{end:>end_width$} kind={kind:<kind_width$} text={text}{modifiers_part}"
            )
        })
        .collect()
}

fn collect_resolve_targets(response: Option<GotoDefinitionResponse>) -> Vec<Position> {
    match response {
        Some(GotoDefinitionResponse::Scalar(location)) => vec![location.range.start],
        Some(GotoDefinitionResponse::Array(locations)) => locations
            .into_iter()
            .map(|location| location.range.start)
            .collect(),
        Some(GotoDefinitionResponse::Link(links)) => links
            .into_iter()
            .map(|link| link.target_range.start)
            .collect(),
        None => Vec::new(),
    }
}

fn collect_modifiers(bitset: u32, legend: &SemanticTokensLegend) -> Vec<String> {
    let mut result = Vec::new();
    for (idx, modifier) in legend.token_modifiers.iter().enumerate() {
        if let Some(mask) = 1u32.checked_shl(idx as u32)
            && bitset & mask != 0
        {
            result.push(modifier.as_str().to_owned());
        }
    }
    result
}

fn format_position(position: Position) -> String {
    format!("{}:{}", position.line, position.character)
}

fn render_hover_contents(contents: &HoverContents) -> String {
    let rendered = match contents {
        HoverContents::Markup(markup) => normalize_hover_markdown(&markup.value),
        HoverContents::Scalar(marked) => render_marked_string(marked),
        HoverContents::Array(items) => {
            let mut blocks = Vec::new();
            for item in items {
                let block = render_marked_string(item);
                if !block.is_empty() {
                    blocks.push(block);
                }
            }
            blocks.join("\n\n")
        }
    };

    if rendered.is_empty() {
        return "<empty>".to_owned();
    }
    rendered
}

fn render_marked_string(marked: &MarkedString) -> String {
    match marked {
        MarkedString::String(value) => normalize_hover_markdown(value),
        MarkedString::LanguageString(value) => render_language_string(value),
    }
}

fn render_language_string(value: &LanguageString) -> String {
    let body = normalize_hover_markdown(&value.value);
    if body.is_empty() {
        return String::new();
    }
    format!("```{}\n{body}\n```", value.language)
}

fn normalize_hover_markdown(text: &str) -> String {
    text.replace('\r', "").trim().to_owned()
}
