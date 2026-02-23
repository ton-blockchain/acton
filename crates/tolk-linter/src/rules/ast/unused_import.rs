use crate::diagnostic::DiagnosticTag;
use crate::rules::diagnostic::{Annotation, Applicability, Diagnostic, Edit, Fix};
use crate::rules::violation::Violation;
use crate::{Checker, FixAvailability};
use rustc_hash::{FxBuildHasher, FxHashSet};
use tolk_macros::ViolationMetadata;
use tolk_resolver::file_index::{FileId, Span};
use tolk_resolver::resolve_index::Resolved;

/// ### What it does
/// Checks for imports that are never used in the file.
///
/// ### Why is this bad?
/// Unused imports clutter the code and increase compilation time.
///
/// ### Example
/// ```tolk
/// import "other.tolk";
///
/// fun main() {
///     println("hello");
/// }
/// ```
///
/// Use instead:
/// ```tolk
/// fun main() {
///     println("hello");
/// }
/// ```
///
/// ### Behavior notes
/// - Autofix is available only when the unused import can be removed as a whole-line edit.
/// - If multiple imports share one line, warning is emitted but autofix is skipped.
/// - Whole-line autofix also removes inline comments attached to that import line.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct UnusedImport;

impl Violation for UnusedImport {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    fn message(&self) -> String {
        "unused import".to_string()
    }
}

pub fn check_file(checker: &mut Checker, file_id: FileId) -> Option<()> {
    let file = checker.file_db.get_by_id(file_id)?;
    let source = file.source().source.as_ref();

    let project_index = checker.type_db.project_index;
    let imports = project_index.imports().get(&file_id)?;
    if imports.is_empty() {
        // fast path for files without imports
        return None;
    }

    let resolve_index = checker.resolve_index_for(file_id)?;

    let mut used_files = FxHashSet::with_capacity_and_hasher(10, FxBuildHasher);
    for name_use in &resolve_index.uses {
        if let Resolved::Global(symbol_id) = name_use.resolved {
            used_files.insert(symbol_id.file_id);
        }
    }

    for (&symbol_file_id, types) in checker.body_types {
        if symbol_file_id != file_id {
            continue;
        }

        for inference in types.values() {
            for name_use in &inference.resolved_refs {
                if let Resolved::Global(symbol_id) = name_use.resolved {
                    used_files.insert(symbol_id.file_id);
                }
            }
        }
    }

    for resolved_import in imports {
        let Some(target_id) = resolved_import.target() else {
            continue;
        };

        if !used_files.contains(&target_id) {
            fire_diagnostic(checker, resolved_import.import().span, file_id, source);
        }
    }

    Some(())
}

#[cold]
fn fire_diagnostic(checker: &mut Checker, span: Span, file_id: FileId, source: &str) {
    let fixes = if let Some(removal_span) = expand_to_whole_line(source, span) {
        vec![Fix {
            message: "remove unused import".to_string(),
            edits: vec![Edit {
                span: removal_span,
                replacement: "".to_string(),
                file_id,
            }],
            applicability: Applicability::Auto,
        }]
    } else {
        vec![]
    };

    let diagnostic = Diagnostic::warning_for(file_id, UnusedImport)
        .with_annotations(vec![Annotation {
            span,
            message: Some("this import is unused".to_string()),
            is_primary: true,
            tags: vec![DiagnosticTag::Unnecessary],
        }])
        .with_fixes(fixes);
    checker.emit_diagnostic(diagnostic);
}

fn expand_to_whole_line(source: &str, span: Span) -> Option<Span> {
    let start = span.start as usize;
    let end = span.end as usize;

    let line_start = source[..start].rfind('\n').map(|idx| idx + 1).unwrap_or(0);
    if !source[line_start..start].trim().is_empty() {
        return None;
    }

    let line_end = source[end..]
        .find('\n')
        .map(|idx| end + idx + 1)
        .unwrap_or(source.len());
    let trailing = source[end..line_end].trim();
    if !is_safe_trailing_after_import(trailing) {
        return None;
    }

    Some(Span {
        start: line_start as u32,
        end: line_end as u32,
    })
}

fn is_safe_trailing_after_import(trailing: &str) -> bool {
    if trailing.is_empty() {
        return true;
    }

    let Some(rest) = trailing.strip_prefix(';') else {
        return false;
    };
    let rest = rest.trim_start();
    rest.is_empty() || rest.starts_with("//") || rest.starts_with("/*")
}
