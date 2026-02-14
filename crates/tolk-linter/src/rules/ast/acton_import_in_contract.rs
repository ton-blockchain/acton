use crate::rules::diagnostic::{Annotation, Diagnostic, Severity};
use crate::rules::violation::{Violation, ViolationMetadata};
use crate::{Checker, FixAvailability};
use tolk_macros::ViolationMetadata;
use tolk_resolver::file_index::{FileId, Span};

/// ### What it does
/// Forbids importing files from `.acton` in contract dependency trees.
///
/// ### Why is this bad?
/// Contract sources should not depend on Acton internals from `.acton`.
/// Such imports are environment-dependent and make contracts less portable.
///
/// ### Example
/// ```tolk
/// import "../.acton/tlb/maybe.tolk";
/// ```
///
/// Use instead:
/// ```tolk
/// import "@stdlib/gas-payments";
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct ActonImportInContract;

impl Violation for ActonImportInContract {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::None;

    fn message(&self) -> String {
        "contracts cannot import files from Acton standard library".to_string()
    }
}

pub fn check_file(checker: &mut Checker, file_id: FileId) -> Option<()> {
    let project_index = checker.type_db.project_index;
    let imports = project_index.imports().get(&file_id)?;

    for resolved_import in imports {
        let Some(target_id) = resolved_import.target() else {
            continue;
        };
        if !checker.file_db.is_acton_file(target_id) {
            continue;
        }

        let import = resolved_import.import();
        fire_diagnostic(checker, file_id, import.span);
    }

    Some(())
}

#[cold]
fn fire_diagnostic(checker: &mut Checker, file_id: FileId, span: Span) {
    let diagnostic = Diagnostic {
        file_id,
        severity: Severity::Error,
        name: ActonImportInContract::rule().name(),
        code: ActonImportInContract::code().map(|c| c.to_string()),
        message: ActonImportInContract.message(),
        annotations: vec![Annotation {
            span,
            message: Some("this import resolves to a file in .acton".to_string()),
            is_primary: true,
            tags: vec![],
        }],
        fixes: vec![],
        help: Some(
            "Acton stdlib may use emulator-only instructions that are unavailable on-chain, so such imports can break contract execution.
Use on-chain-safe modules (for example, `@stdlib/...`), and if you only need data types, copy their definitions into your project."
                .to_string(),
        ),
    };
    checker.emit_diagnostic(ActonImportInContract::rule(), diagnostic);
}
