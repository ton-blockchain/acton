pub mod comments;
pub mod common;
pub mod decls;
pub mod exprs;
pub mod pretty;
pub mod stmts;
pub mod types;

use anyhow::anyhow;
use std::collections::HashMap;
use std::rc::Rc;
use tree_sitter::Node;

pub use comments::{Comment, CommentKind, collect_comments};

#[derive(Clone, Copy, Debug)]
pub struct FormatOptions {
    pub width: usize,
    pub separate_import_groups: bool,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            width: 100,
            separate_import_groups: false,
        }
    }
}

#[derive(Clone)]
pub struct Context<'tree> {
    pub code: Rc<str>,
    pub comments: HashMap<Node<'tree>, Vec<Comment<'tree>>>,
    pub options: FormatOptions,
}

pub fn format_source(source: &str, options: FormatOptions) -> anyhow::Result<String> {
    let source_file = tolk_syntax::parse(source)?;
    if source_file.has_errors() {
        anyhow::bail!("Cannot format code with syntax error");
    }
    let root_node = source_file.tree.root_node();
    let comments_map = collect_comments(root_node);

    let ctx = Context {
        code: source.into(),
        comments: comments_map,
        options,
    };

    let doc = decls::print_source_file(&ctx, &source_file)
        .ok_or_else(|| anyhow!("Failed to format source"))?;
    let mut out = Vec::new();
    doc.render(options.width, &mut out)
        .map_err(|e| anyhow!("Failed to render: {e}"))?;

    let res = String::from_utf8(out).map_err(|e| anyhow!("Invalid UTF-8: {e}"))?;

    // TODO: for some reason there are lines with whitespace only, trim manually for now
    Ok(res
        .lines()
        .map(|l| if l.trim().is_empty() { "" } else { l })
        .collect::<Vec<_>>()
        .join("\n"))
}
