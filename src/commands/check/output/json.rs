use crate::commands::check::pos;
use acton_config::lint_output::{
    LintJsonAnnotation, LintJsonAnnotationTag, LintJsonDiagnostic, LintJsonFix,
    LintJsonFixApplicability, LintJsonFixEdit, LintJsonPosition, LintJsonRange, LintJsonReport,
    LintJsonSeverity, LintJsonSource,
};
use std::io::Write;
use tolk_linter::diagnostic::{Applicability, Diagnostic, DiagnosticTag, Severity};
use tolk_resolver::{FileDb, Span};

pub(crate) fn write_report(
    writer: &mut dyn Write,
    success: bool,
    all_diagnostics: &[Diagnostic],
    file_db: &FileDb,
) -> anyhow::Result<()> {
    let report = LintJsonReport {
        success,
        diagnostics: all_diagnostics
            .iter()
            .map(|diagnostic| diagnostic_to_json(diagnostic, file_db))
            .collect(),
    };

    let json = serde_json::to_string_pretty(&report)?;
    writer.write_all(json.as_bytes())?;
    Ok(())
}

fn diagnostic_to_json(diag: &Diagnostic, file_db: &FileDb) -> LintJsonDiagnostic {
    let file_info = file_db
        .get_by_id(diag.file_id)
        .expect("File info should exist for diagnostic");
    let file_path = file_info.index().path.to_string_lossy().to_string();
    let source = file_info.source().source.as_ref();

    let severity = match diag.severity {
        Severity::Warning => LintJsonSeverity::Warning,
        Severity::Error | Severity::Fatal => LintJsonSeverity::Error,
        Severity::Info | Severity::Help => LintJsonSeverity::Info,
    };

    let mut annotations = Vec::new();
    for annotation in &diag.annotations {
        if let Some(range) = create_range_json(source, annotation.span) {
            annotations.push(LintJsonAnnotation {
                range,
                message: annotation.message.clone(),
                is_primary: annotation.is_primary,
                tags: annotation_tags(annotation),
            });
        }
    }

    let mut fixes = Vec::new();
    for fix in &diag.fixes {
        let mut edits = Vec::new();
        for edit in &fix.edits {
            let (edit_source, edit_file) = file_db.get_by_id(edit.file_id).map_or_else(
                || (source.into(), file_path.clone()),
                |info| {
                    (
                        info.source().source.clone(),
                        info.index().path.to_string_lossy().to_string(),
                    )
                },
            );
            if let Some(range) = create_range_json(edit_source.as_ref(), edit.span) {
                edits.push(LintJsonFixEdit {
                    range,
                    new_text: edit.replacement.clone(),
                    file: edit_file,
                });
            }
        }
        let applicability = match fix.applicability {
            Applicability::Auto => LintJsonFixApplicability::Auto,
            Applicability::Manual => LintJsonFixApplicability::Manual,
        };
        fixes.push(LintJsonFix {
            message: fix.message.clone(),
            edits,
            applicability,
        });
    }

    LintJsonDiagnostic {
        file: file_path,
        severity,
        name: diag.name.to_string(),
        code: diag.code.clone(),
        message: diag.message.clone(),
        help: diag.help.clone(),
        annotations,
        fixes,
        source: LintJsonSource::Tolk,
    }
}

fn annotation_tags(
    annotation: &tolk_linter::diagnostic::Annotation,
) -> Option<Vec<LintJsonAnnotationTag>> {
    let tags = annotation
        .tags
        .iter()
        .map(|tag| match tag {
            DiagnosticTag::Unnecessary => LintJsonAnnotationTag::Unnecessary,
            DiagnosticTag::Deprecated => LintJsonAnnotationTag::Deprecated,
        })
        .collect::<Vec<_>>();

    (!tags.is_empty()).then_some(tags)
}

fn create_range_json(source: &str, span: Span) -> Option<LintJsonRange> {
    if let (Some((start_line, start_col)), Some((end_line, end_col))) = (
        pos::byte_to_line_col(source, span.start as usize),
        pos::byte_to_line_col(source, span.end as usize),
    ) {
        Some(LintJsonRange {
            start: LintJsonPosition {
                line: start_line,
                character: start_col,
            },
            end: LintJsonPosition {
                line: end_line,
                character: end_col,
            },
        })
    } else {
        None
    }
}
