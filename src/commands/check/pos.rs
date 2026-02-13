use tree_sitter::Point;

pub(super) fn byte_offset_from_point(point: &Point, source: &str) -> usize {
    let lines = source.lines().collect::<Vec<_>>();
    let mut offset = 0;

    for i in 0..point.row {
        if i < lines.len() {
            offset += lines[i].len() + 1; // +1 for newline
        }
    }

    if point.row < lines.len() {
        offset += point.column;
    }

    offset
}

pub(super) fn byte_to_char_index(s: &str, byte_index: usize) -> usize {
    s.char_indices()
        .nth(byte_index)
        .map(|(i, _)| i)
        .unwrap_or(byte_index)
}

pub(super) fn byte_to_line_col(source: &str, byte_offset: usize) -> Option<(u32, u32)> {
    let mut line = 0u32;
    let mut col = 0u32;
    let mut current_byte = 0usize;

    for (i, ch) in source.char_indices() {
        if i >= byte_offset {
            return Some((line, col));
        }

        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
        current_byte = i;
    }

    // If we reach the end, return the last position
    if current_byte < byte_offset && byte_offset <= source.len() {
        Some((line, col + (byte_offset - current_byte) as u32))
    } else {
        None
    }
}
