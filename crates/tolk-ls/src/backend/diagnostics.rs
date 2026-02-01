use crate::backend::utils::{FileInfoExt, SpanExt, offset_to_range};
use diagnostic::Severity;
use lsp_types::*;
use std::sync::Arc;
use tolk_linter::diagnostic;
use tolk_resolver::file_db::FileInfo;

pub fn convert_single_diagnostic(
    diag: &diagnostic::Diagnostic,
    file: &Arc<FileInfo>,
) -> Option<Diagnostic> {
    let uri = file.url()?;

    let mut related_information = Vec::new();
    let mut primary_range = None;

    for annotation in &diag.annotations {
        let range = annotation.span.start_range(file);

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
            .map(|ann| offset_to_range(file, ann.span.start()))
            .unwrap_or_default()
    });

    let severity = match diag.severity {
        Severity::Info => DiagnosticSeverity::INFORMATION,
        Severity::Warning => DiagnosticSeverity::WARNING,
        Severity::Error => DiagnosticSeverity::ERROR,
        Severity::Fatal => DiagnosticSeverity::ERROR,
        Severity::Help => DiagnosticSeverity::HINT,
    };

    Some(Diagnostic {
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
    })
}
