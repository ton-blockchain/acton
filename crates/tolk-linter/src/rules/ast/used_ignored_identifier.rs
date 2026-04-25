use crate::rules::diagnostic::{Annotation, Applicability, Diagnostic, Edit, Fix};
use crate::rules::violation::Violation;
use crate::{Checker, FixAvailability};
use tolk_macros::ViolationMetadata;
use tolk_resolver::file_index::FileId;

/// ### What it does
/// Checks for usages of identifiers explicitly marked as unused with a leading underscore.
///
/// ### Why is this bad?
/// Prefixing with `_` means the identifier is intentionally unused.
/// Using it later is misleading and makes code harder to read.
///
/// ### Example
/// ```tolk twoslash
/// fun main() {
///     val _value = 10;
///     _value;
/// //  ^^^^^^ E010: identifier marked as unused is used
/// }
/// ```
///
/// Use instead:
/// ```tolk
/// fun main() {
///     val value = 10;
///     value;
/// }
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct UsedIgnoredIdentifier;

impl Violation for UsedIgnoredIdentifier {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Always;

    fn message(&self) -> String {
        "identifier marked as unused is used".to_string()
    }
}

pub fn check_file(checker: &mut Checker, file_id: FileId) -> Option<()> {
    let file = checker.file_db.get_by_id(file_id)?;
    if !file.is_workspace_file() {
        return None;
    }

    let resolve_index = checker.resolve_index_for(file_id)?;

    for local in &resolve_index.locals {
        if !local.name.starts_with('_') || local.name.starts_with("__") {
            continue;
        }
        let Some(renamed) = local.name.strip_prefix('_') else {
            continue;
        };
        if renamed.is_empty() {
            continue;
        }

        let usages = resolve_index.local_usages_of(local.id).collect::<Vec<_>>();
        let Some(first_use) = usages.first() else {
            continue;
        };
        let first_use_span = first_use.span;

        let mut edits = Vec::with_capacity(usages.len() + 1);
        edits.push(Edit {
            span: local.def_span,
            replacement: renamed.to_string(),
            file_id,
        });
        for usage in &usages {
            edits.push(Edit {
                span: usage.span,
                replacement: renamed.to_string(),
                file_id,
            });
        }

        let diagnostic = Diagnostic::warning_for(file_id, UsedIgnoredIdentifier)
            .with_annotations(vec![
                Annotation {
                    span: first_use_span,
                    message: Some(format!(
                        "`{}` is marked as unused but used here",
                        local.name
                    )),
                    is_primary: true,
                    tags: vec![],
                },
                Annotation {
                    span: local.def_span,
                    message: Some("declared with leading underscore here".to_string()),
                    is_primary: false,
                    tags: vec![],
                },
            ])
            .with_fixes(vec![Fix {
                message: format!("rename `{}` to `{}`", local.name, renamed),
                edits,
                applicability: Applicability::Auto,
            }])
            .with_help("remove leading underscore if this identifier should be used");
        checker.emit_diagnostic(diagnostic);
    }

    Some(())
}
