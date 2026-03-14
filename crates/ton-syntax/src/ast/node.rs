use crate::ast::traits::AstNode;
use std::marker::PhantomData;
use tree_sitter::{Node, TreeCursor};

#[derive(Clone)]
pub struct SyntaxNodeChildren<'tree> {
    cursor: Option<TreeCursor<'tree>>,
    at_end: bool,
}

impl<'tree> SyntaxNodeChildren<'tree> {
    fn new(node: Node<'tree>) -> Self {
        let mut cursor = node.walk();
        cursor.goto_first_child();
        Self {
            cursor: Some(cursor),
            at_end: false,
        }
    }

    const fn empty() -> Self {
        Self {
            cursor: None,
            at_end: true,
        }
    }
}

impl<'tree> Iterator for SyntaxNodeChildren<'tree> {
    type Item = Node<'tree>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.at_end {
            return None;
        }

        let cursor = self.cursor.as_mut()?;
        let node = cursor.node();
        self.at_end = !cursor.goto_next_sibling();
        Some(node)
    }
}

/// An iterator over `SyntaxNode` children of a particular AST type `N`.
#[derive(Clone)]
pub struct AstChildren<'tree, N> {
    inner: SyntaxNodeChildren<'tree>,
    ph: PhantomData<N>,
}

impl<'tree, N> Default for AstChildren<'tree, N> {
    fn default() -> Self {
        Self {
            inner: SyntaxNodeChildren::empty(),
            ph: PhantomData,
        }
    }
}

impl<'tree, N> AstChildren<'tree, N> {
    /// Creates a new `AstChildren` iterator for the children of the given node.
    pub fn new(parent: Node<'tree>) -> Self {
        AstChildren {
            inner: SyntaxNodeChildren::new(parent),
            ph: PhantomData,
        }
    }
}

impl<'tree, N: AstNode<'tree>> Iterator for AstChildren<'tree, N> {
    type Item = N;

    fn next(&mut self) -> Option<N> {
        self.inner.find_map(|node| N::try_from_node(node).ok())
    }
}

impl<'tree, N: AstNode<'tree>> AstChildren<'tree, N> {
    /// Returns `true` if there are no children of type `N`.
    pub fn is_empty(&self) -> bool {
        let mut clone = self.clone();
        clone.next().is_none()
    }

    /// Returns the first child of type `N`, if any.
    pub fn first(&self) -> Option<N> {
        let mut clone = self.clone();
        clone.next()
    }
}

/// A wrapper around a [tree-sitter node](tree_sitter::Node) providing convenience methods.
#[derive(Clone, Copy, Debug)]
pub struct RawNode<'tree>(pub Node<'tree>);

impl<'tree> RawNode<'tree> {
    /// Creates a new `RawNode` from a tree-sitter node.
    #[must_use]
    pub const fn new(node: Node<'tree>) -> Self {
        Self(node)
    }

    /// Returns the underlying tree-sitter node.
    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        self.0
    }

    /// Returns the text content of the node from the source string.
    #[must_use]
    pub fn text(&self, source: &'tree str) -> &'tree str {
        self.0
            .utf8_text(source.as_bytes())
            .unwrap_or("<invalid utf8>")
    }
}

/// Implements common `SourceFile` boilerplate for syntax crates.
///
/// Expects the target type to have fields `tree: tree_sitter::Tree` and
/// `source: std::sync::Arc<str>`.
#[macro_export]
macro_rules! impl_source_file_basics {
    ($ty:ty, $parse_error:ty, $collect_errors:path, $language:path) => {
        impl $ty {
            /// Returns the root node of the tree.
            pub fn root_node(&'_ self) -> tree_sitter::Node<'_> {
                self.tree.root_node()
            }

            /// Returns `true` if the source file contains any syntax errors.
            pub fn has_errors(&self) -> bool {
                self.tree.root_node().has_error()
            }

            /// Collects and returns all syntax errors found in the file.
            pub fn errors(&self) -> Vec<$parse_error> {
                $collect_errors(&self.source, &self.tree, &$language())
            }

            /// Creates a new source file from a tree-sitter tree and source code.
            #[must_use]
            pub fn new(tree: tree_sitter::Tree, source: String) -> Self {
                Self {
                    tree,
                    source: std::sync::Arc::from(source),
                }
            }
        }

        impl std::cmp::PartialOrd for $ty {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }

        impl std::cmp::Ord for $ty {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                self.source.cmp(&other.source)
            }
        }

        impl std::cmp::Eq for $ty {}

        impl std::cmp::PartialEq for $ty {
            fn eq(&self, other: &Self) -> bool {
                self.source == other.source
            }
        }

        impl std::hash::Hash for $ty {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.source.hash(state);
            }
        }
    };
}
