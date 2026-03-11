use crate::languages::engine::text_index::TextIndex;
use lsp_types::*;
use std::path::Path;
use std::sync::Arc;
use tolk_resolver::FileInfo;
use tolk_resolver::file_index::Span;
use tree_sitter::Point;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceLanguage {
    Tolk,
    Tasm,
    Fift,
    Toml,
    Unknown,
}

impl SourceLanguage {
    #[must_use]
    pub const fn is_self_contained(self) -> bool {
        matches!(self, Self::Tasm | Self::Fift | Self::Toml)
    }
}

#[must_use]
pub fn detect_language(uri: &Url) -> SourceLanguage {
    let ext = Path::new(uri.path())
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase());

    match ext.as_deref() {
        Some("tolk") => SourceLanguage::Tolk,
        Some("tasm") => SourceLanguage::Tasm,
        Some("fif") | Some("fift") => SourceLanguage::Fift,
        Some("toml") => SourceLanguage::Toml,
        _ => SourceLanguage::Unknown,
    }
}

pub trait SpanExt {
    fn start_position(&self, file: &Arc<FileInfo>) -> Position;
    fn end_position(&self, file: &Arc<FileInfo>) -> Position;
    fn range(&self, file: &Arc<FileInfo>) -> Range;
}

pub trait FileInfoExt {
    fn url(&self) -> Option<Url>;
    fn position_to_offset(&self, pos: Position) -> Option<usize>;
}

impl FileInfoExt for FileInfo {
    fn url(&self) -> Option<Url> {
        Url::from_file_path(self.path()).ok()
    }

    fn position_to_offset(&self, pos: Position) -> Option<usize> {
        let source = &self.source().source;
        let text_index = TextIndex::new(source);
        if pos.line as usize >= text_index.line_starts().len() {
            return None;
        }
        Some(text_index.position_to_offset(source, pos))
    }
}

impl SpanExt for Span {
    fn start_position(&self, file: &Arc<FileInfo>) -> Position {
        offset_to_pos(file, self.start())
    }

    fn end_position(&self, file: &Arc<FileInfo>) -> Position {
        offset_to_pos(file, self.end())
    }

    fn range(&self, file: &Arc<FileInfo>) -> Range {
        Range {
            start: self.start_position(file),
            end: self.end_position(file),
        }
    }
}

pub fn compute_offsets(text: &str) -> Vec<usize> {
    TextIndex::new(text).line_starts().to_vec()
}

pub fn offset_to_range(file: &Arc<FileInfo>, offset: usize) -> Range {
    let pos = offset_to_pos(file, offset);
    Range::new(pos, Position::new(pos.line, pos.character + 1))
}

pub fn get_byte_offset(text: &str, pos: Position) -> usize {
    TextIndex::new(text).position_to_offset(text, pos)
}

pub fn get_point(text: &str, pos: Position) -> Point {
    TextIndex::new(text).position_to_point(text, pos)
}

pub fn offset_to_pos(file: &Arc<FileInfo>, offset: usize) -> Position {
    let source = &file.source().source;
    TextIndex::new(source).offset_to_position(source, offset)
}

pub fn offset_to_lsp_pos(offset: usize, text: &str) -> Position {
    TextIndex::new(text).offset_to_position(text, offset)
}

pub fn offsets_to_lsp_range(start_offset: usize, end_offset: usize, text: &str) -> Range {
    Range::new(
        offset_to_lsp_pos(start_offset, text),
        offset_to_lsp_pos(end_offset, text),
    )
}

pub fn ranges_intersect(a: &Range, b: &Range) -> bool {
    let a_start = (a.start.line, a.start.character);
    let a_end = (a.end.line, a.end.character);
    let b_start = (b.start.line, b.start.character);
    let b_end = (b.end.line, b.end.character);

    a_start <= b_end && b_start <= a_end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_offsets_handles_crlf() {
        assert_eq!(compute_offsets("a\r\nb\n"), vec![0, 3, 5]);
    }

    #[test]
    fn get_byte_offset_handles_utf16_in_crlf_line() {
        let text = "ab\r\nc😀d";
        assert_eq!(get_byte_offset(text, Position::new(1, 3)), 9);
    }

    #[test]
    fn offset_to_lsp_pos_handles_crlf() {
        let text = "ab\r\ncd";
        assert_eq!(offset_to_lsp_pos(5, text), Position::new(1, 1));
    }
}
