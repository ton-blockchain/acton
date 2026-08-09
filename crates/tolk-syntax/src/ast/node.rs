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
    /// Returns documentation declared for the whole file.
    #[must_use]
    pub fn documentation(&self) -> Option<String> {
        file_documentation(&self.tree, self.source.as_ref())
    }

    /// Returns an iterator over all top-level declarations in the file.
    #[must_use]
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
    #[must_use]
    pub fn find_top_levels_at(&self, start: usize, end: usize) -> Option<TopLevel<'_>> {
        self.top_levels().find(|decl| {
            let decl_start = decl.syntax().start_byte();
            let decl_end = decl.syntax().end_byte();

            // find declaration that covers `start..offset `
            decl_start <= start && start <= decl_end && end <= decl_end
        })
    }
}

fn file_documentation(tree: &Tree, source: &str) -> Option<String> {
    let root = tree.root_node();
    let mut cursor = root.walk();
    let mut children = root.named_children(&mut cursor);
    let first = children.next()?;
    let first_text = first.utf8_text(source.as_bytes()).ok()?;
    if first.kind() != "comment"
        || !source.get(..first.start_byte())?.trim().is_empty()
        || !is_documentation_comment(first_text)
    {
        return None;
    }

    let mut comments = vec![first_text];
    let mut last_end = first.end_byte();
    let mut boundary = source.len();
    for node in children {
        let gap = source.get(last_end..node.start_byte())?;
        let text = node.utf8_text(source.as_bytes()).ok()?;
        if node.kind() == "comment"
            && gap.matches('\n').count() <= 1
            && is_documentation_comment(text)
        {
            comments.push(text);
            last_end = node.end_byte();
            continue;
        }
        boundary = node.start_byte();
        break;
    }

    if boundary < source.len() && source.get(last_end..boundary)?.matches('\n').count() <= 1 {
        return None;
    }

    let documentation = comments
        .into_iter()
        .map(clean_comment)
        .collect::<Vec<_>>()
        .join("\n");
    (!documentation.is_empty()).then_some(documentation)
}

fn is_documentation_comment(comment: &str) -> bool {
    comment.starts_with("///") || comment.starts_with("/**")
}

/// Removes Tolk comment delimiters and one optional leading or trailing space.
#[must_use]
pub fn clean_comment(comment: &str) -> String {
    if let Some(comment) = comment
        .strip_prefix("///")
        .or_else(|| comment.strip_prefix("//"))
    {
        return comment.strip_prefix(' ').unwrap_or(comment).to_owned();
    }

    let comment = comment
        .strip_prefix("/**")
        .or_else(|| comment.strip_prefix("/*"))
        .and_then(|comment| comment.strip_suffix("*/"))
        .unwrap_or_default();
    let comment = comment.strip_prefix(' ').unwrap_or(comment);

    comment.strip_suffix(' ').unwrap_or(comment).to_owned()
}

pub use ton_syntax::ast::{AstChildren, RawNode, SyntaxNodeChildren};
