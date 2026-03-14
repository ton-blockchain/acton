use crate::ParseError;
use crate::ast::top_level::{IncludeDirective, Program, TopLevel};
use crate::language;
use std::sync::Arc;
use tree_sitter::Tree;

/// Represents a parsed Fift source file.
#[derive(Debug, Clone)]
pub struct SourceFile {
    /// The tree-sitter tree representing the structure of the file.
    pub tree: Tree,
    /// The original source code of the file.
    pub source: Arc<str>,
}

ton_syntax::impl_source_file_basics!(SourceFile, ParseError, collect_errors, language);

impl SourceFile {
    /// Returns the `include_directive` node, if present.
    pub fn include_directive(&self) -> Option<IncludeDirective<'_>> {
        AstChildren::<IncludeDirective<'_>>::new(self.tree.root_node()).first()
    }

    /// Returns the `program` node, if present.
    pub fn program(&self) -> Option<Program<'_>> {
        AstChildren::<Program<'_>>::new(self.tree.root_node()).first()
    }

    /// Returns an iterator over declaration/definition items in program body.
    pub fn top_levels(&self) -> AstChildren<'_, TopLevel<'_>> {
        self.program().map(|p| p.items()).unwrap_or_default()
    }
}

pub use ton_syntax::ast::{AstChildren, RawNode, SyntaxNodeChildren};
use ton_syntax::errors::collect_errors;
