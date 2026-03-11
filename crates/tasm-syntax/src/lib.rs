//! Syntax analysis for the TASM language.
//!
//! This crate provides a high-level AST and parser for TASM, built on top of
//! [tree-sitter](https://tree-sitter.github.io/tree-sitter/).
//!
//! # Main entry points
//!
//! - [`parse`]: Parses TASM source code into a [`SourceFile`].
//! - [`SourceFile`]: Represents a parsed TASM file and provides access to the AST.
//! - [`AstNode`]: A trait implemented by all AST nodes.

pub mod ast;

pub use ast::expressions::*;
pub use ast::node::*;
pub use ast::top_level::*;
pub use ast::walker::*;
pub use ton_syntax::ast::{
    AstNode, AstNodeBytesKind, HasName, HasTreeSitterKind, InvalidNodeKindError, TryFromNode,
};
pub use ton_syntax::errors::{ParseError, ParseErrorKind, Span};
pub use ton_syntax::impl_ast_node;

use tree_sitter::{Language, Tree};

/// Parses the given TASM source code into a [`SourceFile`].
///
/// # Errors
///
/// Returns an error if the tree-sitter parser cannot be initialized.
pub fn parse(code: &str) -> anyhow::Result<SourceFile> {
    parse_with_old_tree(code, None)
}

/// Parses the given TASM source code into a [`SourceFile`], potentially reusing an existing tree.
///
/// # Errors
///
/// Returns an error if the tree-sitter parser cannot be initialized.
pub fn parse_with_old_tree(code: &str, old_tree: Option<&Tree>) -> anyhow::Result<SourceFile> {
    let tree = ton_syntax::parser::parse_with_old_tree(
        code,
        old_tree,
        tree_sitter_tasm::LANGUAGE.into(),
        "TASM",
    )?;

    Ok(SourceFile {
        tree,
        source: code.into(),
    })
}

/// Returns the tree-sitter [`Language`] for TASM.
#[must_use]
pub fn language() -> Language {
    tree_sitter_tasm::LANGUAGE.into()
}

#[cfg(test)]
mod tests {
    use crate::{AstNode, TopLevel, parse};

    #[test]
    fn api_smoke_test() -> anyhow::Result<()> {
        let source = "PUSHINT_4 1\nref { SWAP }\n";
        let file = parse(source)?;

        assert!(!file.has_errors());
        let tops: Vec<_> = file.top_levels().collect();
        assert_eq!(tops.len(), 2);

        match tops[0] {
            TopLevel::Instruction(insn) => {
                let name = insn.name().expect("instruction should have name");
                assert_eq!(name.text(source), "PUSHINT_4");
            }
            _ => panic!("expected instruction"),
        }

        match tops[1] {
            TopLevel::ExplicitRef(r) => {
                let code = r.code().expect("ref should have code");
                assert!(code.instructions().is_some());
            }
            _ => panic!("expected explicit ref"),
        }

        Ok(())
    }
}
