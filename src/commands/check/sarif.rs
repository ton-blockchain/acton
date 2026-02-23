use crate::commands::check::pos;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use tolk_linter::diagnostic::{Diagnostic, Severity};
use tolk_resolver::{FileDb, Span};

pub(super) fn write_report(
    diagnostics: &[Diagnostic],
    file_db: &FileDb,
    output_path: &Path,
) -> anyhow::Result<()> {
    if let Some(parent) = output_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }

    let sarif = diagnostics_to_sarif(diagnostics, file_db);
    fs::write(output_path, format!("{}\n", serde_json::to_string_pretty(&sarif)?))?;
    Ok(())
}

fn diagnostics_to_sarif(diagnostics: &[Diagnostic], file_db: &FileDb) -> serde_json::Value {
    let mut rules = BTreeMap::new();
    let mut sorted_diagnostics = diagnostics.to_vec();
    sorted_diagnostics.sort();

    let results = sorted_diagnostics
        .iter()
        .map(|diag| {
            let rule_id = diagnostic_rule_id(diag);
            rules.entry(rule_id.clone()).or_insert_with(|| {
                serde_json::json!({
                    "id": rule_id,
                    "name": diag.name,
                    "shortDescription": {
                        "text": diag.message
                    }
                })
            });

            let mut result = serde_json::json!({
                "ruleId": diagnostic_rule_id(diag),
                "level": severity_to_sarif_level(diag.severity),
                "message": {
                    "text": diag.message
                }
            });

            if let Some(location) = diagnostic_location(diag, file_db) {
                result["locations"] = serde_json::json!([location]);
            }

            result
        })
        .collect::<Vec<_>>();

    serde_json::json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "acton check",
                    "rules": rules.into_values().collect::<Vec<_>>()
                }
            },
            "results": results
        }]
    })
}

fn diagnostic_location(diag: &Diagnostic, file_db: &FileDb) -> Option<serde_json::Value> {
    let file_info = file_db.get_by_id(diag.file_id)?;
    let file_path = file_info.index().path.to_string_lossy().replace('\\', "/");
    let source = file_info.source().source.as_ref();
    let annotation = diag
        .annotations
        .iter()
        .find(|annotation| annotation.is_primary)
        .or_else(|| diag.annotations.first());

    let mut physical_location = serde_json::json!({
        "artifactLocation": {
            "uri": file_path
        }
    });

    if let Some(annotation) = annotation
        && let Some(region) = span_to_region(source, &annotation.span)
    {
        physical_location["region"] = region;
    }

    Some(serde_json::json!({
        "physicalLocation": physical_location
    }))
}

fn span_to_region(source: &str, span: &Span) -> Option<serde_json::Value> {
    let (start_line, start_col) = pos::byte_to_line_col(source, span.start as usize)?;
    let (end_line, end_col) = pos::byte_to_line_col(source, span.end as usize)?;

    Some(serde_json::json!({
        "startLine": start_line + 1,
        "startColumn": start_col + 1,
        "endLine": end_line + 1,
        "endColumn": end_col + 1
    }))
}

fn diagnostic_rule_id(diag: &Diagnostic) -> String {
    diag.code.clone().unwrap_or_else(|| diag.name.to_string())
}

fn severity_to_sarif_level(severity: Severity) -> &'static str {
    match severity {
        Severity::Warning => "warning",
        Severity::Error | Severity::Fatal => "error",
        Severity::Info | Severity::Help => "note",
    }
}
