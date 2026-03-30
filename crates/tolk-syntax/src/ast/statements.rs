use crate::ast::AstNode;
use crate::ast::expressions::{Expr, Ident, Match};
use crate::ast::node::{AstChildren, RawNode};
use crate::{AstNodeBytesKind, impl_ast_node};
use tree_sitter::Node;

#[derive(Clone, Copy, Debug)]
pub enum Stmt<'tree> {
    Block(Block<'tree>),
    If(If<'tree>),
    While(While<'tree>),
    Repeat(Repeat<'tree>),
    TryCatch(TryCatch<'tree>),
    Return(Return<'tree>),
    DoWhile(DoWhile<'tree>),
    Break(Break<'tree>),
    Continue(Continue<'tree>),
    Throw(Throw<'tree>),
    Assert(Assert<'tree>),
    Match(MatchStmt<'tree>),
    EmptyStmt(crate::ast::top_level::EmptyStmt<'tree>),
    ExprStmt(ExprStmt<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> Stmt<'tree> {
    #[must_use]
    pub fn text(&self, source: &'tree str) -> &'tree str {
        self.syntax().utf8_text(source.as_bytes()).unwrap_or("")
    }

    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            Stmt::Block(n) => n.0,
            Stmt::If(n) => n.0,
            Stmt::While(n) => n.0,
            Stmt::Repeat(n) => n.0,
            Stmt::TryCatch(n) => n.0,
            Stmt::Return(n) => n.0,
            Stmt::DoWhile(n) => n.0,
            Stmt::Break(n) => n.0,
            Stmt::Continue(n) => n.0,
            Stmt::Throw(n) => n.0,
            Stmt::Assert(n) => n.0,
            Stmt::Match(n) => n.0,
            Stmt::EmptyStmt(n) => n.0,
            Stmt::ExprStmt(n) => n.0,
            Stmt::Unmapped(n) => n.0,
        }
    }
}

impl<'t> From<Node<'t>> for Stmt<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"block_statement" => Stmt::Block(Block(node)),
            b"if_statement" => Stmt::If(If(node)),
            b"while_statement" => Stmt::While(While(node)),
            b"repeat_statement" => Stmt::Repeat(Repeat(node)),
            b"try_catch_statement" => Stmt::TryCatch(TryCatch(node)),
            b"return_statement" => Stmt::Return(Return(node)),
            b"do_while_statement" => Stmt::DoWhile(DoWhile(node)),
            b"break_statement" => Stmt::Break(Break(node)),
            b"continue_statement" => Stmt::Continue(Continue(node)),
            b"throw_statement" => Stmt::Throw(Throw(node)),
            b"assert_statement" => Stmt::Assert(Assert(node)),
            b"match_statement" => Stmt::Match(MatchStmt(node)),
            b"empty_statement" => Stmt::EmptyStmt(crate::ast::top_level::EmptyStmt(node)),
            b"expression_statement" => Stmt::ExprStmt(ExprStmt(node)),
            _ => Stmt::Unmapped(RawNode::new(node)),
        }
    }
}

impl<'tree> crate::ast::traits::TryFromNode<'tree> for Stmt<'tree> {
    type Error = crate::ast::traits::InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        let res = Self::from(node);
        match res {
            Stmt::Unmapped(_) => Err(crate::ast::traits::InvalidNodeKindError {
                expected: "statement",
                actual: node.kind().to_string(),
            }),
            _ => Ok(res),
        }
    }
}

impl<'tree> AstNode<'tree> for Stmt<'tree> {
    fn syntax(&self) -> Node<'tree> {
        self.syntax()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Block<'tree>(pub Node<'tree>);

impl_ast_node!(Block, "block_statement");

impl<'tree> Block<'tree> {
    #[must_use]
    pub fn stmts(&self) -> AstChildren<'tree, Stmt<'tree>> {
        AstChildren::new(self.0)
    }

    pub fn statements_including_comments(&self) -> Vec<Stmt<'tree>> {
        let mut cursor = self.0.walk();
        self.0
            .children(&mut cursor)
            .filter(|n| !matches!(n.kind_bytes(), b"{" | b"}"))
            .map(Into::into)
            .collect()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct If<'tree>(pub Node<'tree>);

impl_ast_node!(If, "if_statement");

#[derive(Clone, Copy, Debug)]
pub enum IfAlt<'tree> {
    If(If<'tree>),
    Block(Block<'tree>),
}

impl<'t> From<Node<'t>> for IfAlt<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"if_statement" => IfAlt::If(If(node)),
            b"block_statement" => IfAlt::Block(Block(node)),
            _ => panic!("Unexpected if statement alternative kind: {}", node.kind()),
        }
    }
}

impl<'tree> IfAlt<'tree> {
    #[must_use]
    pub const fn syntax(&self) -> &Node<'tree> {
        match self {
            IfAlt::If(if_stmt) => &if_stmt.0,
            IfAlt::Block(block_stmt) => &block_stmt.0,
        }
    }

    #[must_use]
    pub fn as_stmt(self) -> Stmt<'tree> {
        Stmt::from(*self.syntax())
    }
}

impl<'tree> If<'tree> {
    #[must_use]
    pub fn condition(&self) -> Option<Expr<'tree>> {
        self.0.field("condition")
    }

    #[must_use]
    pub fn body(&self) -> Option<Block<'tree>> {
        self.0.field("body")
    }

    #[must_use]
    pub fn alternative(&self) -> Option<IfAlt<'tree>> {
        self.0.field("alternative")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct While<'tree>(pub Node<'tree>);

impl_ast_node!(While, "while_statement");

impl<'tree> While<'tree> {
    #[must_use]
    pub fn condition(&self) -> Option<Expr<'tree>> {
        self.0.field("condition")
    }

    #[must_use]
    pub fn body(&self) -> Option<Block<'tree>> {
        self.0.field("body")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Repeat<'tree>(pub Node<'tree>);

impl_ast_node!(Repeat, "repeat_statement");

impl<'tree> Repeat<'tree> {
    #[must_use]
    pub fn count(&self) -> Option<Expr<'tree>> {
        self.0.field("count")
    }

    #[must_use]
    pub fn body(&self) -> Option<Block<'tree>> {
        self.0.field("body")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TryCatch<'tree>(pub Node<'tree>);

impl_ast_node!(TryCatch, "try_catch_statement");

impl<'tree> TryCatch<'tree> {
    #[must_use]
    pub fn body(&self) -> Option<Block<'tree>> {
        self.0.field("try_body")
    }

    #[must_use]
    pub fn catch(&self) -> Option<CatchClause<'tree>> {
        self.0.field("catch")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Return<'tree>(pub Node<'tree>);

impl_ast_node!(Return, "return_statement");

impl<'tree> Return<'tree> {
    #[must_use]
    pub fn expr(&self) -> Option<Expr<'tree>> {
        self.0.field("body")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DoWhile<'tree>(pub Node<'tree>);

impl_ast_node!(DoWhile, "do_while_statement");

impl<'tree> DoWhile<'tree> {
    #[must_use]
    pub fn body(&self) -> Option<Block<'tree>> {
        self.0.field("body")
    }

    #[must_use]
    pub fn condition(&self) -> Option<Expr<'tree>> {
        self.0.field("condition")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Break<'tree>(pub Node<'tree>);

impl_ast_node!(Break, "break_statement");

#[derive(Clone, Copy, Debug)]
pub struct Continue<'tree>(pub Node<'tree>);

impl_ast_node!(Continue, "continue_statement");

#[derive(Clone, Copy, Debug)]
pub struct Throw<'tree>(pub Node<'tree>);

impl_ast_node!(Throw, "throw_statement");

impl<'tree> Throw<'tree> {
    #[must_use]
    pub fn expr(&self) -> Option<Expr<'tree>> {
        self.0.field("excNo")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Assert<'tree>(pub Node<'tree>);

impl_ast_node!(Assert, "assert_statement");

impl<'tree> Assert<'tree> {
    #[must_use]
    pub fn condition(&self) -> Option<Expr<'tree>> {
        self.0.field("condition")
    }

    #[must_use]
    pub fn expr(&self) -> Option<Expr<'tree>> {
        self.0.field("excNo")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MatchStmt<'tree>(pub Node<'tree>);

impl_ast_node!(MatchStmt, "match_statement");

impl<'tree> MatchStmt<'tree> {
    pub fn expr(&self) -> Option<Match<'tree>> {
        let mut cursor = self.0.walk();
        self.0.children(&mut cursor).next().map(Into::into)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ExprStmt<'tree>(pub Node<'tree>);

impl_ast_node!(ExprStmt, "expression_statement");

impl<'tree> ExprStmt<'tree> {
    pub fn expr(&self) -> Option<Expr<'tree>> {
        let mut cursor = self.0.walk();
        self.0.children(&mut cursor).next().map(Into::into)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CatchClause<'tree>(pub Node<'tree>);

impl_ast_node!(CatchClause, "catch_clause");

impl<'tree> CatchClause<'tree> {
    #[must_use]
    pub fn catch_var1(&self) -> Option<Ident<'tree>> {
        self.0.field("catch_var1")
    }

    #[must_use]
    pub fn catch_var2(&self) -> Option<Ident<'tree>> {
        self.0.field("catch_var2")
    }

    #[must_use]
    pub fn body(&self) -> Option<Block<'tree>> {
        self.0.field("catch_body")
    }
}
