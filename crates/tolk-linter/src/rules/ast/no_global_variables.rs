use crate::diagnostic::{Annotation, Diagnostic};
use crate::{Checker, FixAvailability, Violation};
use tolk_macros::ViolationMetadata;
use tolk_resolver::{AstNodeSpanExt, FileId, Span};
use tolk_syntax::{GlobalVar, HasName};

/// ### What it does
/// Disallows top-level `global` variable declarations.
///
/// ### Why is this bad?
/// Global mutable state makes contracts harder to reason about and audit.
/// Keeping state in explicit storage structures and passing values through function scope is
/// clearer and less error-prone.
///
/// ### Example
/// ```tolk
/// global counter: int
/// ```
///
/// Use instead:
/// ```tolk
/// struct State {
///     counter: int
/// }
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct NoGlobalVariables;

impl Violation for NoGlobalVariables {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::None;

    fn message(&self) -> String {
        "global variable declarations are not allowed".to_owned()
    }
}

pub fn check_global_var(checker: &mut Checker, file_id: FileId, node: &GlobalVar) -> Option<()> {
    let global_name = node.name()?;
    fire_diagnostic(checker, file_id, global_name.span());
    Some(())
}

#[cold]
fn fire_diagnostic(checker: &mut Checker, file_id: FileId, span: Span) {
    let diagnostic = Diagnostic::warning_for(file_id, NoGlobalVariables)
        .with_annotations(vec![Annotation {
            span,
            message: Some("global variable declaration is not allowed".to_string()),
            is_primary: true,
            tags: vec![],
        }])
        .with_help("move the variable into a function scope or contract storage");
    checker.emit_diagnostic(diagnostic);
}
