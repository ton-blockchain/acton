use crate::rules::diagnostic::{Annotation, Applicability, Diagnostic, DiagnosticTag, Edit, Fix};
use crate::rules::violation::Violation;
use crate::{Checker, FixAvailability};
use tolk_macros::ViolationMetadata;
use tolk_resolver::file_index::{FileId, Span};
use tolk_resolver::resolve_index::{LocalDef, LocalDefKind};
use tolk_syntax::{HasName, Ident, LambdaParameter, Parameter, TryFromNode, VarDecl};

/// ### What it does
/// Checks for variables and parameters that are declared but never used.
///
/// ### Why is this bad?
/// Unused variables and parameters clutter the code and can be a sign of a bug.
///
/// ### Example
/// ```tolk twoslash
/// fun main() {
///     val x = 1;
///     //  ^ E002: variable is unused
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
/// Or prefix with an underscore if the variable is intentionally unused:
/// ```tolk
/// fun main() {
///     val _x = 1;
///     println("hello");
/// }
/// ```
///
/// ### Behavior notes
/// - Locals prefixed with `_` are intentionally ignored.
/// - The rule also skips type parameters, implicit asm/builtin parameters, and `self`.
/// - Autofix prefixes the declaration identifier with `_`.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct UnusedVariable;

impl Violation for UnusedVariable {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Always;

    fn message(&self) -> String {
        "variable is unused".to_string()
    }
}

pub fn check_file(checker: &mut Checker, file_id: FileId) -> Option<()> {
    let file = checker.file_db.get_by_id(file_id)?;
    let resolved_index = checker.resolve_index_for(file_id)?;
    let root = file.source().tree.root_node();
    let use_facts = checker.use_facts(file_id)?;

    for local in &resolved_index.locals {
        if local.name.starts_with('_') {
            // fast path, no need to find usages
            continue;
        }
        if let LocalDefKind::Param {
            in_asm_or_builtin: true,
            ..
        } = local.kind
        {
            // parameters of assembly or builtin functions is always implicitly used
            continue;
        }
        if let LocalDefKind::Param { is_self: true, .. } = local.kind {
            // `self` has a dedicated rule (`method_can_be_static`), avoid duplicate diagnostics
            continue;
        }
        if matches!(local.kind, LocalDefKind::TypeParameter) {
            continue;
        }

        let Some(facts) = use_facts.per_local.get(&local.id) else {
            continue;
        };

        if !facts.flags.is_empty() {
            // local is used somewhere
            continue;
        }

        // no usage found
        fire_diagnostic(checker, root, local, file_id);
    }

    Some(())
}

#[cold]
fn fire_diagnostic(
    checker: &mut Checker,
    root: tree_sitter::Node,
    local: &LocalDef,
    file_id: FileId,
) {
    let name = local.name.clone();
    let mut fixes = vec![];

    // Try to find the identifier to suggest prefixing it with _
    if let Some(node) = root.descendant_for_byte_range(local.def_span.start(), local.def_span.end())
    {
        let ident = if let Ok(ident) = Ident::try_from_node(node) {
            Some(ident)
        } else if let Ok(v) = VarDecl::try_from_node(node) {
            v.name()
        } else if let Ok(p) = Parameter::try_from_node(node) {
            p.name()
        } else if let Ok(lp) = LambdaParameter::try_from_node(node) {
            lp.name()
        } else {
            None
        };

        if let Some(ident) = ident {
            fixes.push(Fix {
                message: format!("prefix with underscore: `_{name}`"),
                edits: vec![Edit {
                    span: Span::from_syntax(&ident.0),
                    replacement: format!("_{name}"),
                    file_id,
                }],
                applicability: Applicability::Auto,
            });
        }
    }

    let diagnostic = Diagnostic::warning_for(file_id, UnusedVariable)
        .with_annotations(vec![Annotation {
            span: local.def_span,
            message: Some(format!("unused variable `{name}`",)),
            is_primary: true,
            tags: vec![DiagnosticTag::Unnecessary],
        }])
        .with_fixes(fixes);
    checker.emit_diagnostic(diagnostic);
}
