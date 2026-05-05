//! Syntax analysis for the TL-B language.
//!
//! This crate provides a high-level AST and parser for TL-B, built on top of
//! [tree-sitter](https://tree-sitter.github.io/tree-sitter/).

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

/// Parses the given TL-B source code into a [`SourceFile`].
///
/// # Errors
///
/// Returns an error if the tree-sitter parser cannot be initialized.
pub fn parse(code: &str) -> anyhow::Result<SourceFile> {
    parse_with_old_tree(code, None)
}

/// Parses the given TL-B source code into a [`SourceFile`], potentially reusing an existing tree.
///
/// # Errors
///
/// Returns an error if the tree-sitter parser cannot be initialized.
pub fn parse_with_old_tree(code: &str, old_tree: Option<&Tree>) -> anyhow::Result<SourceFile> {
    let tree = ton_syntax::parser::parse_with_old_tree(
        code,
        old_tree,
        tree_sitter_tlb::LANGUAGE.into(),
        "TL-B",
    )?;

    Ok(SourceFile {
        tree,
        source: code.into(),
    })
}

/// Returns the tree-sitter [`Language`] for TL-B.
#[must_use]
pub fn language() -> Language {
    tree_sitter_tlb::LANGUAGE.into()
}

#[cfg(test)]
mod tests {
    use crate::{AstNode, TopLevel, parse};

    #[test]
    fn api_smoke_test() -> anyhow::Result<()> {
        let source = "int_msg_info$0 ihr_disabled:Bool bounce:Bool = CommonMsgInfo;\n";
        let file = parse(source)?;

        assert!(!file.has_errors());
        let tops: Vec<_> = file.top_levels().collect();
        assert_eq!(tops.len(), 1);

        match tops[0] {
            TopLevel::Declaration(decl) => {
                let constructor = decl
                    .constructor()
                    .expect("declaration should have constructor");
                let name = constructor.name().expect("constructor should have name");
                assert_eq!(name.text(source), "int_msg_info");

                let combinator = decl
                    .combinator()
                    .expect("declaration should have combinator");
                assert_eq!(
                    combinator.name().expect("combinator name").text(source),
                    "CommonMsgInfo"
                );
            }
            TopLevel::Unmapped(_) => panic!("expected declaration"),
        }

        Ok(())
    }
}
