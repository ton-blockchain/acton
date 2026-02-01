use lsp_types::*;
use tree_sitter::Point;

pub fn compute_offsets(text: &str) -> Vec<usize> {
    let mut offsets = vec![0];
    let mut last_offset = 0;
    for line in text.lines() {
        last_offset += line.len() + 1;
        offsets.push(last_offset);
    }
    offsets
}

pub fn offset_to_range(line_offsets: &[usize], _source: &str, offset: usize) -> Range {
    let line = line_offsets
        .binary_search(&offset)
        .unwrap_or_else(|idx| idx.saturating_sub(1));
    let character = offset - line_offsets[line];
    Range::new(
        Position::new(line as u32, character as u32),
        Position::new(line as u32, (character + 1) as u32),
    )
}

pub fn get_byte_offset(text: &str, pos: Position) -> usize {
    let mut byte_offset = 0;
    for (i, line) in text.split('\n').enumerate() {
        if i == pos.line as usize {
            let mut utf16_count = 0;
            for c in line.chars() {
                if utf16_count >= pos.character as usize {
                    break;
                }
                byte_offset += c.len_utf8();
                utf16_count += c.len_utf16();
            }
            return byte_offset;
        }
        byte_offset += line.len() + 1;
    }
    byte_offset
}

pub fn get_point(text: &str, pos: Position) -> Point {
    let mut byte_col = 0;
    for (i, line) in text.split('\n').enumerate() {
        if i == pos.line as usize {
            let mut utf16_count = 0;
            for c in line.chars() {
                if utf16_count >= pos.character as usize {
                    break;
                }
                byte_col += c.len_utf8();
                utf16_count += c.len_utf16();
            }
            break;
        }
    }
    Point::new(pos.line as usize, byte_col)
}

pub fn offset_to_lsp_pos(offset: usize, text: &str) -> Position {
    let mut current_offset = 0;
    for (i, line) in text.split('\n').enumerate() {
        if current_offset + line.len() >= offset {
            let mut utf16_count = 0;
            let mut byte_in_line = 0;
            for c in line.chars() {
                if current_offset + byte_in_line >= offset {
                    break;
                }
                byte_in_line += c.len_utf8();
                utf16_count += c.len_utf16();
            }
            return Position::new(i as u32, utf16_count as u32);
        }
        current_offset += line.len() + 1;
    }
    Position::new(0, 0)
}

pub fn ranges_intersect(a: &Range, b: &Range) -> bool {
    let a_start = (a.start.line, a.start.character);
    let a_end = (a.end.line, a.end.character);
    let b_start = (b.start.line, b.start.character);
    let b_end = (b.end.line, b.end.character);

    a_start <= b_end && b_start <= a_end
}
