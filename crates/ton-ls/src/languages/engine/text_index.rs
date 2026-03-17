use lsp_types::Position;
use tree_sitter::Point;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextIndex {
    line_starts: Vec<usize>,
}

impl TextIndex {
    #[must_use]
    pub fn new(text: &str) -> Self {
        Self {
            line_starts: build_line_starts(text),
        }
    }

    #[must_use]
    pub fn line_starts(&self) -> &[usize] {
        &self.line_starts
    }

    #[must_use]
    pub fn position_to_offset(&self, text: &str, position: Position) -> usize {
        let line = self.clamped_line(position.line as usize);
        let line_start = self.line_starts[line].min(text.len());
        if line_start >= text.len() {
            return line_start;
        }

        let line_end = line_end_byte(text, line_start);
        let target_utf16 = position.character as usize;

        let mut byte_offset = 0usize;
        let mut utf16_offset = 0usize;
        for ch in text[line_start..line_end].chars() {
            if utf16_offset >= target_utf16 {
                break;
            }
            byte_offset += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }

        line_start + byte_offset
    }

    #[must_use]
    pub fn position_to_point(&self, text: &str, position: Position) -> Point {
        let line = self.clamped_line(position.line as usize);
        let line_start = self.line_starts[line].min(text.len());
        let offset = self.position_to_offset(text, position);
        Point::new(line, offset.saturating_sub(line_start))
    }

    #[must_use]
    pub fn offset_to_position(&self, text: &str, offset: usize) -> Position {
        if text.is_empty() {
            return Position::new(0, 0);
        }

        let clamped_offset = offset.min(text.len());
        let line = self.line_for_offset(clamped_offset);
        let line_start = self.line_starts[line].min(text.len());
        let line_end = line_end_byte(text, line_start);
        let bounded_offset = clamped_offset.min(line_end);

        let mut utf16_offset = 0usize;
        for ch in text[line_start..bounded_offset].chars() {
            utf16_offset += ch.len_utf16();
        }

        Position::new(line as u32, utf16_offset as u32)
    }

    #[must_use]
    pub fn offset_to_point(&self, text: &str, offset: usize) -> Point {
        let clamped_offset = offset.min(text.len());
        let line = self.line_for_offset(clamped_offset);
        let line_start = self.line_starts[line].min(text.len());
        let line_end = line_end_byte(text, line_start);
        let bounded_offset = clamped_offset.min(line_end);
        Point::new(line, bounded_offset.saturating_sub(line_start))
    }

    fn clamped_line(&self, requested_line: usize) -> usize {
        requested_line.min(self.line_starts.len().saturating_sub(1))
    }

    fn line_for_offset(&self, offset: usize) -> usize {
        self.line_starts
            .binary_search(&offset)
            .unwrap_or_else(|idx| idx.saturating_sub(1))
            .min(self.line_starts.len().saturating_sub(1))
    }
}

fn build_line_starts(text: &str) -> Vec<usize> {
    let mut line_starts = vec![0];
    let bytes = text.as_bytes();

    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'\r' => {
                if bytes.get(index + 1) == Some(&b'\n') {
                    index += 2;
                } else {
                    index += 1;
                }
                line_starts.push(index);
            }
            b'\n' => {
                index += 1;
                line_starts.push(index);
            }
            _ => {
                index += 1;
            }
        }
    }

    line_starts
}

fn line_end_byte(text: &str, line_start: usize) -> usize {
    let bytes = text.as_bytes();
    let mut index = line_start;
    while index < bytes.len() {
        if matches!(bytes[index], b'\n' | b'\r') {
            return index;
        }
        index += 1;
    }
    bytes.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_line_starts_for_mixed_newlines() {
        let index = TextIndex::new("a\r\nb\nc\rd");
        assert_eq!(index.line_starts(), &[0, 3, 5, 7]);
    }

    #[test]
    fn converts_utf16_position_to_offset() {
        let text = "a😀b";
        let index = TextIndex::new(text);

        assert_eq!(index.position_to_offset(text, Position::new(0, 3)), 5);
    }

    #[test]
    fn converts_crlf_offset_roundtrip() {
        let text = "ab\r\ncd";
        let index = TextIndex::new(text);

        let offset = index.position_to_offset(text, Position::new(1, 1));
        assert_eq!(offset, 5);
        assert_eq!(index.offset_to_position(text, offset), Position::new(1, 1));
    }

    #[test]
    fn clamps_position_to_line_end_before_crlf() {
        let text = "a\r\nb";
        let index = TextIndex::new(text);

        assert_eq!(index.position_to_offset(text, Position::new(0, 100)), 1);
    }
}
