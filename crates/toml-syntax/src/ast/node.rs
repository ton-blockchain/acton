use crate::ParseError;
use crate::ast::top_level::{Document, TopLevel};
use crate::ast::traits::TryFromNode;
use crate::errors::collect_errors;
use crate::language;
use std::sync::Arc;
use tree_sitter::Tree;

/// Represents a parsed TOML source file.
#[derive(Debug, Clone)]
pub struct SourceFile {
    /// The tree-sitter tree representing the structure of the file.
    pub tree: Tree,
    /// The original source code of the file.
    pub source: Arc<str>,
}

ton_syntax::impl_source_file_basics!(SourceFile, ParseError, collect_errors, language);

impl SourceFile {
    /// Returns the `document` node, if present.
    pub fn document(&self) -> Option<Document<'_>> {
        let root = self.tree.root_node();
        Document::try_from_node(root).ok().or_else(|| {
            root.child(0)
                .and_then(|node| Document::try_from_node(node).ok())
        })
    }

    /// Returns an iterator over top-level items in the file.
    pub fn top_levels(&self) -> AstChildren<'_, TopLevel<'_>> {
        self.document().map(|doc| doc.items()).unwrap_or_default()
    }
}

pub use ton_syntax::ast::{AstChildren, RawNode, SyntaxNodeChildren};
