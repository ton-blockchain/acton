use crate::ParseError;
use crate::ast::top_level::{Program, TopLevel};
use crate::language;
use std::sync::Arc;
use tree_sitter::Tree;

/// Represents a parsed TL-B source file.
#[derive(Debug, Clone)]
pub struct SourceFile {
    /// The tree-sitter tree representing the structure of the file.
    pub tree: Tree,
    /// The original source code of the file.
    pub source: Arc<str>,
}

ton_syntax::impl_source_file_basics!(SourceFile, ParseError, collect_errors, language);

impl SourceFile {
    /// Returns the `program` node, if present.
    pub fn program(&self) -> Option<Program<'_>> {
        Program::try_from_node(self.tree.root_node()).ok()
    }

    /// Returns an iterator over top-level declarations in the file.
    pub fn top_levels(&self) -> AstChildren<'_, TopLevel<'_>> {
        self.program().map(|p| p.items()).unwrap_or_default()
    }
}

use ton_syntax::ast::TryFromNode;
pub use ton_syntax::ast::{AstChildren, RawNode, SyntaxNodeChildren};
use ton_syntax::errors::collect_errors;
