use crate::backend::Backend;
use crate::backend::utils::{SpanExt, ranges_intersect};
use crate::languages::tolk::diagnostics::convert_single_diagnostic;
use lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams, CodeActionResponse, Range,
    TextEdit, Url, WorkspaceEdit,
};
use std::collections::HashMap;
use std::time::Instant;
use tower_lsp::jsonrpc::Result as LspResult;

impl Backend {
    pub async fn handle_code_action(
        &self,
        params: CodeActionParams,
    ) -> LspResult<Option<CodeActionResponse>> {
        crate::profile!(self, "code_action");
        let now = Instant::now();
        let uri = params.text_document.uri;
        log::info!("Request: code_action for {uri}");

        let result = self.code_actions(&params.range, &uri);

        log::info!("Response: code_action took {:?}", now.elapsed());
        Ok(result)
    }

    fn code_actions(&self, range: &Range, uri: &Url) -> Option<Vec<CodeActionOrCommand>> {
        let analysis = self.analysis.get(uri)?;
        let path = uri.to_file_path().ok()?;
        let file_info = self.file_db.get_by_path(&path)?;

        let file_id = file_info.id();
        let mut actions = Vec::new();

        // find diagnostics for this file that have fixes
        for diag in &analysis.diagnostics {
            if diag.file_id != file_id {
                continue;
            }

            // check if the diagnostic range intersects with the requested range
            if let Some(first_annotation) = diag.annotations.first() {
                let diag_range = first_annotation.span.range(&file_info);
                if !ranges_intersect(&diag_range, range) {
                    continue;
                }
            }

            // convert fixes to code actions
            for (fix_idx, fix) in diag.fixes.iter().enumerate() {
                let mut edits = Vec::with_capacity(fix.edits.len());

                for edit in &fix.edits {
                    let edit_range = edit.span.range(&file_info);
                    edits.push(TextEdit::new(edit_range, edit.replacement.clone()));
                }

                let Some(diagnostic) = convert_single_diagnostic(diag, &file_info) else {
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
                    is_preferred: Some(fix_idx == 0), // first fix is preferred
                    disabled: None,
                });

                actions.push(action);
            }
        }

        Some(actions)
    }
}
