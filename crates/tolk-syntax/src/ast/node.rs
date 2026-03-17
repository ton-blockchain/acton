use crate::ParseError;
use crate::ast::top_level::{Import, TopLevel};
use crate::errors::collect_errors;
use crate::{Func, GetMethod, language};
use std::sync::Arc;
use tree_sitter::Tree;

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

ton_syntax::impl_source_file_basics!(SourceFile, ParseError, collect_errors, language);

impl SourceFile {
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

pub use ton_syntax::ast::{AstChildren, RawNode, SyntaxNodeChildren};
