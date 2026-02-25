pub(super) fn has_safety_comment_above(
    source: &str,
    line_offsets: &[usize],
    anchor_start: usize,
) -> bool {
    let anchor_line = offset_to_line(line_offsets, anchor_start);
    if anchor_line == 0 {
        return false;
    }

    let mut line_idx = anchor_line as isize - 1;
    while line_idx >= 0 {
        let Some(line) = line_text(source, line_offsets, line_idx as usize) else {
            break;
        };
        let trimmed = line.trim_start();

        if trimmed.is_empty() || !trimmed.starts_with("//") {
            break;
        }

        if contains_safety_word(trimmed) {
            return true;
        }

        line_idx -= 1;
    }

    false
}

fn line_text<'a>(source: &'a str, line_offsets: &[usize], line_idx: usize) -> Option<&'a str> {
    let start = *line_offsets.get(line_idx)?;
    let end = line_offsets
        .get(line_idx + 1)
        .copied()
        .unwrap_or(source.len());
    source.get(start..end)
}

fn offset_to_line(line_offsets: &[usize], offset: usize) -> usize {
    match line_offsets.binary_search(&offset) {
        Ok(line) => line,
        Err(0) => 0,
        Err(next_line) => next_line - 1,
    }
}

fn contains_safety_word(text: &str) -> bool {
    text.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .any(|token| token.eq_ignore_ascii_case("safety"))
}
