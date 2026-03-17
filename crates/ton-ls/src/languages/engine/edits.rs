use crate::languages::engine::text_index::TextIndex;
use lsp_types::TextDocumentContentChangeEvent;
use tree_sitter::InputEdit;

#[derive(Debug, Clone)]
pub struct AppliedTextChanges {
    pub text: String,
    pub incremental_edits: Option<Vec<InputEdit>>,
}

#[must_use]
pub fn apply_lsp_changes(
    initial_text: &str,
    changes: &[TextDocumentContentChangeEvent],
) -> AppliedTextChanges {
    let mut text = initial_text.to_owned();
    let mut text_index = TextIndex::new(&text);
    let mut incremental_edits = Vec::with_capacity(changes.len());
    let mut can_apply_incremental = true;

    for change in changes {
        if let Some(range) = change.range {
            let start_byte = text_index.position_to_offset(&text, range.start);
            let old_end_byte = text_index.position_to_offset(&text, range.end);
            let start_position = text_index.position_to_point(&text, range.start);
            let old_end_position = text_index.position_to_point(&text, range.end);

            text.replace_range(start_byte..old_end_byte, &change.text);
            text_index = TextIndex::new(&text);

            if can_apply_incremental {
                let new_end_byte = start_byte + change.text.len();
                let new_end_position = text_index.offset_to_point(&text, new_end_byte);

                incremental_edits.push(InputEdit {
                    start_byte,
                    old_end_byte,
                    new_end_byte,
                    start_position,
                    old_end_position,
                    new_end_position,
                });
            }
        } else {
            text = change.text.clone();
            text_index = TextIndex::new(&text);
            can_apply_incremental = false;
            incremental_edits.clear();
        }
    }

    AppliedTextChanges {
        text,
        incremental_edits: can_apply_incremental.then_some(incremental_edits),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::{Position, Range};

    fn pos(line: u32, character: u32) -> Position {
        Position { line, character }
    }

    fn range(start_line: u32, start_character: u32, end_line: u32, end_character: u32) -> Range {
        Range {
            start: pos(start_line, start_character),
            end: pos(end_line, end_character),
        }
    }

    fn change(range: Option<Range>, text: &str) -> TextDocumentContentChangeEvent {
        TextDocumentContentChangeEvent {
            range,
            range_length: None,
            text: text.to_string(),
        }
    }

    #[test]
    fn applies_multiple_incremental_changes() {
        let initial = "hello world";
        let changes = vec![
            change(Some(range(0, 0, 0, 5)), "hi"),
            change(Some(range(0, 2, 0, 2)), " beautiful"),
        ];

        let applied = apply_lsp_changes(initial, &changes);
        assert_eq!(applied.text, "hi beautiful world");

        let edits = applied
            .incremental_edits
            .expect("incremental edits should be available");
        assert_eq!(edits.len(), 2);
        assert_eq!(edits[0].start_byte, 0);
        assert_eq!(edits[0].old_end_byte, 5);
        assert_eq!(edits[0].new_end_byte, 2);
    }

    #[test]
    fn uses_utf16_offsets_when_building_input_edit() {
        let initial = "a😀b";
        let changes = vec![change(Some(range(0, 3, 0, 4)), "c")];

        let applied = apply_lsp_changes(initial, &changes);
        assert_eq!(applied.text, "a😀c");

        let edits = applied
            .incremental_edits
            .expect("incremental edits should be available");
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].start_byte, 5);
        assert_eq!(edits[0].old_end_byte, 6);
        assert_eq!(edits[0].new_end_byte, 6);
    }

    #[test]
    fn handles_crlf_ranges_when_building_input_edit() {
        let initial = "ab\r\ncd\r\n";
        let changes = vec![change(Some(range(1, 1, 1, 2)), "D")];

        let applied = apply_lsp_changes(initial, &changes);
        assert_eq!(applied.text, "ab\r\ncD\r\n");

        let edits = applied
            .incremental_edits
            .expect("incremental edits should be available");
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].start_byte, 5);
        assert_eq!(edits[0].old_end_byte, 6);
        assert_eq!(edits[0].new_end_byte, 6);
    }

    #[test]
    fn full_replace_disables_incremental_edits() {
        let initial = "hello";
        let changes = vec![change(None, "new content")];

        let applied = apply_lsp_changes(initial, &changes);
        assert_eq!(applied.text, "new content");
        assert!(applied.incremental_edits.is_none());
    }
}
