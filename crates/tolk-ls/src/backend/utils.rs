use lsp_types::*;
use std::sync::Arc;
use tolk_resolver::FileInfo;
use tolk_resolver::file_index::Span;
use tree_sitter::Point;

pub trait SpanExt {
    fn start_position(&self, file: &Arc<FileInfo>) -> Position;
    fn end_position(&self, file: &Arc<FileInfo>) -> Position;
    fn start_range(&self, file: &Arc<FileInfo>) -> Range;
}

pub trait FileInfoExt {
    fn url(&self) -> Option<Url>;
}

impl FileInfoExt for FileInfo {
    fn url(&self) -> Option<Url> {
        Url::from_file_path(self.path()).ok()
    }
}

impl SpanExt for Span {
    fn start_position(&self, file: &Arc<FileInfo>) -> Position {
        offset_to_pos(file, self.start())
    }

    fn end_position(&self, file: &Arc<FileInfo>) -> Position {
        offset_to_pos(file, self.end())
    }

    fn start_range(&self, file: &Arc<FileInfo>) -> Range {
        offset_to_range(file, self.start())
    }
}

pub fn compute_offsets(text: &str) -> Vec<usize> {
    let mut offsets = vec![0];
    let mut last_offset = 0;
    for line in text.lines() {
        last_offset += line.len() + 1;
        offsets.push(last_offset);
    }
    offsets
}

pub fn offset_to_range(file: &Arc<FileInfo>, offset: usize) -> Range {
    let pos = offset_to_pos(file, offset);
    Range::new(pos, Position::new(pos.line, pos.character + 1))
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

pub fn offset_to_pos(file: &Arc<FileInfo>, offset: usize) -> Position {
    let line_offsets = file.line_offsets();
    let source = file.source().source.clone();

    offset_to_pos_internal(line_offsets, &source, offset)
}

fn offset_to_pos_internal(line_offsets: &[usize], source: &str, offset: usize) -> Position {
    let line = line_offsets
        .binary_search(&offset)
        .unwrap_or_else(|idx| idx.saturating_sub(1));

    let line_start_offset = line_offsets[line];
    let col_byte_offset = offset - line_start_offset;

    let line_content = &source[line_start_offset..];
    let mut utf16_count = 0;
    let mut byte_count = 0;
    for c in line_content.chars() {
        if byte_count >= col_byte_offset {
            break;
        }
        byte_count += c.len_utf8();
        utf16_count += c.len_utf16();
    }

    Position::new(line as u32, utf16_count as u32)
}

pub fn offset_to_lsp_pos(offset: usize, text: &str) -> Position {
    let offsets = compute_offsets(text);
    offset_to_pos_internal(&offsets, text, offset)
}

pub fn ranges_intersect(a: &Range, b: &Range) -> bool {
    let a_start = (a.start.line, a.start.character);
    let a_end = (a.end.line, a.end.character);
    let b_start = (b.start.line, b.start.character);
    let b_end = (b.end.line, b.end.character);

    a_start <= b_end && b_start <= a_end
}
