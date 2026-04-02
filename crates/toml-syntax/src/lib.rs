//! Syntax analysis for TOML.
//!
//! This crate provides a high-level AST and parser for TOML, built on top of
//! [tree-sitter](https://tree-sitter.github.io/tree-sitter/).

pub mod ast;
mod errors;

pub use ast::expressions::*;
pub use ast::node::*;
pub use ast::top_level::*;
pub use ast::traits::*;
pub use ast::walker::*;
pub use ton_syntax::errors::{ParseError, ParseErrorKind, Span};
pub use ton_syntax::impl_ast_node;

use tree_sitter::{Language, Tree};

/// Parses the given TOML source code into a [`SourceFile`].
///
/// # Errors
///
/// Returns an error if the tree-sitter parser cannot be initialized.
pub fn parse(code: &str) -> anyhow::Result<SourceFile> {
    parse_with_old_tree(code, None)
}

/// Parses the given TOML source code into a [`SourceFile`], potentially reusing an existing tree.
///
/// # Errors
///
/// Returns an error if the tree-sitter parser cannot be initialized.
pub fn parse_with_old_tree(code: &str, old_tree: Option<&Tree>) -> anyhow::Result<SourceFile> {
    let tree = ton_syntax::parser::parse_with_old_tree(
        code,
        old_tree,
        tree_sitter_toml_ng::LANGUAGE.into(),
        "TOML",
    )?;

    Ok(SourceFile {
        tree,
        source: code.into(),
    })
}

/// Returns the tree-sitter [`Language`] for TOML.
#[must_use]
pub fn language() -> Language {
    tree_sitter_toml_ng::LANGUAGE.into()
}

#[cfg(test)]
mod tests {
    use crate::{TopLevel, parse};

    #[test]
    fn api_smoke_test() -> anyhow::Result<()> {
        let source = r#"
title = "TOML Example"

[owner]
name = "Tom"

[[products]]
name = "Hammer"
"#;

        let file = parse(source)?;
        assert!(!file.has_errors());

        let tops: Vec<_> = file.top_levels().collect();
        assert_eq!(tops.len(), 3);

        match tops[0] {
            TopLevel::Pair(pair) => {
                let key = pair.key().expect("pair should have key");
                assert_eq!(key.text(source), "title");
            }
            _ => panic!("expected pair"),
        }

        match tops[1] {
            TopLevel::Table(table) => {
                let key = table.key().expect("table should have key");
                assert_eq!(key.text(source), "owner");
            }
            _ => panic!("expected table"),
        }

        match tops[2] {
            TopLevel::TableArrayElement(table) => {
                let key = table.key().expect("table array should have key");
                assert_eq!(key.text(source), "products");
            }
            _ => panic!("expected table array"),
        }

        Ok(())
    }
}
