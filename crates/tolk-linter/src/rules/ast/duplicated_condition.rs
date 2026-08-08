use crate::rules::diagnostic::{Annotation, Diagnostic};
use crate::rules::violation::Violation;
use crate::{Checker, FixAvailability};
use tolk_macros::ViolationMetadata;
use tolk_resolver::AstNodeSpanExt;
use tolk_resolver::file_index::FileId;
use tolk_syntax::{Expr, If, IfAlt, NodeTraversalExt};

/// ### What it does
/// Detects duplicated conditions in `if / else if` chains.
///
/// ### Why is this bad?
/// Repeating the same condition in one conditional chain usually means a
/// copy-paste bug and can make branches unreachable.
///
/// ### Example
/// ```tolk twoslash
/// if (a < 1) {
///     return 1;
/// } else if (a > 4) {
///     return 2;
/// } else if (a > 4) {
/// //         ^^^^^ E020: duplicated condition in conditional chain
///     return 3;
/// }
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct DuplicatedCondition;

impl Violation for DuplicatedCondition {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::None;

    fn message(&self) -> String {
        "duplicated condition in conditional chain".to_string()
    }
}

pub fn check_if(checker: &mut Checker, file_id: FileId, node: &If) -> Option<()> {
    if is_else_if(node) {
        return None;
    }

    let file = checker.file_db.get_by_id(file_id)?;
    let source = file.source().source.as_ref();
    let conditions = collect_if_conditions(node);
    emit_duplicate_diagnostics(checker, file_id, source, &conditions)
}

fn emit_duplicate_diagnostics(
    checker: &mut Checker,
    file_id: FileId,
    source: &str,
    conditions: &[Expr<'_>],
) -> Option<()> {
    let duplicate_pairs = find_duplicate_condition_pairs(conditions, source);
    if duplicate_pairs.is_empty() {
        return None;
    }

    for (lhs, rhs) in duplicate_pairs {
        let diagnostic = Diagnostic::warning_for(file_id, DuplicatedCondition)
            .with_annotations(vec![
                Annotation {
                    span: lhs.span(),
                    message: Some("first occurrence of this condition".to_string()),
                    is_primary: false,
                    tags: vec![],
                },
                Annotation {
                    span: rhs.span(),
                    message: Some("duplicated condition".to_string()),
                    is_primary: true,
                    tags: vec![],
                },
            ])
            .with_help("remove duplicated condition or change it");
        checker.emit_diagnostic(diagnostic);
    }

    Some(())
}

fn find_duplicate_condition_pairs<'tree>(
    conditions: &[Expr<'tree>],
    source: &str,
) -> Vec<(Expr<'tree>, Expr<'tree>)> {
    let mut duplicates = Vec::new();
    for (i, &lhs) in conditions.iter().enumerate() {
        for &rhs in conditions.iter().skip(i + 1) {
            if lhs
                .syntax()
                .structurally_equivalent(rhs.syntax(), source, source)
            {
                duplicates.push((lhs, rhs));
            }
        }
    }
    duplicates
}

fn collect_if_conditions<'tree>(node: &If<'tree>) -> Vec<Expr<'tree>> {
    let mut conditions = Vec::with_capacity(2);
    if let Some(condition) = node.condition() {
        conditions.push(condition);
    }

    let mut alternative = node.alternative();
    while let Some(IfAlt::If(next_if)) = alternative {
        if let Some(condition) = next_if.condition() {
            conditions.push(condition);
        }
        alternative = next_if.alternative();
    }

    conditions
}

fn is_else_if(node: &If) -> bool {
    let raw = node.0;
    let Some(parent) = raw.parent() else {
        return false;
    };
    parent
        .child_by_field_name("alternative")
        .is_some_and(|alternative| alternative == raw)
}

#[cfg(test)]
mod tests {
    use super::{collect_if_conditions, find_duplicate_condition_pairs, is_else_if};
    use tolk_syntax::{If, NodeTraversalExt, TryFromNode, parse};

    fn with_first_if(code: &str, f: impl FnOnce(If<'_>, &str)) {
        let file = parse(code).expect("parse failed");
        let if_stmt = file
            .root_node()
            .traverse()
            .find_map(|n| If::try_from_node(n).ok())
            .expect("if statement not found");
        f(if_stmt, file.source.as_ref());
    }

    #[test]
    fn collects_if_chain_conditions() {
        with_first_if(
            r"
            fun main(a: int): int {
                if (a < 1) {
                    return 1;
                } else if (a > 4) {
                    return 2;
                } else if (a > 5) {
                    return 3;
                }
                return 4;
            }
            ",
            |if_stmt, source| {
                let conditions = collect_if_conditions(&if_stmt);
                assert_eq!(conditions.len(), 3);
                assert_eq!(find_duplicate_condition_pairs(&conditions, source).len(), 0);
            },
        );
    }

    #[test]
    fn finds_duplicate_if_conditions() {
        with_first_if(
            r"
            fun main(a: int): int {
                if (a < 1) {
                    return 1;
                } else if (a > 4) {
                    return 2;
                } else if (a > 4) {
                    return 3;
                }
                return 4;
            }
            ",
            |if_stmt, source| {
                let conditions = collect_if_conditions(&if_stmt);
                assert_eq!(find_duplicate_condition_pairs(&conditions, source).len(), 1);
            },
        );
    }

    #[test]
    fn detects_else_if_node() {
        with_first_if(
            r"
            fun main(a: int): int {
                if (a < 1) {
                    return 1;
                } else if (a > 4) {
                    return 2;
                }
                return 4;
            }
            ",
            |if_stmt, _| {
                assert!(!is_else_if(&if_stmt));

                let Some(tolk_syntax::IfAlt::If(nested_if)) = if_stmt.alternative() else {
                    panic!("expected else-if branch");
                };
                assert!(is_else_if(&nested_if));
            },
        );
    }

    #[test]
    fn finds_all_duplicate_pairs_in_if_conditions() {
        with_first_if(
            r"
            fun main(a: int): int {
                if (a > 7) {
                    return 1;
                } else if (a < 0) {
                    return 2;
                } else if (a > 7) {
                    return 3;
                } else if (a > 7) {
                    return 4;
                }
                return 5;
            }
            ",
            |if_stmt, source| {
                let conditions = collect_if_conditions(&if_stmt);
                // Pairs: (1st, 3rd), (1st, 4th), (3rd, 4th).
                assert_eq!(find_duplicate_condition_pairs(&conditions, source).len(), 3);
            },
        );
    }

    #[test]
    fn ignores_similar_conditions_with_different_literals() {
        with_first_if(
            r"
            fun main(a: int): int {
                if (((a + 1) * (a - 2)) > 10) {
                    return 1;
                } else if (((a + 1) * (a - 2)) > 11) {
                    return 2;
                }
                return 3;
            }
            ",
            |if_stmt, source| {
                let conditions = collect_if_conditions(&if_stmt);
                assert_eq!(find_duplicate_condition_pairs(&conditions, source).len(), 0);
            },
        );
    }
}
