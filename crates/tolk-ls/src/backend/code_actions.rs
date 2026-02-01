use lsp_types::*;
use std::collections::HashMap;
use std::time::Instant;
use tower_lsp::jsonrpc::Result as LspResult;
use crate::backend::Backend;
use crate::backend::utils::{offset_to_range, ranges_intersect};
use crate::backend::diagnostics::convert_single_diagnostic;

impl Backend {
    pub async fn handle_code_action(&self, params: CodeActionParams) -> LspResult<Option<CodeActionResponse>> {
        let now = Instant::now();
        let uri = params.text_document.uri;
        log::info!("Request: code_action for {}", uri);

        let result = if let Some(analysis) = self.analysis.get(&uri) {
            if let Ok(path) = uri.to_file_path() {
                if let Some(file_info) = self.file_db.get_by_path(&path) {
                    let file_id = file_info.id();
                    let mut actions = Vec::new();

                    // Find diagnostics for this file that have fixes
                    for diag in &analysis.diagnostics {
                        if diag.file_id != file_id {
                            continue;
                        }

                        // Check if the diagnostic range intersects with the requested range
                        if let Some(first_annotation) = diag.annotations.first() {
                            let diag_range =
                                offset_to_range(&file_info, first_annotation.span.start());
                            if !ranges_intersect(&diag_range, &params.range) {
                                continue;
                            }
                        }

                        // Convert fixes to code actions
                        for (fix_idx, fix) in diag.fixes.iter().enumerate() {
                            let mut edits = Vec::new();
                            for edit in &fix.edits {
                                let start_range = offset_to_range(&file_info, edit.span.start());
                                let end_range = offset_to_range(&file_info, edit.span.end());
                                let edit_range = Range::new(start_range.start, end_range.start);

                                edits.push(TextEdit::new(edit_range, edit.replacement.clone()));
                            }

                            let Some(diagnostic) = convert_single_diagnostic(diag, &file_info)
                            else {
                                continue;
                            };

                            let action = CodeActionOrCommand::CodeAction(CodeAction {
                                title: fix.message.clone(),
                                kind: Some(CodeActionKind::QUICKFIX),
                                diagnostics: Some(vec![diagnostic]),
                                edit: Some(WorkspaceEdit {
                                    changes: Some(HashMap::from([(uri.clone(), edits)])),
                                    document_changes: None,
                                    change_annotations: None,
                                }),
                                command: None,
                                data: None,
                                is_preferred: Some(fix_idx == 0), // First fix is preferred
                                disabled: None,
                            });

                            actions.push(action);
                        }
                    }

                    Some(actions)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        log::info!("Response: code_action took {:?}", now.elapsed());
        Ok(result)
    }
}
