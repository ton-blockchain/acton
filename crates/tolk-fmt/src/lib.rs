pub mod comments;
pub mod common;
pub mod decls;
pub mod exprs;
pub mod pretty;
pub mod stmts;
pub mod types;

use std::collections::HashMap;
use std::rc::Rc;
use thiserror::Error;
use tree_sitter::Node;

pub use comments::{Comment, CommentKind, collect_comments};

#[derive(Clone, Copy, Debug)]
pub struct FormatOptions {
    pub width: usize,
    pub separate_import_groups: bool,
    pub range: Option<FormatRange>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormatRange {
    pub start: FormatPosition,
    pub end: FormatPosition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormatPosition {
    pub line: usize,
    pub character: usize,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            width: 100,
            separate_import_groups: false,
            range: None,
        }
    }
}

#[derive(Debug, Error)]
pub enum FormatError {
    #[error(transparent)]
    Parse(#[from] anyhow::Error),
    #[error("Cannot format code with syntax error")]
    SyntaxErrors,
    #[error("Failed to format source")]
    Source,
    #[error("Failed to render: {0}")]
    Render(#[source] std::io::Error),
    #[error("Invalid UTF-8: {0}")]
    InvalidUtf8(#[from] std::string::FromUtf8Error),
}

#[derive(Clone)]
pub struct Context<'tree> {
    pub code: Rc<str>,
    pub comments: HashMap<Node<'tree>, Vec<Comment<'tree>>>,
    pub options: FormatOptions,
}

pub fn format_source(source: &str, options: FormatOptions) -> Result<String, FormatError> {
    let source_file = tolk_syntax::parse(source)?;
    if source_file.has_errors() {
        return Err(FormatError::SyntaxErrors);
    }
    let root_node = source_file.tree.root_node();
    let comments_map = collect_comments(root_node);

    let ctx = Context {
        code: source.into(),
        comments: comments_map,
        options,
    };

    let doc = decls::print_source_file(&ctx, &source_file).ok_or(FormatError::Source)?;
    let mut out = Vec::new();
    doc.render(options.width, &mut out)
        .map_err(FormatError::Render)?;

    let res = String::from_utf8(out).map_err(FormatError::InvalidUtf8)?;

    // TODO: for some reason there are lines with whitespace only, trim manually for now
    Ok(res
        .lines()
        .map(|l| if l.trim().is_empty() { "" } else { l })
        .collect::<Vec<_>>()
        .join("\n"))
}
