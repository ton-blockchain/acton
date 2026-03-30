use std::sync::OnceLock;
use tree_sitter::{Node, TreeCursor};

pub use ton_syntax::ast::PreorderTraverse;

pub trait NodeTraversalExt<'tree> {
    fn traverse(&self) -> PreorderTraverse<'tree>;

    /// Compares two syntax subtrees for structural equivalence.
    ///
    /// This comparison ignores comments and semicolon-related nodes (`;` and
    /// `empty_statement`) and compares identifier names by text.
    fn structurally_equivalent(&self, other: Node, self_source: &str, other_source: &str) -> bool;
}

impl<'tree> NodeTraversalExt<'tree> for Node<'tree> {
    fn traverse(&self) -> PreorderTraverse<'tree> {
        PreorderTraverse::new(self.walk())
    }

    fn structurally_equivalent(&self, other: Node, self_source: &str, other_source: &str) -> bool {
        structurally_equivalent(*self, other, self_source, other_source)
    }
}

/// Compares two syntax subtrees for structural equivalence, ignoring comments
/// and semicolon-related nodes (`;` and `empty_statement`), and comparing
/// all leaf node text verbatim
#[must_use]
pub fn structurally_equivalent(
    left: Node,
    right: Node,
    left_source: &str,
    right_source: &str,
) -> bool {
    let mut left_events = StructuralEvents::new(left);
    let mut right_events = StructuralEvents::new(right);

    loop {
        match (left_events.next(), right_events.next()) {
            (Some(left_event), Some(right_event)) => {
                if left_event.phase != right_event.phase {
                    return false;
                }

                let left_node = left_event.node;
                let right_node = right_event.node;
                if left_node.kind_id() != right_node.kind_id() {
                    return false;
                }

                if is_leaf_node(left_node)
                    && !same_node_text(left_node, right_node, left_source, right_source)
                {
                    return false;
                }
            }
            (None, None) => {
                return true;
            }
            _ => {
                return false;
            }
        }
    }
}

#[inline]
fn is_ignored_node(node: Node<'_>) -> bool {
    let ignored = kind_ids();
    matches!(
        node.kind_id(),
        id if id == ignored.comment || id == ignored.semicolon || id == ignored.empty_statement
    )
}

#[derive(Clone, Copy)]
struct KindIds {
    comment: u16,
    semicolon: u16,
    empty_statement: u16,
}

fn kind_ids() -> KindIds {
    static IDS: OnceLock<KindIds> = OnceLock::new();
    *IDS.get_or_init(|| {
        let language = crate::language();
        let ids = KindIds {
            comment: language.id_for_node_kind("comment", true),
            semicolon: language.id_for_node_kind(";", false),
            empty_statement: language.id_for_node_kind("empty_statement", true),
        };

        debug_assert_eq!(language.node_kind_for_id(ids.comment), Some("comment"));
        debug_assert_eq!(language.node_kind_for_id(ids.semicolon), Some(";"));
        debug_assert_eq!(
            language.node_kind_for_id(ids.empty_statement),
            Some("empty_statement")
        );
        ids
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TraversalPhase {
    Entering,
    Exiting,
}

#[derive(Clone, Copy)]
struct StructuralEvent<'tree> {
    phase: TraversalPhase,
    node: Node<'tree>,
}

struct StructuralEvents<'tree> {
    cursor: Option<TreeCursor<'tree>>,
    phase: TraversalPhase,
}

impl<'tree> StructuralEvents<'tree> {
    fn new(root: Node<'tree>) -> Self {
        Self {
            cursor: Some(root.walk()),
            phase: TraversalPhase::Entering,
        }
    }
}

impl<'tree> Iterator for StructuralEvents<'tree> {
    type Item = StructuralEvent<'tree>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let cursor = self.cursor.as_mut()?;
            let node = cursor.node();
            let phase = self.phase;
            match phase {
                TraversalPhase::Entering => {
                    if cursor.goto_first_child() {
                        self.phase = TraversalPhase::Entering;
                    } else {
                        self.phase = TraversalPhase::Exiting;
                    }
                }
                TraversalPhase::Exiting => {
                    if cursor.goto_next_sibling() {
                        self.phase = TraversalPhase::Entering;
                    } else if cursor.goto_parent() {
                        self.phase = TraversalPhase::Exiting;
                    } else {
                        self.cursor = None;
                    }
                }
            }

            if !is_ignored_node(node) {
                return Some(StructuralEvent { phase, node });
            }
        }
    }
}

#[inline]
fn is_leaf_node(node: Node<'_>) -> bool {
    node.child_count() == 0
}

#[inline]
fn same_node_text(left: Node<'_>, right: Node<'_>, left_source: &str, right_source: &str) -> bool {
    let left_start = left.start_byte();
    let left_end = left.end_byte();
    let right_start = right.start_byte();
    let right_end = right.end_byte();
    let left_len = left_end - left_start;
    let right_len = right_end - right_start;

    if left_len != right_len {
        return false;
    }

    let left_bytes = left_source.as_bytes();
    let right_bytes = right_source.as_bytes();
    if left_end > left_bytes.len() || right_end > right_bytes.len() {
        return false;
    }

    left_bytes[left_start..left_end] == right_bytes[right_start..right_end]
}

#[cfg(test)]
mod tests {
    use super::{NodeTraversalExt, structurally_equivalent};
    use crate::parse;

    #[test]
    fn structural_eq_ignores_comments_and_semicolons() -> anyhow::Result<()> {
        let left = parse(
            r"
            fun foo() {
                1 + 1;
            }
            ",
        )?;
        let right = parse(
            r"
            fun foo() {
                // formatting-only change
                1 + 1;;
            }
            ",
        )?;

        assert!(left.root_node().structurally_equivalent(
            right.root_node(),
            left.source.as_ref(),
            right.source.as_ref()
        ));

        Ok(())
    }

    #[test]
    fn structural_eq_ignores_optional_semicolon_tokens() -> anyhow::Result<()> {
        let left = parse("const X = 1;")?;
        let right = parse("const X = 1")?;

        assert!(left.root_node().structurally_equivalent(
            right.root_node(),
            left.source.as_ref(),
            right.source.as_ref()
        ));

        Ok(())
    }

    #[test]
    fn structural_eq_detects_real_structure_changes() -> anyhow::Result<()> {
        let left = parse("fun foo() { 1 + 1; }")?;
        let right = parse("fun foo() { 1 - 1; }")?;

        assert!(!left.root_node().structurally_equivalent(
            right.root_node(),
            left.source.as_ref(),
            right.source.as_ref()
        ));

        Ok(())
    }

    #[test]
    fn structural_eq_multiple_cases() -> anyhow::Result<()> {
        let cases = [
            ("fun foo() { 1 + 1; }", "fun foo() { // c\n1 + 1;; }", true),
            ("const X = 1;", "const X = 1", true),
            ("fun foo() { 1 + 1; }", "fun foo() { 1 - 1; }", false),
            (
                "fun foo() { if (a) { b(); } else { c(); } }",
                "fun foo() { while (a) { b(); } }",
                false,
            ),
        ];

        for (left_src, right_src, expected) in cases {
            let left = parse(left_src)?;
            let right = parse(right_src)?;
            let left_node = left.root_node();
            let right_node = right.root_node();

            let direct = structurally_equivalent(
                left_node,
                right_node,
                left.source.as_ref(),
                right.source.as_ref(),
            );

            assert_eq!(direct, expected);
            assert_eq!(
                left_node.structurally_equivalent(
                    right_node,
                    left.source.as_ref(),
                    right.source.as_ref(),
                ),
                direct
            );
        }

        Ok(())
    }

    #[test]
    fn structural_eq_distinguishes_identifier_names() -> anyhow::Result<()> {
        let left = parse("fun foo() { do_work(); }")?;
        let right = parse("fun foo() { other_work(); }")?;

        assert!(!left.root_node().structurally_equivalent(
            right.root_node(),
            left.source.as_ref(),
            right.source.as_ref(),
        ));

        Ok(())
    }

    #[test]
    fn structural_eq_distinguishes_number_literals() -> anyhow::Result<()> {
        let left = parse("fun foo(): int { return 1; }")?;
        let right = parse("fun foo(): int { return 2; }")?;

        assert!(!left.root_node().structurally_equivalent(
            right.root_node(),
            left.source.as_ref(),
            right.source.as_ref(),
        ));

        Ok(())
    }

    #[test]
    fn structural_eq_distinguishes_string_literals() -> anyhow::Result<()> {
        let left = parse(r#"fun foo(): slice { return "a"; }"#)?;
        let right = parse(r#"fun foo(): slice { return "b"; }"#)?;

        assert!(!left.root_node().structurally_equivalent(
            right.root_node(),
            left.source.as_ref(),
            right.source.as_ref(),
        ));

        Ok(())
    }
}
