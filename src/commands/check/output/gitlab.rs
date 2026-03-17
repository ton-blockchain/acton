use crate::commands::check::pos;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::Path;
use tolk_linter::diagnostic::{Annotation, Applicability, Diagnostic, Severity};
use tolk_resolver::FileDb;

#[derive(serde::Serialize)]
struct CodeQualityIssue {
    description: String,
    check_name: String,
    fingerprint: String,
    severity: &'static str,
    location: CodeQualityLocation,
    content: CodeQualityContent,
}

#[derive(serde::Serialize)]
struct CodeQualityLocation {
    path: String,
    lines: CodeQualityLines,
}

#[derive(serde::Serialize)]
struct CodeQualityLines {
    begin: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    end: Option<u32>,
}

#[derive(serde::Serialize)]
struct CodeQualityContent {
    body: String,
}

pub(crate) fn write_report(
    writer: &mut dyn Write,
    diagnostics: &[Diagnostic],
    file_db: &FileDb,
    project_root: &Path,
) -> anyhow::Result<()> {
    let mut sorted_diagnostics = diagnostics.to_vec();
    sorted_diagnostics.sort();

    let report = sorted_diagnostics
        .iter()
        .filter_map(|diagnostic| diagnostic_to_issue(diagnostic, file_db, project_root))
        .collect::<Vec<_>>();

    let json = serde_json::to_string_pretty(&report)?;
    writer.write_all(json.as_bytes())?;
    Ok(())
}

fn diagnostic_to_issue(
    diagnostic: &Diagnostic,
    file_db: &FileDb,
    project_root: &Path,
) -> Option<CodeQualityIssue> {
    let file_info = file_db.get_by_id(diagnostic.file_id)?;
    let primary_annotation = primary_annotation(diagnostic);
    let source = file_info.source().source.as_ref();
    let lines = annotation_lines(primary_annotation, source);
    let path = diagnostic_file(file_info.path(), project_root);

    let description = diagnostic.message.clone();
    let check_name = diagnostic_check_name(diagnostic);

    Some(CodeQualityIssue {
        description,
        check_name: check_name.clone(),
        fingerprint: diagnostic_fingerprint(&check_name, &path, &lines, primary_annotation),
        severity: severity_to_gitlab_level(diagnostic.severity),
        location: CodeQualityLocation { path, lines },
        content: diagnostic_content(diagnostic, primary_annotation),
    })
}

fn primary_annotation(diagnostic: &Diagnostic) -> Option<&Annotation> {
    diagnostic
        .annotations
        .iter()
        .find(|annotation| annotation.is_primary)
        .or_else(|| diagnostic.annotations.first())
}

fn annotation_lines(annotation: Option<&Annotation>, source: &str) -> CodeQualityLines {
    let Some(annotation) = annotation else {
        return CodeQualityLines {
            begin: 1,
            end: None,
        };
    };

    let Some((start_line, _)) = pos::byte_to_line_col(source, annotation.span.start as usize)
    else {
        return CodeQualityLines {
            begin: 1,
            end: None,
        };
    };

    let begin = start_line + 1;
    let end = pos::byte_to_line_col(source, annotation.span.end as usize)
        .map(|(end_line, _)| end_line + 1)
        .filter(|end_line| *end_line > begin);

    CodeQualityLines { begin, end }
}

fn diagnostic_check_name(diagnostic: &Diagnostic) -> String {
    match diagnostic.code.as_deref() {
        Some(code) => format!("{code}:{}", diagnostic.name),
        None => diagnostic.name.to_string(),
    }
}

fn diagnostic_fingerprint(
    check_name: &str,
    path: &str,
    lines: &CodeQualityLines,
    primary_annotation: Option<&Annotation>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(check_name.as_bytes());
    hasher.update([0]);
    hasher.update(path.as_bytes());
    hasher.update([0]);
    hasher.update(lines.begin.to_string().as_bytes());
    hasher.update([0]);
    if let Some(end) = lines.end {
        hasher.update(end.to_string().as_bytes());
    }
    hasher.update([0]);
    if let Some(annotation) = primary_annotation {
        hasher.update(annotation.span.start.to_string().as_bytes());
        hasher.update([0]);
        hasher.update(annotation.span.end.to_string().as_bytes());
    }

    format!("{:x}", hasher.finalize())
}

fn diagnostic_content(
    diagnostic: &Diagnostic,
    primary_annotation: Option<&Annotation>,
) -> CodeQualityContent {
    let mut body = Vec::new();
    match diagnostic.code.as_deref() {
        Some(code) => body.push(format!("Rule: `{}` (`{code}`)", diagnostic.name)),
        None => body.push(format!("Rule: `{}`", diagnostic.name)),
    }

    let primary_message = primary_annotation.and_then(|annotation| annotation.message.as_deref());
    if let Some(message) = primary_message.filter(|message| *message != diagnostic.message) {
        body.push(format!("Primary: {message}"));
    }

    if let Some(help) = diagnostic.help.as_deref() {
        body.push(format!("Help: {help}"));
    }

    let secondary_messages = diagnostic
        .annotations
        .iter()
        .filter(|annotation| !annotation.is_primary)
        .filter_map(|annotation| annotation.message.as_deref())
        .collect::<Vec<_>>();
    if !secondary_messages.is_empty() {
        body.push("Related:".to_string());
        for message in secondary_messages {
            body.push(format!("- {message}"));
        }
    }

    if !diagnostic.fixes.is_empty() {
        body.push("Fixes:".to_string());
        for fix in &diagnostic.fixes {
            body.push(format!(
                "- [{}] {}",
                fix_applicability(fix.applicability),
                fix.message
            ));
        }
    }

    CodeQualityContent {
        body: body.join("\n"),
    }
}

const fn fix_applicability(applicability: Applicability) -> &'static str {
    match applicability {
        Applicability::Auto => "auto",
        Applicability::Manual => "manual",
    }
}

const fn severity_to_gitlab_level(severity: Severity) -> &'static str {
    match severity {
        Severity::Info | Severity::Help => "info",
        Severity::Warning => "minor",
        Severity::Error => "major",
        Severity::Fatal => "critical",
    }
}

fn diagnostic_file(path: &Path, project_root: &Path) -> String {
    if let Ok(relative) = path.strip_prefix(project_root) {
        return relative.to_string_lossy().to_string();
    }

    path.to_string_lossy().to_string()
}
