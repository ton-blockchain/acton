use crate::logging;
use crate::types::{Position, Range};
use std::ops::Range as StdRange;
use tree_sitter::{InputEdit, Node, Point};

#[derive(Clone, Debug)]
pub struct TextIndex {
    line_starts: Vec<usize>,
}

impl TextIndex {
    #[must_use]
    pub fn new(text: &str) -> Self {
        let mut line_starts = vec![0];
        for (idx, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(idx + 1);
            }
        }
        Self { line_starts }
    }

    #[must_use]
    pub fn line_starts(&self) -> &[usize] {
        &self.line_starts
    }

    #[must_use]
    pub fn position_to_offset(&self, text: &str, position: Position) -> usize {
        let line = position.line as usize;
        let Some(&line_start) = self.line_starts.get(line) else {
            return text.len();
        };
        let line_end = self
            .line_starts
            .get(line + 1)
            .copied()
            .unwrap_or(text.len());
        let line_text = &text[line_start..line_end];
        let target_units = position.character as usize;
        let mut utf16_units = 0;

        for (byte_offset, ch) in line_text.char_indices() {
            let next_units = utf16_units + ch.len_utf16();
            if next_units > target_units {
                return line_start + byte_offset;
            }
            utf16_units = next_units;
        }

        line_end
    }

    #[must_use]
    pub fn offset_to_position(&self, text: &str, offset: usize) -> Position {
        let offset = offset.min(text.len());
        let line = match self.line_starts.binary_search(&offset) {
            Ok(line) => line,
            Err(next_line) => next_line.saturating_sub(1),
        };
        let line_start = self.line_starts[line];
        let character = text[line_start..offset]
            .chars()
            .map(char::len_utf16)
            .sum::<usize>();
        Position::new(line as u32, character as u32)
    }

    #[must_use]
    pub fn position_to_point(&self, text: &str, position: Position) -> Point {
        let offset = self.position_to_offset(text, position);
        let line_start = self
            .line_starts
            .get(position.line as usize)
            .copied()
            .unwrap_or(offset);
        Point::new(position.line as usize, offset.saturating_sub(line_start))
    }

    #[must_use]
    pub fn range_for_offsets(&self, text: &str, start_offset: usize, end_offset: usize) -> Range {
        Range::new(
            self.offset_to_position(text, start_offset),
            self.offset_to_position(text, end_offset),
        )
    }

    #[must_use]
    pub fn range_of_node(&self, text: &str, node: Node<'_>) -> Range {
        self.range_for_offsets(text, node.start_byte(), node.end_byte())
    }

    pub(crate) fn apply_edit(
        &self,
        text: &mut String,
        range: Range,
        new_text: &str,
    ) -> anyhow::Result<InputEdit> {
        let byte_range = self.byte_range_for_range(text, range)?;
        let start_position = self.point_for_offset(text, byte_range.start);
        let old_end_position = self.point_for_offset(text, byte_range.end);

        let start_byte = byte_range.start;
        let old_end_byte = byte_range.end;
        let new_end_byte = start_byte + new_text.len();
        text.replace_range(byte_range, new_text);
        let updated_index = Self::new(text);
        let new_end_position = updated_index.point_for_offset(text, new_end_byte);

        tracing::trace!(
            target: logging::EDIT_TARGET,
            operation = "edit.input_edit",
            start_line = range.start.line,
            start_character = range.start.character,
            end_line = range.end.line,
            end_character = range.end.character,
            new_text_len = new_text.len(),
            start_byte,
            old_end_byte,
            new_end_byte,
            start_row = start_position.row,
            start_column = start_position.column,
            old_end_row = old_end_position.row,
            old_end_column = old_end_position.column,
            new_end_row = new_end_position.row,
            new_end_column = new_end_position.column,
            "built tree-sitter input edit"
        );

        Ok(InputEdit {
            start_byte,
            old_end_byte,
            new_end_byte,
            start_position,
            old_end_position,
            new_end_position,
        })
    }

    fn byte_range_for_range(&self, text: &str, range: Range) -> anyhow::Result<StdRange<usize>> {
        let start = self.position_to_offset(text, range.start);
        let end = self.position_to_offset(text, range.end);
        if start > end {
            anyhow::bail!(
                "edit range start {}:{} is after end {}:{}",
                range.start.line,
                range.start.character,
                range.end.line,
                range.end.character
            );
        }
        Ok(start..end)
    }

    fn point_for_offset(&self, text: &str, offset: usize) -> Point {
        let offset = offset.min(text.len());
        let line = match self.line_starts.binary_search(&offset) {
            Ok(line) => line,
            Err(next_line) => next_line.saturating_sub(1),
        };
        let line_start = self.line_starts[line];
        Point::new(line, offset.saturating_sub(line_start))
    }
}
