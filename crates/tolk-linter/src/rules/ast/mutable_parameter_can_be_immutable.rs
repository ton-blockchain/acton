use crate::rules::diagnostic::{Annotation, Applicability, Diagnostic, Edit, Fix};
use crate::rules::violation::Violation;
use crate::{Checker, FixAvailability};
use tolk_analysis::UseFlags;
use tolk_macros::ViolationMetadata;
use tolk_resolver::file_index::FileId;
use tolk_resolver::file_index::Span;
use tolk_resolver::resolve_index::LocalDefKind;
use tolk_syntax::{LambdaParameter, Parameter, TryFromNode};

/// ### What it does
/// Checks for parameters that are declared as mutable (`mutate`) but are never mutated.
///
/// ### Why is this bad?
/// Using immutable parameters makes the function contract clearer and reduces accidental mutations.
///
/// ### Example
/// ```tolk
/// fun increment(mutate value: int): int {
///     return value + 1;
/// }
/// ```
///
/// Use instead:
/// ```tolk
/// fun increment(value: int): int {
///     return value + 1;
/// }
/// ```
///
/// ### Behavior notes
/// - Parameters with zero usages are not reported by this rule.
/// - Passing a parameter as `mutate a` counts as write usage and suppresses this diagnostic.
/// - Autofix removes the `mutate` keyword (and one trailing whitespace if present).
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct MutableParameterCanBeImmutable;

impl Violation for MutableParameterCanBeImmutable {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Always;

    fn message(&self) -> String {
        "parameter can be immutable".to_string()
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
            LocalDefKind::Param {
                is_mutable: true,
                is_self: false,
                in_asm_or_builtin: false,
                ..
            }
        ) {
            // not a mutable parameter
            continue;
        }

        let Some(facts) = use_facts.per_local.get(&local.id) else {
            continue;
        };

        if facts.flags.is_empty() {
            // no usages for parameter, don't report additional diagnostic
            continue;
        }

        if facts.flags.contains(UseFlags::WRITE) {
            // parameter is used for writing somewhere
            continue;
        }

        let mut fixes = vec![];

        if let Some(def_node) =
            root.descendant_for_byte_range(local.def_span.start(), local.def_span.end())
            && let Some(span) = find_mutate_keyword_span(def_node, file.source().source.as_bytes())
        {
            fixes.push(Fix {
                message: "remove `mutate`".to_string(),
                edits: vec![Edit {
                    span,
                    replacement: String::new(),
                    file_id,
                }],
                applicability: Applicability::Auto,
            });
        }

        let diagnostic = Diagnostic::warning_for(file_id, MutableParameterCanBeImmutable)
            .with_annotations(vec![Annotation {
                span: local.def_span,
                message: Some("can be made immutable".to_owned()),
                is_primary: true,
                tags: vec![],
            }])
            .with_fixes(fixes);
        checker.emit_diagnostic(diagnostic);
    }
    Some(())
}

fn find_mutate_keyword_span(node: tree_sitter::Node<'_>, source: &[u8]) -> Option<Span> {
    let mut current = Some(node);
    while let Some(node) = current {
        if let Ok(parameter) = Parameter::try_from_node(node)
            && let Some(mutate_node) = parameter.0.child_by_field_name("mutate")
        {
            return Some(mutate_keyword_span(&mutate_node, source));
        }
        if let Ok(parameter) = LambdaParameter::try_from_node(node)
            && let Some(mutate_node) = parameter.0.child_by_field_name("mutate")
        {
            return Some(mutate_keyword_span(&mutate_node, source));
        }
        current = node.parent();
    }
    None
}

fn mutate_keyword_span(mutate_node: &tree_sitter::Node<'_>, source: &[u8]) -> Span {
    let mut span = Span::from_syntax(mutate_node);
    if source.get(span.end()) == Some(&b' ') || source.get(span.end()) == Some(&b'\t') {
        span.end += 1;
    }
    span
}
