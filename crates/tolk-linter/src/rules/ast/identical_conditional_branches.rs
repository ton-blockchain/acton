use crate::rules::diagnostic::{Annotation, Diagnostic};
use crate::rules::violation::Violation;
use crate::{Checker, FixAvailability};
use tolk_macros::ViolationMetadata;
use tolk_resolver::AstNodeSpanExt;
use tolk_resolver::file_index::FileId;
use tolk_syntax::{If, IfAlt, NodeTraversalExt, Ternary};

/// ### What it does
/// Detects conditionals where both branches are structurally identical.
///
/// ### Why is this bad?
/// Such condition has no effect on behavior and usually indicates a bug or a
/// copy-paste mistake.
///
/// ### Example
/// ```tolk twoslash
/// if (flag) {
///     checkAccess();
/// } else {
///     checkAccess();
/// //  ^^^^^^^^^^^^^ E027: conditional branches are identical
/// }
///
/// val result = flag ? value + 1 : value + 1;
/// //                              ^^^^^^^^^ E027: conditional branches are identical
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct IdenticalConditionalBranches;

impl Violation for IdenticalConditionalBranches {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::None;

    fn message(&self) -> String {
        "conditional branches are identical".to_string()
    }
}

pub fn check_if(checker: &mut Checker, file_id: FileId, node: &If) -> Option<()> {
    let file = checker.file_db.get_by_id(file_id)?;
    if !has_identical_if_else_branches(file.source().source.as_ref(), node) {
        return None;
    }

    let if_body = node.body()?;
    let else_branch = match node.alternative()? {
        IfAlt::Block(else_block) => else_block,
        IfAlt::If(_) => return None,
    };

    let diagnostic = Diagnostic::warning_for(file_id, IdenticalConditionalBranches)
        .with_annotations(vec![
            Annotation {
                span: if_body.span(),
                message: Some("first branch is here".to_string()),
                is_primary: false,
                tags: vec![],
            },
            Annotation {
                span: else_branch.span(),
                message: Some("this branch duplicates the `if` body".to_string()),
                is_primary: true,
                tags: vec![],
            },
        ])
        .with_help("remove `else` or make branches semantically different");
    checker.emit_diagnostic(diagnostic);
    Some(())
}

pub fn check_ternary(checker: &mut Checker, file_id: FileId, node: &Ternary) -> Option<()> {
    let file = checker.file_db.get_by_id(file_id)?;
    if !has_identical_ternary_branches(file.source().source.as_ref(), node) {
        return None;
    }

    let consequence = node.consequence()?;
    let alternative = node.alternative()?;
    let diagnostic = Diagnostic::warning_for(file_id, IdenticalConditionalBranches)
        .with_annotations(vec![
            Annotation {
                span: consequence.span(),
                message: Some("first branch is here".to_string()),
                is_primary: false,
                tags: vec![],
            },
            Annotation {
                span: alternative.span(),
                message: Some("this branch duplicates the other ternary branch".to_string()),
                is_primary: true,
                tags: vec![],
            },
        ])
        .with_help("simplify ternary or make branches semantically different");
    checker.emit_diagnostic(diagnostic);
    Some(())
}

fn has_identical_if_else_branches(source: &str, node: &If) -> bool {
    let Some(body) = node.body() else {
        return false;
    };
    let Some(IfAlt::Block(else_branch)) = node.alternative() else {
        return false;
    };

    body.0
        .structurally_equivalent(else_branch.0, source, source)
}

fn has_identical_ternary_branches(source: &str, node: &Ternary) -> bool {
    let Some(consequence) = node.consequence() else {
        return false;
    };
    let Some(alternative) = node.alternative() else {
        return false;
    };

    consequence
        .syntax()
        .structurally_equivalent(alternative.syntax(), source, source)
}

#[cfg(test)]
mod tests {
    use super::{has_identical_if_else_branches, has_identical_ternary_branches};
    use tolk_syntax::{If, NodeTraversalExt, Ternary, TryFromNode, parse};

    fn has_identical_branches_for_first_if(code: &str) -> bool {
        let file = parse(code).expect("parse failed");
        let if_stmt = file
            .root_node()
            .traverse()
            .find_map(|n| If::try_from_node(n).ok())
            .expect("if statement not found");
        has_identical_if_else_branches(file.source.as_ref(), &if_stmt)
    }

    fn has_identical_branches_for_first_ternary(code: &str) -> bool {
        let file = parse(code).expect("parse failed");
        let ternary = file
            .root_node()
            .traverse()
            .find_map(|n| Ternary::try_from_node(n).ok())
            .expect("ternary not found");
        has_identical_ternary_branches(file.source.as_ref(), &ternary)
    }

    #[test]
    fn detects_identical_branches_ignoring_comments_and_semicolons() {
        assert!(has_identical_branches_for_first_if(
            r"
            fun f(flag: bool) {
                if (flag) {
                    // comment
                    do_work();;
                } else {
                    do_work();
                }
            }
            ",
        ));
    }

    #[test]
    fn ignores_non_identical_or_missing_else() {
        assert!(!has_identical_branches_for_first_if(
            r"
            fun f(flag: bool) {
                if (flag) {
                    do_work();
                } else {
                    other_work();
                }
            }
            ",
        ));

        assert!(!has_identical_branches_for_first_if(
            r"
            fun f(flag: bool) {
                if (flag) {
                    do_work();
                }
            }
            ",
        ));
    }

    #[test]
    fn detects_identical_ternary_branches() {
        assert!(has_identical_branches_for_first_ternary(
            r"
            fun f(flag: bool, value: int): int {
                return flag ? (value + 1 /* comment */) : (value + 1);
            }
            ",
        ));
    }

    #[test]
    fn ignores_non_identical_ternary_branches() {
        assert!(!has_identical_branches_for_first_ternary(
            r"
            fun f(flag: bool, left: int, right: int): int {
                return flag ? left : right;
            }
            ",
        ));
    }
}
