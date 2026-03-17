use crate::commands::check::pos;
use std::io::Write;
use std::path::Path;
use tolk_linter::diagnostic::{Annotation, Diagnostic, Severity};
use tolk_resolver::{FileDb, FileInfo};

const fn severity_to_annotation_level(severity: Severity) -> &'static str {
    match severity {
        Severity::Info => "notice",
        Severity::Warning => "warning",
        Severity::Error | Severity::Fatal => "error",
        Severity::Help => "notice",
    }
}

// Description of the `Workflow commands` format
// https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-commands

pub(crate) fn write_report(
    writer: &mut dyn Write,
    diagnostics: &[Diagnostic],
    file_db: &FileDb,
    project_root: &Path,
) -> anyhow::Result<()> {
    let mut sorted_diagnostics = diagnostics.to_vec();
    sorted_diagnostics.sort();

    for diagnostic in sorted_diagnostics {
        let Some(file_info) = file_db.get_by_id(diagnostic.file_id) else {
            continue;
        };

        let annotation = diagnostic
            .annotations
            .iter()
            .find(|annotation| annotation.is_primary)
            .or_else(|| diagnostic.annotations.first());

        let Some(annotation) = annotation else {
            continue;
        };

        let start_line = diagnostic_start_line(annotation, file_info.as_ref());
        let Some(start_line) = start_line else {
            continue;
        };

        let severity = severity_to_annotation_level(diagnostic.severity);
        let filepath = diagnostic_file(file_info.path(), project_root);
        let title = diagnostic_title(&diagnostic);
        let message = diagnostic_message(&diagnostic, annotation);

        write_annotation(writer, severity, filepath, start_line, title, message)?;
    }

    Ok(())
}

fn write_annotation(
    writer: &mut dyn Write,
    severity: &str,
    filepath: String,
    start_line: u32,
    title: String,
    message: String,
) -> std::io::Result<()> {
    writeln!(
        writer,
        "::{severity} file={},line={start_line},title={}::{}",
        escape_property(&filepath),
        escape_property(&title),
        escape_data(&message),
    )
}

fn diagnostic_start_line(annotation: &Annotation, file_info: &FileInfo) -> Option<u32> {
    let source = file_info.source().source.as_ref();
    let (start_line, _) = pos::byte_to_line_col(source, annotation.span.start as usize)?;
    Some(start_line + 1)
}

fn diagnostic_title(diagnostic: &Diagnostic) -> String {
    match diagnostic.code.as_deref() {
        Some(code) => format!("[{code}]: {}", diagnostic.name),
        None => diagnostic.name.to_string(),
    }
}

fn diagnostic_message(diagnostic: &Diagnostic, annotation: &Annotation) -> String {
    annotation
        .message
        .as_deref()
        .unwrap_or(diagnostic.message.as_str())
        .to_string()
}

fn diagnostic_file(report_path: &Path, project_root: &Path) -> String {
    if let Ok(relative) = report_path.strip_prefix(project_root) {
        return relative.to_string_lossy().to_string();
    }

    report_path.to_string_lossy().to_string()
}

fn escape_property(text: &str) -> String {
    escape_data(text).replace(':', "%3A").replace(',', "%2C")
}

fn escape_data(text: &str) -> String {
    text.replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_error_levels_to_github_error() {
        assert_eq!(severity_to_annotation_level(Severity::Warning), "warning");
        assert_eq!(severity_to_annotation_level(Severity::Error), "error");
        assert_eq!(severity_to_annotation_level(Severity::Fatal), "error");
    }

    #[test]
    fn escapes_property_field() {
        assert_eq!(escape_property("a:b,c"), "a%3Ab%2Cc");
    }

    #[test]
    fn escapes_data_field() {
        assert_eq!(escape_data("a%b\r\nc"), "a%25b%0D%0Ac");
        assert_eq!(escape_data("a:b,c"), "a:b,c");
    }

    #[test]
    fn escapes_filepath_and_keeps_message_punctuation() {
        let mut output = Vec::new();
        write_annotation(
            &mut output,
            "error",
            "contracts/a:b,c.tolk".to_string(),
            10,
            "title: part".to_string(),
            "message: keep, punctuation".to_string(),
        )
        .unwrap();

        let line = String::from_utf8(output).unwrap();
        assert_eq!(
            line,
            "::error file=contracts/a%3Ab%2Cc.tolk,line=10,title=title%3A part::message: keep, punctuation\n"
        );
    }
}
