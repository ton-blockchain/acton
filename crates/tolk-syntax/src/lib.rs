//! Syntax analysis for the Tolk language.
//!
//! This crate provides a high-level AST and parser for the Tolk language, built on top of
//! [tree-sitter](https://tree-sitter.github.io/tree-sitter/).
//!
//! # Main entry points
//!
//! - [`parse`]: Parses Tolk source code into a [`SourceFile`].
//! - [`SourceFile`]: Represents a parsed Tolk file and provides access to the AST.
//! - [`AstNode`]: A trait implemented by all AST nodes.
//!
//! # Example
//!
//! ```rust
//! use tolk_syntax::parse;
//!
//! let code = "import \"stdlib.tolk\";\n\nfun main() {}";
//! let source_file = parse(code).expect("Failed to parse");
//!
//! for top_level in source_file.top_levels() {
//!     println!("{:?}", top_level);
//! }
//! ```

pub mod ast;
mod errors;

pub use ast::expressions::*;
pub use ast::node::*;
pub use ast::statements::*;
pub use ast::top_level::*;
pub use ast::traits::*;
pub use ast::traversal::*;
pub use ast::types::*;
pub use ast::walker::*;
pub use ton_syntax::errors::{ParseError, ParseErrorKind, Span};
pub use ton_syntax::impl_ast_node;
use tree_sitter::{Language, Tree};

/// Parses the given Tolk source code into a [`SourceFile`].
///
/// # Errors
///
/// Returns an error if the tree-sitter parser cannot be initialized.
/// Note that syntax errors in the source code do not cause this function to return `Err`;
/// instead, use [`SourceFile::has_errors`] and [`SourceFile::errors`] to check for syntax errors.
pub fn parse(code: &str) -> anyhow::Result<SourceFile> {
    parse_with_old_tree(code, None)
}

/// Parses the given Tolk source code into a [`SourceFile`], potentially reusing an existing tree.
///
/// # Errors
///
/// Returns an error if the tree-sitter parser cannot be initialized.
pub fn parse_with_old_tree(code: &str, old_tree: Option<&Tree>) -> anyhow::Result<SourceFile> {
    let tree = ton_syntax::parser::parse_with_old_tree(
        code,
        old_tree,
        tree_sitter_tolk::LANGUAGE.into(),
        "Tolk",
    )?;
    Ok(SourceFile {
        tree,
        source: code.into(),
    })
}

/// Returns the tree-sitter [`Language`] for Tolk.
#[must_use]
pub fn language() -> Language {
    tree_sitter_tolk::LANGUAGE.into()
}

#[cfg(test)]
mod tests {
    use crate::{
        AnnotationName, AstNode, Func, FunctionLike, HasName, HasTreeSitterKind, Struct, Walker,
        ast, match_parents, parse,
    };

    /// This test does not assert much and instead just shows off the crate's API.
    #[test]
    fn api_walkthrough() -> anyhow::Result<()> {
        let source_code = "
            fun foo() {
                1 + 1;
            }
        ";

        // `parse` is the main entry point.
        // It returns an `anyhow::Result<SourceFile>`.
        let source_file = parse(source_code)?;

        // We can check for errors.
        assert!(!source_file.has_errors());
        assert!(source_file.errors().is_empty());

        // `SourceFile` is the root of the syntax tree. We can iterate file's items (top-levels).
        // Let's fetch the `foo` function.
        let mut func = None;
        for top_level in source_file.top_levels() {
            match top_level {
                ast::TopLevel::Func(f) => func = Some(f),
                _ => continue,
            }
        }
        let func = func.expect("function foo not found");

        // Each AST node has a bunch of getters for children. All getters return
        // `Option's to account for incomplete code. Some getters are common
        // for several kinds of node, provided by traits like `HasName`.
        let name_ident: Option<ast::Ident> = func.name();
        let name_ident = name_ident.expect("function should have a name");
        assert_eq!(name_ident.text(source_code), "foo");

        // Let's get the function body.
        let body = func.body().expect("function should have a body");
        let block = match body {
            ast::FuncBody::Block(b) => b,
            _ => panic!("expected block statement"),
        };

        // We can iterate over statements in the block.
        let mut stmt_iter = block.stmts();
        let stmt = stmt_iter
            .next()
            .expect("block should have at least one statement");

        // Statements are also enums.
        let expr_stmt = match stmt {
            ast::Stmt::ExprStmt(s) => s,
            _ => panic!("expected expression statement"),
        };

        let expr = expr_stmt
            .expr()
            .expect("expression statement should have an expression");

        // Expressions are enums too.
        let bin_expr = match expr {
            ast::Expr::Bin(e) => e,
            _ => panic!("expected binary operator"),
        };

        // Besides the "typed" AST API, there's the underlying tree-sitter node.
        // To switch from AST to tree-sitter, call `.syntax()` method:
        let node = bin_expr.syntax();

        // Note how `expr` and `bin_expr` are in fact the same node underneath:
        assert_eq!(node, expr.syntax());

        // The tree-sitter node has a kind:
        assert_eq!(node.kind(), "binary_operator");

        // And text range (start and end bytes):
        assert_eq!(node.start_byte(), source_code.find("1 + 1").unwrap());

        // You can get node's text using `utf8_text`:
        let text = node.utf8_text(source_code.as_bytes())?;
        assert_eq!(text, "1 + 1");

        // There's a bunch of traversal methods on `tree_sitter::Node`:
        assert_eq!(node.parent(), Some(expr_stmt.syntax()));
        assert_eq!(node.child(0).map(|c| c.kind()), Some("number_literal"));

        // To go from tree-sitter to AST, we can use `TopLevel::from`, `Statement::from`, etc.
        // or the `TryFromNode` trait.
        use crate::TryFromNode;
        let _expr =
            ast::Expr::try_from_node(node).expect("should be able to cast back to expression");

        // We can also use tree-sitter's walk() for more fine-grained iteration:
        let mut cursor = node.walk();
        assert!(cursor.goto_first_child());
        assert_eq!(cursor.node().kind(), "number_literal");
        assert!(cursor.goto_next_sibling());
        // In this grammar, the operator might be an anonymous node or a named one.
        // Let's just check its text to be sure.
        assert_eq!(cursor.node().utf8_text(source_code.as_bytes())?, "+");

        // Finally, `match_parents!` is a powerful macro for upward navigation.
        let parent_func = match_parents!(node, Func(...));
        assert_eq!(parent_func.map(|f| f.syntax()), Some(func.syntax()));

        Ok(())
    }

    #[test]
    fn dotted_annotation_name_is_exposed_via_ast() -> anyhow::Result<()> {
        let source = r"
            @abi.minimalMsgValue(1)
            struct Message {}
        ";

        let file = parse(source)?;
        assert!(!file.has_errors());

        let strukt = file
            .top_levels()
            .find_map(|top_level| match top_level {
                ast::TopLevel::Struct(s) => Some(s),
                _ => None,
            })
            .expect("struct should exist");

        let annotation = strukt
            .annotations()
            .expect("struct should have annotations")
            .annotations()
            .next()
            .expect("annotation should exist");

        let name = annotation.name().expect("annotation should have a name");
        assert_eq!(name.text(source), "abi.minimalMsgValue");

        Ok(())
    }

    #[test]
    fn dotted_annotation_name_without_compact_dot_produces_parse_error() -> anyhow::Result<()> {
        let source = r"
            @abi . minimalMsgValue(1)
            struct Message {}
        ";

        let file = parse(source)?;
        assert!(file.has_errors());

        Ok(())
    }

    #[test]
    fn walker_visits_annotation_name_node() -> anyhow::Result<()> {
        #[derive(Default)]
        struct AnnotationNameCollector {
            names: Vec<String>,
            source: String,
        }

        impl<'tree> Walker<'tree> for AnnotationNameCollector {
            type Result = ();

            fn walk_annotation_name(&mut self, node: &AnnotationName<'tree>) -> Self::Result {
                self.names.push(node.text(&self.source).to_string());
            }

            fn default_result(&self) -> Self::Result {}
        }

        impl AnnotationNameCollector {
            fn visit_with_source<'tree>(&mut self, source: &str, strukt: &Struct<'tree>) {
                self.source = source.to_string();
                self.walk_struct(strukt);
            }
        }

        let source = r"
            @custom.flag
            struct Message {}
        ";

        let file = parse(source)?;
        assert!(!file.has_errors());

        let strukt = file
            .top_levels()
            .find_map(|top_level| match top_level {
                ast::TopLevel::Struct(s) => Some(s),
                _ => None,
            })
            .expect("struct should exist");

        let mut collector = AnnotationNameCollector::default();
        collector.visit_with_source(source, &strukt);

        assert_eq!(collector.names, vec!["custom.flag"]);

        Ok(())
    }
}
