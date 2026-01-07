use crate::expressions::{Expression, Ident, MatchExpression};
use crate::node::{NodeFieldExt, RawNode};
use crate::types::Type;
use tree_sitter::Node;

#[derive(Clone, Copy, Debug)]
pub enum Statement<'tree> {
    BlockStatement(BlockStatement<'tree>),
    IfStatement(IfStatement<'tree>),
    WhileStatement(WhileStatement<'tree>),
    RepeatStatement(RepeatStatement<'tree>),
    TryCatchStatement(TryCatchStatement<'tree>),
    ReturnStatement(ReturnStatement<'tree>),
    LocalVarsDeclaration(LocalVarsDeclaration<'tree>),
    DoWhileStatement(DoWhileStatement<'tree>),
    BreakStatement(BreakStatement<'tree>),
    ContinueStatement(ContinueStatement<'tree>),
    ThrowStatement(ThrowStatement<'tree>),
    AssertStatement(AssertStatement<'tree>),
    MatchStatement(MatchStatement<'tree>),
    EmptyStatement(crate::top_level::EmptyStatement<'tree>),
    ExpressionStatement(ExpressionStatement<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> Statement<'tree> {
    pub fn text(&self, source: &'tree str) -> &'tree str {
        self.raw_node().utf8_text(source.as_bytes()).unwrap_or("")
    }

    pub fn raw_node(&self) -> Node<'tree> {
        match self {
            Statement::BlockStatement(n) => n.0,
            Statement::IfStatement(n) => n.0,
            Statement::WhileStatement(n) => n.0,
            Statement::RepeatStatement(n) => n.0,
            Statement::TryCatchStatement(n) => n.0,
            Statement::ReturnStatement(n) => n.0,
            Statement::LocalVarsDeclaration(n) => n.0,
            Statement::DoWhileStatement(n) => n.0,
            Statement::BreakStatement(n) => n.0,
            Statement::ContinueStatement(n) => n.0,
            Statement::ThrowStatement(n) => n.0,
            Statement::AssertStatement(n) => n.0,
            Statement::MatchStatement(n) => n.0,
            Statement::EmptyStatement(n) => n.0,
            Statement::ExpressionStatement(n) => n.0,
            Statement::Unmapped(n) => n.0,
        }
    }
}

impl<'t> From<Node<'t>> for Statement<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind() {
            "block_statement" => Statement::BlockStatement(BlockStatement(node)),
            "if_statement" => Statement::IfStatement(IfStatement(node)),
            "while_statement" => Statement::WhileStatement(WhileStatement(node)),
            "repeat_statement" => Statement::RepeatStatement(RepeatStatement(node)),
            "try_catch_statement" => Statement::TryCatchStatement(TryCatchStatement(node)),
            "return_statement" => Statement::ReturnStatement(ReturnStatement(node)),
            "local_vars_declaration" => Statement::LocalVarsDeclaration(LocalVarsDeclaration(node)),
            "do_while_statement" => Statement::DoWhileStatement(DoWhileStatement(node)),
            "break_statement" => Statement::BreakStatement(BreakStatement(node)),
            "continue_statement" => Statement::ContinueStatement(ContinueStatement(node)),
            "throw_statement" => Statement::ThrowStatement(ThrowStatement(node)),
            "assert_statement" => Statement::AssertStatement(AssertStatement(node)),
            "match_statement" => Statement::MatchStatement(MatchStatement(node)),
            "empty_statement" => Statement::EmptyStatement(crate::top_level::EmptyStatement(node)),
            "expression_statement" => Statement::ExpressionStatement(ExpressionStatement(node)),
            _ => Statement::Unmapped(RawNode::new(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BlockStatement<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for BlockStatement<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> BlockStatement<'tree> {
    pub fn statements(&self) -> Vec<Statement<'tree>> {
        let mut cursor = self.0.walk();
        self.0
            .children(&mut cursor)
            .filter(|n| !matches!(n.kind(), "{" | "}"))
            .map(Into::into)
            .collect()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct IfStatement<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for IfStatement<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum IfStatementAlternative<'tree> {
    IfStatement(IfStatement<'tree>),
    BlockStatement(BlockStatement<'tree>),
}

impl<'t> From<Node<'t>> for IfStatementAlternative<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind() {
            "if_statement" => IfStatementAlternative::IfStatement(IfStatement(node)),
            "block_statement" => IfStatementAlternative::BlockStatement(BlockStatement(node)),
            _ => panic!("Unexpected if statement alternative kind: {}", node.kind()),
        }
    }
}

impl<'tree> IfStatementAlternative<'tree> {
    pub fn raw_node(&self) -> &Node<'tree> {
        match self {
            IfStatementAlternative::IfStatement(if_stmt) => &if_stmt.0,
            IfStatementAlternative::BlockStatement(block_stmt) => &block_stmt.0,
        }
    }
}

impl<'tree> IfStatement<'tree> {
    pub fn condition(&self) -> Option<Expression<'tree>> {
        self.0.field("condition")
    }

    pub fn body(&self) -> Option<BlockStatement<'tree>> {
        self.0.field("body")
    }

    pub fn alternative(&self) -> Option<IfStatementAlternative<'tree>> {
        self.0.field("alternative")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct WhileStatement<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for WhileStatement<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> WhileStatement<'tree> {
    pub fn condition(&self) -> Option<Expression<'tree>> {
        self.0.field("condition")
    }

    pub fn body(&self) -> Option<BlockStatement<'tree>> {
        self.0.field("body")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RepeatStatement<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for RepeatStatement<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> RepeatStatement<'tree> {
    pub fn count(&self) -> Option<Expression<'tree>> {
        self.0.field("count")
    }

    pub fn body(&self) -> Option<BlockStatement<'tree>> {
        self.0.field("body")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TryCatchStatement<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for TryCatchStatement<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> TryCatchStatement<'tree> {
    pub fn body(&self) -> Option<BlockStatement<'tree>> {
        self.0.field("try_body")
    }

    pub fn catch(&self) -> Option<CatchClause<'tree>> {
        self.0.field("catch")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ReturnStatement<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for ReturnStatement<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> ReturnStatement<'tree> {
    pub fn expr(&self) -> Option<Expression<'tree>> {
        self.0.field("body")
    }

    pub fn raw_node(&self) -> &Node<'tree> {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VarKind {
    Var,
    Val,
}

impl VarKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            VarKind::Var => "var",
            VarKind::Val => "val",
        }
    }
}

impl<'tree> From<Node<'tree>> for VarKind {
    fn from(node: Node<'tree>) -> Self {
        match node.kind() {
            "val" => VarKind::Val,
            _ => VarKind::Var,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LocalVarsDeclaration<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for LocalVarsDeclaration<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> LocalVarsDeclaration<'tree> {
    pub fn kind(&self) -> VarKind {
        self.0
            .field::<Node>("kind")
            .map(VarKind::from)
            .unwrap_or(VarKind::Var)
    }

    pub fn lhs(&self) -> Option<VarDeclarationLhs<'tree>> {
        self.0.field("lhs")
    }

    pub fn assigned_val(&self) -> Option<Expression<'tree>> {
        self.0.field("assigned_val")
    }

    pub fn raw_node(&self) -> &Node<'tree> {
        &self.0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DoWhileStatement<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for DoWhileStatement<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> DoWhileStatement<'tree> {
    pub fn body(&self) -> Option<BlockStatement<'tree>> {
        self.0.field("body")
    }

    pub fn condition(&self) -> Option<Expression<'tree>> {
        self.0.field("condition")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BreakStatement<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for BreakStatement<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ContinueStatement<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for ContinueStatement<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ThrowStatement<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for ThrowStatement<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> ThrowStatement<'tree> {
    pub fn expression(&self) -> Option<Expression<'tree>> {
        self.0.field("excNo")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AssertStatement<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for AssertStatement<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> AssertStatement<'tree> {
    pub fn condition(&self) -> Option<Expression<'tree>> {
        self.0.field("condition")
    }

    pub fn expression(&self) -> Option<Expression<'tree>> {
        self.0.field("excNo")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MatchStatement<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for MatchStatement<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> MatchStatement<'tree> {
    pub fn expression(&self) -> Option<MatchExpression<'tree>> {
        let mut cursor = self.0.walk();
        self.0.children(&mut cursor).next().map(Into::into)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ExpressionStatement<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for ExpressionStatement<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> ExpressionStatement<'tree> {
    pub fn expression(&self) -> Option<Expression<'tree>> {
        let mut cursor = self.0.walk();
        self.0.children(&mut cursor).next().map(Into::into)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CatchClause<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for CatchClause<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> CatchClause<'tree> {
    pub fn catch_var1(&self) -> Option<Ident<'tree>> {
        self.0.field("catch_var1")
    }

    pub fn catch_var2(&self) -> Option<Ident<'tree>> {
        self.0.field("catch_var2")
    }

    pub fn body(&self) -> Option<BlockStatement<'tree>> {
        self.0.field("catch_body")
    }

    pub fn raw_node(&self) -> &Node<'tree> {
        &self.0
    }
}

#[derive(Clone, Copy, Debug)]
pub enum VarDeclarationLhs<'tree> {
    TupleVarsDeclaration(TupleVarsDeclaration<'tree>),
    TensorVarsDeclaration(TensorVarsDeclaration<'tree>),
    VarDeclaration(VarDeclaration<'tree>),
}

impl<'t> From<Node<'t>> for VarDeclarationLhs<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind() {
            "tuple_vars_declaration" => {
                VarDeclarationLhs::TupleVarsDeclaration(TupleVarsDeclaration(node))
            }
            "tensor_vars_declaration" => {
                VarDeclarationLhs::TensorVarsDeclaration(TensorVarsDeclaration(node))
            }
            "var_declaration" => VarDeclarationLhs::VarDeclaration(VarDeclaration(node)),
            _ => panic!("Unexpected var declaration lhs kind: {}", node.kind()),
        }
    }
}

impl<'tree> VarDeclarationLhs<'tree> {
    pub fn raw_node(&self) -> &Node<'tree> {
        match self {
            VarDeclarationLhs::TupleVarsDeclaration(t) => &t.0,
            VarDeclarationLhs::TensorVarsDeclaration(t) => &t.0,
            VarDeclarationLhs::VarDeclaration(v) => &v.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TupleVarsDeclaration<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for TupleVarsDeclaration<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> TupleVarsDeclaration<'tree> {
    pub fn vars(&self) -> Vec<VarDeclarationLhs<'tree>> {
        let mut cursor = self.0.walk();
        self.0
            .children(&mut cursor)
            .filter(|n| !matches!(n.kind(), "[" | "]" | ","))
            .map(VarDeclarationLhs::from)
            .collect()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TensorVarsDeclaration<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for TensorVarsDeclaration<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> TensorVarsDeclaration<'tree> {
    pub fn vars(&self) -> Vec<VarDeclarationLhs<'tree>> {
        let mut cursor = self.0.walk();
        self.0
            .children(&mut cursor)
            .filter(|n| !matches!(n.kind(), "(" | ")" | ","))
            .map(VarDeclarationLhs::from)
            .collect()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct VarDeclaration<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for VarDeclaration<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> VarDeclaration<'tree> {
    pub fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }

    pub fn typ(&self) -> Option<Type<'tree>> {
        self.0.field("type")
    }

    pub fn is_redefinition(&self) -> bool {
        self.0.field::<Ident>("redef").is_some()
    }
}
