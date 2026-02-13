use crate::commands::check::pos;
use tolk_linter::diagnostic::{Applicability, Diagnostic, Severity};
use tolk_resolver::{FileDb, Span};

pub(super) fn diagnostic_to_json(diag: &Diagnostic, file_db: &FileDb) -> serde_json::Value {
    let file_info = file_db
        .get_by_id(diag.file_id)
        .expect("File info should exist for diagnostic");
    let file_path = file_info.index().path.to_string_lossy().to_string();
    let source = file_info.source().source.as_ref();

    let severity = match diag.severity {
        Severity::Info => "info",
        Severity::Warning => "warning",
        Severity::Error => "error",
        Severity::Fatal => "error",
        Severity::Help => "info",
    };

    let mut annotations_json = Vec::new();
    for annotation in &diag.annotations {
        if let Some(range) = create_range_json(source, &annotation.span) {
            annotations_json.push(serde_json::json!({
                "range": range,
                "message": annotation.message,
                "is_primary": annotation.is_primary
            }));
        }
    }

    let mut fixes_json = Vec::new();
    for fix in &diag.fixes {
        let mut edits_json = Vec::new();
        for edit in &fix.edits {
            let edit_file_id = edit.file_id;
            let edit_source = file_db
                .get_by_id(edit_file_id)
                .map(|info| info.source().source.clone())
                .unwrap_or_else(|| source.into());
            if let Some(range) = create_range_json(edit_source.as_ref(), &edit.span) {
                edits_json.push(serde_json::json!({
                    "range": range,
                    "newText": &edit.replacement,
                    "file": file_db
                        .get_by_id(edit_file_id)
                        .map(|info| info.index().path.to_string_lossy().to_string())
                        .unwrap_or_else(|| file_path.clone())
                }));
            }
        }
        let applicability = match fix.applicability {
            Applicability::Auto => "auto",
            Applicability::Manual => "manual",
        };
        fixes_json.push(serde_json::json!({
            "message": &fix.message,
            "edits": edits_json,
            "applicability": applicability
        }));
    }

    serde_json::json!({
        "file": file_path,
        "severity": severity,
        "name": &diag.name,
        "code": &diag.code,
        "message": &diag.message,
        "annotations": annotations_json,
        "fixes": fixes_json,
        "source": "tolk"
    })
}

fn create_range_json(source: &str, span: &Span) -> Option<serde_json::Value> {
    if let (Some((start_line, start_col)), Some((end_line, end_col))) = (
        pos::byte_to_line_col(source, span.start as usize),
        pos::byte_to_line_col(source, span.end as usize),
    ) {
        Some(serde_json::json!({
            "start": {"line": start_line, "character": start_col},
            "end": {"line": end_line, "character": end_col}
        }))
    } else {
        None
    }
}
