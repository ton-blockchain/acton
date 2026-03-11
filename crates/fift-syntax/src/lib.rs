//! Syntax analysis for the Fift language.
//!
//! This crate provides a high-level AST and parser for Fift, built on top of
//! [tree-sitter](https://tree-sitter.github.io/tree-sitter/).
//!
//! # Main entry points
//!
//! - [`parse`]: Parses Fift source code into a [`SourceFile`].
//! - [`SourceFile`]: Represents a parsed Fift file and provides access to the AST.
//! - [`AstNode`]: A trait implemented by all AST nodes.

pub mod ast;

pub use ast::expressions::*;
pub use ast::node::*;
pub use ast::top_level::*;
pub use ast::walker::*;
pub use ton_syntax::ast::{AstNode, HasName};
pub use ton_syntax::errors::{ParseError, ParseErrorKind, Span};
pub use ton_syntax::impl_ast_node;

use tree_sitter::{Language, Tree};

/// Parses the given Fift source code into a [`SourceFile`].
///
/// # Errors
///
/// Returns an error if the tree-sitter parser cannot be initialized.
pub fn parse(code: &str) -> anyhow::Result<SourceFile> {
    parse_with_old_tree(code, None)
}

/// Parses the given Fift source code into a [`SourceFile`], potentially reusing an existing tree.
///
/// # Errors
///
/// Returns an error if the tree-sitter parser cannot be initialized.
pub fn parse_with_old_tree(code: &str, old_tree: Option<&Tree>) -> anyhow::Result<SourceFile> {
    let tree = ton_syntax::parser::parse_with_old_tree(
        code,
        old_tree,
        tree_sitter_fift::LANGUAGE.into(),
        "Fift",
    )?;

    Ok(SourceFile {
        tree,
        source: code.into(),
    })
}

/// Returns the tree-sitter [`Language`] for Fift.
#[must_use]
pub fn language() -> Language {
    tree_sitter_fift::LANGUAGE.into()
}

#[cfg(test)]
mod tests {
    use crate::{DefinitionKind, TopLevel, parse};
    use ton_syntax::ast::{AstNode, HasName};

    #[test]
    fn api_smoke_test() -> anyhow::Result<()> {
        let source = "PROGRAM{\nDECLPROC foo\nfoo PROC:<{\n  1\n}>\nEND>c\n";
        let file = parse(source)?;

        assert!(!file.has_errors());
        let tops: Vec<_> = file.top_levels().collect();
        assert_eq!(tops.len(), 2);

        match tops[0] {
            TopLevel::Declaration(decl) => {
                let name = decl.name().expect("declaration should have name");
                assert_eq!(name.text(source), "foo");
            }
            _ => panic!("expected declaration"),
        }

        match tops[1] {
            TopLevel::Definition(def) => {
                let kind = def.kind().expect("definition kind should exist");
                match kind {
                    DefinitionKind::ProcDefinition(proc_def) => {
                        let name = proc_def.name().expect("proc should have name");
                        assert_eq!(name.text(source), "foo");
                        assert_eq!(proc_def.instructions().count(), 1);
                    }
                    _ => panic!("expected proc definition"),
                }
            }
            _ => panic!("expected definition"),
        }

        Ok(())
    }
}
