use crate::diagnostic::DiagnosticTag;
use crate::rules::diagnostic::{Annotation, Diagnostic, Severity};
use crate::rules::violation::Violation;
use crate::rules::violation::ViolationMetadata;
use crate::{Checker, FixAvailability};
use rustc_hash::{FxBuildHasher, FxHashSet};
use tolk_macros::ViolationMetadata;
use tolk_resolver::file_index::FileId;
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
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct UnusedImport;

impl Violation for UnusedImport {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::None;

    fn message(&self) -> String {
        "unused import".to_string()
    }
}

pub fn check_file(checker: &mut Checker, file_id: FileId) -> Option<()> {
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

    for resolved_import in imports {
        let Some(target_id) = resolved_import.target() else {
            continue;
        };

        if !used_files.contains(&target_id) {
            fire_diagnostic(checker, resolved_import.import().span, file_id);
        }
    }

    Some(())
}

#[cold]
fn fire_diagnostic(checker: &mut Checker, span: tolk_resolver::file_index::Span, file_id: FileId) {
    let diagnostic = Diagnostic {
        file_id,
        severity: Severity::Warning,
        code: UnusedImport::code().map(|c| c.to_string()),
        message: UnusedImport.message(),
        annotations: vec![Annotation {
            span,
            message: Some("this import is unused".to_string()),
            is_primary: true,
            tags: vec![DiagnosticTag::Unnecessary],
        }],
        fixes: vec![],
    };
    checker.diagnostics.push(diagnostic);
}
