use crate::ast::top_level::{Import, TopLevel};
use crate::ast::traits::AstNode;
use crate::errors::{ParseError, collect_errors};
use crate::{Func, GetMethod, language};
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::sync::Arc;
use tree_sitter::{Node, Tree, TreeCursor};

/// Represents a parsed Tolk source file.
///
/// It contains the [tree-sitter tree](tree_sitter::Tree) and the original source code.
#[derive(Debug, Clone)]
pub struct SourceFile {
    /// The tree-sitter tree representing the structure of the file.
    pub tree: Tree,
    /// The original source code of the file.
    pub source: Arc<str>,
}

impl SourceFile {
    /// Returns the root node of the tree.
    pub fn root_node(&'_ self) -> Node<'_> {
        self.tree.root_node()
    }

    /// Returns `true` if the source file contains any syntax errors.
    pub fn has_errors(&self) -> bool {
        self.tree.root_node().has_error()
    }

    /// Collects and returns all syntax errors found in the file.
    pub fn errors(&self) -> Vec<ParseError> {
        collect_errors(&self.source, &self.tree, &language())
    }
}

impl PartialOrd for SourceFile {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SourceFile {
    fn cmp(&self, other: &Self) -> Ordering {
        self.source.cmp(&other.source)
    }
}

impl Eq for SourceFile {}

impl PartialEq for SourceFile {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
    }
}

impl Hash for SourceFile {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.source.hash(state);
    }
}

impl SourceFile {
    /// Creates a new `SourceFile` from a tree-sitter tree and source code.
    #[must_use]
    pub fn new(tree: Tree, source: String) -> SourceFile {
        SourceFile {
            tree,
            source: Arc::from(source),
        }
    }

    /// Returns an iterator over all top-level declarations in the file.
    pub fn top_levels(&self) -> AstChildren<'_, TopLevel<'_>> {
        AstChildren::new(self.tree.root_node())
    }

    /// Returns an iterator over all imports in the file.
    pub fn imports(&self) -> impl Iterator<Item = Import<'_>> {
        self.top_levels().filter_map(|tl| match tl {
            TopLevel::Import(i) => Some(i),
            _ => None,
        })
    }

    /// Returns an iterator over all standalone functions in the file.
    pub fn functions(&self) -> impl Iterator<Item = Func<'_>> {
        self.top_levels().filter_map(|tl| match tl {
            TopLevel::Func(f) => Some(f),
            _ => None,
        })
    }

    /// Returns an iterator over all ge methods in the file.
    pub fn get_methods(&self) -> impl Iterator<Item = GetMethod<'_>> {
        self.top_levels().filter_map(|tl| match tl {
            TopLevel::GetMethod(m) => Some(m),
            _ => None,
        })
    }

    /// Finds the top-level declaration that covers the given range of bytes.
    ///
    /// # Parameters
    ///
    /// * `start` — The start byte of the range.
    /// * `end` — The end byte of the range.
    ///
    /// # Returns
    ///
    /// The top-level declaration that covers the given range of bytes, if any.
    pub fn find_top_levels_at(&self, start: usize, end: usize) -> Option<TopLevel<'_>> {
        self.top_levels().find(|decl| {
            let decl_start = decl.syntax().start_byte();
            let decl_end = decl.syntax().end_byte();

            // find declaration that covers `start..offset `
            decl_start <= start && start <= decl_end && end <= decl_end
        })
    }
}

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
