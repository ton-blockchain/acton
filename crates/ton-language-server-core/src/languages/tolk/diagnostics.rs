use super::file_info::FileInfoExt;
use super::{TolkProjectConfig, TolkResolveSnapshot, TolkWorkspaceEngine};
use crate::{
    CodeAction, CodeActionKind, Diagnostic, DiagnosticSeverity, DiagnosticTag, DocumentEdits,
    DocumentSnapshot, Position, Range, TextEdit, TolkDiagnosticSettings, WorkspaceEdit,
    profiling::Profiler,
};
use std::collections::BTreeMap;
use tolk_linter::Checker;
use tolk_linter::diagnostic::{
    Annotation, Diagnostic as LintDiagnostic, DiagnosticTag as LintDiagnosticTag, Severity,
};
use tolk_ty::TypeDb;

impl TolkWorkspaceEngine {
    pub(super) fn diagnostics(
        &self,
        document: &DocumentSnapshot,
        settings: &TolkDiagnosticSettings,
        profiler: &mut Profiler,
    ) -> Vec<Diagnostic> {
        if !settings.enabled {
            return Vec::new();
        }
        #[cfg(not(feature = "tolk-compiler"))]
        let _ = profiler;
        let (snapshot, config) = {
            let state = self.state.read().expect("Tolk workspace lock poisoned");
            (state.latest_snapshot.clone(), state.project_config.clone())
        };
        let Some(snapshot) = snapshot else {
            return Vec::new();
        };
        let Some(file_id) = snapshot.find_document_file(document) else {
            return Vec::new();
        };
        let Some(file) = snapshot.file_db.get_by_id(file_id) else {
            return Vec::new();
        };

        #[cfg_attr(not(feature = "tolk-compiler"), allow(unused_mut))]
        let mut diagnostics = if settings.linter.enabled {
            lint_document(&snapshot, &config, file_id)
        } else {
            Vec::new()
        }
        .into_iter()
        .map(|diagnostic| {
            let annotation = primary_annotation(&diagnostic.annotations);
            let range = annotation.map_or_else(
                || Range::new(Position::new(0, 0), Position::new(0, 0)),
                |annotation| file.range_for_span(annotation.span),
            );
            let severity = match diagnostic.severity {
                Severity::Fatal | Severity::Error => DiagnosticSeverity::Error,
                Severity::Warning => DiagnosticSeverity::Warning,
                Severity::Info => DiagnosticSeverity::Information,
                Severity::Help => DiagnosticSeverity::Hint,
            };
            let code = diagnostic.code;
            let mut message = diagnostic.message;
            if let Some(label) = annotation.and_then(|annotation| annotation.message.as_ref())
                && label != &message
            {
                message.push('\n');
                message.push_str(label);
            }
            if let Some(help) = diagnostic.help
                && help != message
            {
                message.push('\n');
                message.push_str(&help);
            }
            let tags = annotation
                .into_iter()
                .flat_map(|annotation| &annotation.tags)
                .map(|tag| match tag {
                    LintDiagnosticTag::Unnecessary => DiagnosticTag::Unnecessary,
                    LintDiagnosticTag::Deprecated => DiagnosticTag::Deprecated,
                })
                .collect();
            let diagnostic = Diagnostic::new(range, severity, "acton", message).with_tags(tags);
            match code {
                Some(code) => diagnostic.with_code(code),
                None => diagnostic,
            }
        })
        .collect::<Vec<_>>();
        #[cfg(feature = "tolk-compiler")]
        if settings.compiler.enabled {
            diagnostics.extend(compiler_diagnostics(
                &snapshot, &config, document, &file, profiler,
            ));
        }
        diagnostics
    }
}

#[cfg(feature = "tolk-compiler")]
fn compiler_diagnostics(
    snapshot: &TolkResolveSnapshot,
    config: &TolkProjectConfig,
    document: &DocumentSnapshot,
    file: &tolk_resolver::FileInfo,
    profiler: &mut Profiler,
) -> Vec<Diagnostic> {
    if !document.uri().as_str().starts_with("file:") {
        return Vec::new();
    }

    let compiler = {
        let _profile = profiler.span("tolk.diagnostics.compiler.prepare");
        tolk_compiler::Compiler::new(2)
            .with_allow_no_entrypoint(!config.is_contract_root(file.path()))
            .with_mappings(&config.import_mappings)
            .with_source_overrides(snapshot.file_db.iter().map(|source_file| {
                (
                    source_file.path().clone(),
                    source_file.source().source.clone(),
                )
            }))
    };
    let result = {
        let _profile = profiler.span("tolk.diagnostics.compiler.check");
        compiler.check(file.path())
    };
    let _profile = profiler.span("tolk.diagnostics.compiler.convert");

    match result {
        Ok(errors) => errors
            .into_iter()
            .filter(|error| compiler_error_belongs_to_file(&error.range.file_name, file.path()))
            .map(|error| {
                let range = compiler_error_range(file, &error.range);
                let severity = if error.is_warning {
                    DiagnosticSeverity::Warning
                } else {
                    DiagnosticSeverity::Error
                };
                Diagnostic::new(range, severity, "tolk-compiler", error.message).with_code("C001")
            })
            .collect(),
        Err(error) => vec![
            Diagnostic::new(
                Range::new(Position::new(0, 0), Position::new(0, 0)),
                DiagnosticSeverity::Error,
                "tolk-compiler",
                format!("compiler check failed: {error}"),
            )
            .with_code("C001"),
        ],
    }
}

#[cfg(feature = "tolk-compiler")]
fn compiler_error_belongs_to_file(error_path: &str, file_path: &std::path::Path) -> bool {
    let error_path = std::path::Path::new(error_path);
    error_path == file_path
        || match (
            dunce::canonicalize(error_path),
            dunce::canonicalize(file_path),
        ) {
            (Ok(error_path), Ok(file_path)) => error_path == file_path,
            _ => false,
        }
}

#[cfg(feature = "tolk-compiler")]
fn compiler_error_range(
    file: &tolk_resolver::FileInfo,
    range: &tolk_compiler::CompilerErrorRange,
) -> Range {
    let source = file.source().source.as_ref();
    let source_len = source.len();
    let offset = |line: usize, column: usize| {
        file.line_offsets()
            .get(line.saturating_sub(1))
            .copied()
            .unwrap_or(source_len)
            .saturating_add(column.saturating_sub(1))
            .min(source_len)
    };
    // Malformed non-ASCII tokens can make compiler ranges split a UTF-8 code point.
    let mut start = offset(range.start_line_no, range.start_char_no);
    while !source.is_char_boundary(start) {
        start = start.saturating_sub(1);
    }
    let mut end = offset(range.end_line_no, range.end_char_no);
    while !source.is_char_boundary(end) {
        end = end.saturating_add(1).min(source_len);
    }
    file.range_for_span(tolk_resolver::Span {
        start: start as u32,
        end: end as u32,
    })
}

pub(super) fn lint_document(
    snapshot: &TolkResolveSnapshot,
    config: &TolkProjectConfig,
    file_id: u32,
) -> Vec<LintDiagnostic> {
    let Some(file) = snapshot.file_db.get_by_id(file_id) else {
        return Vec::new();
    };
    let mut type_interner = snapshot.type_interner.as_ref().clone();
    let mut type_db = TypeDb::new_with_cache(
        &mut type_interner,
        &snapshot.file_db,
        &snapshot.project_index,
        snapshot.type_db_cache.as_ref().clone(),
        std::iter::empty(),
    );
    let settings = config.lint_settings_for(file.path());
    let mut checker = Checker::new(&snapshot.file_db, &mut type_db, &snapshot.all_body_types)
        .with_settings(settings)
        .with_project_root(config.project_root.clone());
    checker.run_once();
    checker.process_file(file.source(), file_id);
    checker.apply_suppressions();
    checker
        .diagnostics
        .into_iter()
        .filter(|diagnostic| diagnostic.file_id == file_id)
        .collect()
}

pub(super) fn lint_code_actions(
    snapshot: &TolkResolveSnapshot,
    config: &TolkProjectConfig,
    file_id: u32,
    requested_range: Range,
) -> Vec<CodeAction> {
    let Some(file) = snapshot.file_db.get_by_id(file_id) else {
        return Vec::new();
    };
    let mut actions = Vec::new();
    for diagnostic in lint_document(snapshot, config, file_id) {
        let Some(annotation) = primary_annotation(&diagnostic.annotations) else {
            continue;
        };
        if !ranges_overlap(file.range_for_span(annotation.span), requested_range) {
            continue;
        }

        for fix in diagnostic.fixes {
            let mut documents = BTreeMap::<String, Vec<TextEdit>>::new();
            for edit in fix.edits {
                let Some(edit_file) = snapshot.file_db.get_by_id(edit.file_id) else {
                    continue;
                };
                let Some(uri) = snapshot.file_uris.get(&edit.file_id) else {
                    continue;
                };
                documents
                    .entry(uri.as_str().to_owned())
                    .or_default()
                    .push(TextEdit::new(
                        edit_file.range_for_span(edit.span),
                        edit.replacement,
                    ));
            }
            if documents.is_empty() {
                continue;
            }
            let documents = documents
                .into_iter()
                .map(|(uri, edits)| DocumentEdits::new(uri.into(), edits))
                .collect();
            actions.push(CodeAction::new(
                fix.message,
                CodeActionKind::QuickFix,
                WorkspaceEdit::new(documents),
            ));
        }
    }
    actions
}

fn ranges_overlap(left: Range, right: Range) -> bool {
    left.start <= right.end && right.start <= left.end
}

fn primary_annotation(annotations: &[Annotation]) -> Option<&Annotation> {
    annotations
        .iter()
        .find(|annotation| annotation.is_primary)
        .or_else(|| annotations.first())
}
