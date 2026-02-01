use crate::rules::diagnostic::{Annotation, Applicability, Diagnostic, Edit, Fix, Severity};
use crate::rules::violation::Violation;
use crate::rules::violation::ViolationMetadata;
use crate::{Checker, FixAvailability};
use tolk_analysis::UseFlags;
use tolk_macros::ViolationMetadata;
use tolk_resolver::file_index::FileId;
use tolk_resolver::file_index::Span;
use tolk_resolver::resolve_index::LocalDefKind;
use tolk_syntax::HasTreeSitterKind;
use tolk_syntax::{VarDeclLhs, match_parents};

/// ### What it does
/// Checks for variables that are declared as mutable (`var`) but are never mutated.
///
/// ### Why is this bad?
/// Using `val` instead of `var` makes the code clearer by signaling that the variable's value will not change.
///
/// ### Example
/// ```tolk
/// fun main() {
///     var x = 1;
///     println(x);
/// }
/// ```
///
/// Use instead:
/// ```tolk
/// fun main() {
///     val x = 1;
///     println(x);
/// }
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct MutableVariableCanBeImmutable;

impl Violation for MutableVariableCanBeImmutable {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Always;

    fn message(&self) -> String {
        "variable can be immutable".to_string()
    }
}

pub fn check_file(checker: &mut Checker, file_id: FileId) -> Option<()> {
    let file = checker.file_db.get_by_id(file_id)?;
    let resolved_index = checker.resolve_index_for(file_id)?;
    let root = file.source().tree.root_node();
    let use_facts = checker.use_facts(file_id)?;

    for local in &resolved_index.locals {
        if !matches!(
            local.kind,
            LocalDefKind::Var {
                is_mutable: true,
                ..
            }
        ) {
            // not a mutable variable
            continue;
        }

        let Some(facts) = use_facts.per_local.get(&local.id) else {
            continue;
        };

        if facts.flags.is_empty() {
            // no usages for variable, don't report additional diagnostic
            continue;
        }

        if facts.flags.contains(UseFlags::WRITE) {
            // variable is used for writing somewhere
            continue;
        }

        let mut fixes = vec![];

        // Try to find the `var` keyword to replace it with `val`
        if let Some(def_node) =
            root.descendant_for_byte_range(local.def_span.start(), local.def_span.end())
            && let Some(decl) = match_parents!(def_node, VarDeclLhs(...))
            && let Some(kind_node) = decl.kind_node()
        {
            fixes.push(Fix {
                message: "use `val` instead".to_string(),
                edits: vec![Edit {
                    span: Span::from_syntax(&kind_node),
                    replacement: "val".to_string(),
                }],
                applicability: Applicability::Auto,
            });
        }

        let diagnostic = Diagnostic {
            file_id,
            severity: Severity::Warning,
            name: MutableVariableCanBeImmutable::rule().name(),
            code: MutableVariableCanBeImmutable::code().map(|c| c.to_string()),
            message: MutableVariableCanBeImmutable.message(),
            annotations: vec![Annotation {
                span: local.def_span,
                message: Some("can be made val".to_owned()),
                is_primary: true,
                tags: vec![],
            }],
            fixes,
            help: None,
        };
        checker.emit_diagnostic(MutableVariableCanBeImmutable::rule(), diagnostic);
    }
    Some(())
}
