use crate::backend::Backend;
use crate::backend::utils::{FileInfoExt, SpanExt};
use diagnostic::Severity;
use lsp_types::*;
use rustc_hash::FxHashMap;
use std::sync::Arc;
use tolk_linter::diagnostic::{self, DiagnosticTag};
use tolk_resolver::file_db::FileInfo;
use tower_lsp::lsp_types::Url;

impl Backend {
    pub fn convert_linter_diagnostics_to_lsp(
        &self,
        diagnostics: &[diagnostic::Diagnostic],
    ) -> FxHashMap<Url, Vec<Diagnostic>> {
        let mut diagnostics_by_uri: FxHashMap<Url, Vec<Diagnostic>> = FxHashMap::default();

        for diag in diagnostics {
            let Some(file_info) = self.file_db.get_by_id(diag.file_id) else {
                continue;
            };
            let Some(uri) = file_info.url() else {
                continue;
            };
            let Some(lsp_diag) = convert_single_diagnostic(diag, &file_info) else {
                continue;
            };

            diagnostics_by_uri.entry(uri).or_default().push(lsp_diag);
        }

        diagnostics_by_uri
    }
}

pub fn convert_single_diagnostic(
    diag: &diagnostic::Diagnostic,
    file: &Arc<FileInfo>,
) -> Option<Diagnostic> {
    let uri = file.url()?;

    let mut related_information = Vec::new();
    let mut primary_range = None;

    let mut tags = Vec::new();

    for annotation in &diag.annotations {
        let range = annotation.span.range(file);

        for tag in &annotation.tags {
            match tag {
                DiagnosticTag::Unnecessary => {
                    tags.push(lsp_types::DiagnosticTag::UNNECESSARY);
                }
                DiagnosticTag::Deprecated => {
                    tags.push(lsp_types::DiagnosticTag::DEPRECATED);
                }
            }
        }

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
            .map(|ann| ann.span.range(file))
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
        tags: if tags.is_empty() { None } else { Some(tags) },
        data: None,
    })
}
