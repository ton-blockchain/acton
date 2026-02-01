use crate::backend::utils::offset_to_range;
use lsp_types::*;
use std::sync::Arc;
use tolk_resolver::file_db::FileInfo;
use tower_lsp::lsp_types::Url;

pub fn convert_single_diagnostic(
    diag: &tolk_linter::diagnostic::Diagnostic,
    file_info: &Arc<FileInfo>,
) -> Diagnostic {
    let uri = Url::from_file_path(&file_info.index().path).unwrap();

    let mut related_information = Vec::new();
    let mut primary_range = None;

    for annotation in &diag.annotations {
        let range = offset_to_range(file_info, annotation.span.start());

        if annotation.is_primary {
            primary_range = Some(range);
        } else if let Some(message) = &annotation.message {
            related_information.push(DiagnosticRelatedInformation {
                location: Location::new(uri.clone(), range),
                message: message.clone(),
            });
        }
    }

    let range = primary_range.unwrap_or_else(|| {
        diag.annotations
            .first()
            .map(|ann| offset_to_range(file_info, ann.span.start()))
            .unwrap_or_default()
    });

    let severity = match diag.severity {
        tolk_linter::diagnostic::Severity::Info => DiagnosticSeverity::INFORMATION,
        tolk_linter::diagnostic::Severity::Warning => DiagnosticSeverity::WARNING,
        tolk_linter::diagnostic::Severity::Error => DiagnosticSeverity::ERROR,
        tolk_linter::diagnostic::Severity::Fatal => DiagnosticSeverity::ERROR,
        tolk_linter::diagnostic::Severity::Help => DiagnosticSeverity::HINT,
    };

    Diagnostic {
        range,
        severity: Some(severity),
        code: None,
        code_description: None,
        source: Some("tolk-linter".to_string()),
        message: diag.message.clone(),
        related_information: if related_information.is_empty() {
            None
        } else {
            Some(related_information)
        },
        tags: None,
        data: None,
    }
}
