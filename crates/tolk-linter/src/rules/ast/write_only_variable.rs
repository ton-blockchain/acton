use crate::rules::diagnostic::{Annotation, Diagnostic};
use crate::rules::violation::Violation;
use crate::{Checker, FixAvailability};
use tolk_analysis::UseFlags;
use tolk_macros::ViolationMetadata;
use tolk_resolver::file_index::FileId;
use tolk_resolver::resolve_index::LocalDefKind;

/// ### What it does
/// Checks for variables that are written to but never read.
///
/// ### Why is this bad?
/// A variable that is only written to but never read is likely a bug or redundant code.
///
/// ### Example
/// ```tolk
/// fun main() {
///     var x = 1;
///     x = 2;
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
/// - Locals prefixed with `_` are ignored.
/// - Mutable parameters (`mutate`) are ignored.
/// - Variables with zero usages are handled by `unused-variable`.
/// - This rule only reports variables with writes and no reads.
///   Operations that imply reads (for example `+=`) do not match this rule.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct WriteOnlyVariable;

impl Violation for WriteOnlyVariable {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::None;

    fn message(&self) -> String {
        "variable is only written to but never read".to_string()
    }
}

pub fn check_file(checker: &mut Checker, file_id: FileId) -> Option<()> {
    let resolved_index = checker.resolve_index_for(file_id)?;
    let use_facts = checker.use_facts(file_id)?;

    for local in &resolved_index.locals {
        if matches!(
            local.kind,
            LocalDefKind::Param {
                is_mutable: true,
                ..
            }
        ) {
            continue;
        }
        if local.name.starts_with('_') {
            continue;
        }

        let Some(facts) = use_facts.per_local.get(&local.id) else {
            continue;
        };

        if facts.flags.is_empty() {
            // unused
            continue;
        }

        if facts.flags.contains(UseFlags::READ) {
            // certainly read
            continue;
        }

        let diagnostic =
            Diagnostic::warning_for(file_id, WriteOnlyVariable).with_annotations(vec![
                Annotation {
                    span: local.def_span,
                    message: Some(format!("variable `{}` is write-only", local.name)),
                    is_primary: true,
                    tags: vec![],
                },
            ]);

        let mut diagnostic = diagnostic;
        if let Some(write_span) = facts.first_write_span {
            diagnostic.annotations.push(Annotation {
                span: write_span,
                message: Some("first write usage here".to_string()),
                is_primary: false,
                tags: vec![],
            });
        }

        checker.emit_diagnostic(diagnostic);
    }

    Some(())
}
