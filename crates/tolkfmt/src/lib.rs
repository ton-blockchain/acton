pub mod comments;
pub mod common;
pub mod decls;
pub mod exprs;
pub mod stmts;
pub mod types;

use anyhow::anyhow;
use std::collections::HashMap;
use std::rc::Rc;
use tolk_ast::SourceFile;
use tree_sitter::Node;

pub use comments::{Comment, CommentKind, collect_comments};

#[derive(Clone)]
pub struct Context<'tree> {
    pub code: Rc<str>,
    pub comments: HashMap<Node<'tree>, Vec<Comment<'tree>>>,
}

pub fn format_source(source: &str, width: usize) -> anyhow::Result<String> {
    let tree = tolk_parser::parser::parse(source)?;
    let source_file = SourceFile {
        tree,
        source: source.into(),
    };

    let root_node = source_file.tree.root_node();
    if root_node.has_error() {
        anyhow::bail!("Cannot format code with syntax error");
    }
    let comments_map = collect_comments(root_node);

    let ctx = Context {
        code: source.into(),
        comments: comments_map,
    };

    let doc = decls::print_source_file(&ctx, &source_file)
        .ok_or_else(|| anyhow!("Failed to format source"))?;
    let mut out = Vec::new();
    doc.render(width, &mut out)
        .map_err(|e| anyhow!("Failed to render: {}", e))?;

    let res = String::from_utf8(out).map_err(|e| anyhow!("Invalid UTF-8: {}", e))?;

    // TODO: for some reason there are lines with whitespace only, trim manually for now
    Ok(res
        .lines()
        .map(|l| if l.trim().is_empty() { "" } else { l })
        .collect::<Vec<_>>()
        .join("\n"))
}
