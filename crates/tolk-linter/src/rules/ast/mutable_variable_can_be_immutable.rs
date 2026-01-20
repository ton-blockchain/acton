use crate::rules::diagnostic::{Annotation, Applicability, Diagnostic, Edit, Fix, Severity};
use crate::rules::violation::Violation;
use crate::{Checker, FixAvailability};
use tolk_macros::ViolationMetadata;
use tolk_resolver::AstNodeSpanExt;
use tolk_resolver::file_index::FileId;
use tolk_resolver::file_index::Span;
use tolk_resolver::file_index::SymbolKind;
use tolk_resolver::resolve_index::{LocalDefKind, Resolved};
use tolk_syntax::AstNode;
use tolk_syntax::AstNodeBytesKind;
use tolk_syntax::HasTreeSitterKind;
use tolk_syntax::{Assign, Call, CallArgument, DotAccess, SetAssign, VarDeclLhs, match_parents};

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

    for local in &resolved_index.locals {
        if !matches!(local.kind, LocalDefKind::Var { is_mutable: true }) {
            // not a mutable variable
            continue;
        }

        let mut usages = resolved_index.local_usages_of(local.id).peekable();
        if usages.peek().is_none() {
            // no usages for variable, don't report additional diagnostic
            continue;
        }

        // The variable is used for writing in a number of cases:
        // - if it is on the left side of an assignment
        // - if it is used in the `mutate` argument: `foo(mutate a)`
        // - if a mutating method is called on it

        let mut mutates = false;

        for usage in usages {
            let Some(usage_node) =
                root.descendant_for_byte_range(usage.span.start(), usage.span.end())
            else {
                continue;
            };

            // 1. Check assignments
            if let Some(assign) = match_parents!(usage_node, Assign(...))
                && assign.is_lhs(&usage_node)
            {
                mutates = true;
                break;
            }

            if let Some(assign) = match_parents!(usage_node, SetAssign(...))
                && assign.is_lhs(&usage_node)
            {
                mutates = true;
                break;
            }

            // 2. Check mutate arguments
            if let Some(argument) = match_parents!(usage_node, CallArgument)
                && argument.mutate()
            {
                mutates = true;
                break;
            }

            // 3. Check method calls
            if let Some((call, dot)) = match_parents!(usage_node, Call(dot: DotAccess))
                && let Some(callee) = call.callee()
                && dot.is_obj(&usage_node)
                && let Some(inference) = checker.body_types.get(&file_id)
            {
                for inference in inference.values() {
                    let resolved = inference.resolve(callee.span());

                    if let Some(resolved) = resolved
                        && let Resolved::Global(id) = resolved.resolved
                    {
                        let resolved = checker.type_db.project_index.resolve_symbol(id);
                        if let Some(resolved) = resolved
                            && let SymbolKind::Method { is_mutable, .. } = resolved.kind
                            && is_mutable
                        {
                            mutates = true;
                            break;
                        }
                    }
                }
            }

            if mutates {
                break;
            }
        }

        if !mutates {
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
                message: MutableVariableCanBeImmutable.message(),
                annotations: vec![Annotation {
                    span: local.def_span,
                    message: Some("can be made val".to_owned()),
                    is_primary: true,
                    tags: vec![],
                }],
                fixes,
            };
            checker.diagnostics.push(diagnostic);
        }
    }
    Some(())
}
