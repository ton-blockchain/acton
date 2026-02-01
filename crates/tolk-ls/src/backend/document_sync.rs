use crate::backend::Backend;
use crate::backend::utils::*;
use lsp_types::*;
use tower_lsp::lsp_types::Url;
use tree_sitter::{InputEdit, Point, Range as TSRange};

impl Backend {
    pub fn update_document(&self, uri: &Url, text: String) {
        self.documents.insert(uri.clone(), text);
    }

    pub async fn handle_did_change(&self, params: DidChangeTextDocumentParams) {
        let now = std::time::Instant::now();
        let uri = params.text_document.uri;
        log::info!("Notification: did_change for {}", &uri);

        let path = uri.to_file_path().unwrap();
        let mut text = self
            .documents
            .get(&uri)
            .map(|d| d.clone())
            .unwrap_or_default();
        let mut old_tree = self
            .file_db
            .get_by_path(&path)
            .map(|f| f.source().tree.clone());
        let mut changes_ranges = Vec::new();

        for change in params.content_changes {
            if let Some(range) = change.range {
                let start_byte = get_byte_offset(&text, range.start);
                let old_end_byte = get_byte_offset(&text, range.end);
                let start_position = get_point(&text, range.start);
                let old_end_position = get_point(&text, range.end);

                text.replace_range(start_byte..old_end_byte, &change.text);

                let new_end_byte = start_byte + change.text.len();
                let new_end_position = get_point(&text, offset_to_lsp_pos(new_end_byte, &text));

                if let Some(ref mut tree) = old_tree {
                    tree.edit(&InputEdit {
                        start_byte,
                        old_end_byte,
                        new_end_byte,
                        start_position,
                        old_end_position,
                        new_end_position,
                    });
                }

                let diff = (new_end_byte as isize) - (old_end_byte as isize);
                changes_ranges
                    .retain(|r: &TSRange| r.end_byte <= start_byte || r.start_byte >= old_end_byte);

                for r in changes_ranges.iter_mut() {
                    if r.start_byte >= old_end_byte {
                        r.start_byte = (r.start_byte as isize + diff) as usize;
                        r.end_byte = (r.end_byte as isize + diff) as usize;
                    }
                }

                changes_ranges.push(TSRange {
                    start_byte,
                    end_byte: new_end_byte,
                    start_point: start_position,
                    end_point: new_end_position,
                });
            } else {
                text = change.text;
                old_tree = None;
                changes_ranges.clear();
                changes_ranges.push(TSRange {
                    start_byte: 0,
                    end_byte: text.len(),
                    start_point: Point::new(0, 0),
                    end_point: get_point(&text, offset_to_lsp_pos(text.len(), &text)),
                });
            }
        }

        self.update_document(&uri, text.clone());
        self.analyze_incremental(uri, old_tree).await;

        log::info!("Notification: did_change took {:?}", now.elapsed());
    }
}
