use crate::ast::node::{AstChildren, RawNode};
use crate::impl_ast_node;
use ton_syntax::ast::{AstNode, AstNodeBytesKind, InvalidNodeKindError, TryFromNode};
use tree_sitter::Node;

#[derive(Clone, Copy, Debug)]
pub enum InstructionExpr<'tree> {
    Identifier(Ident<'tree>),
    NegativeIdentifier(NegativeIdent<'tree>),
    Number(NumberLit<'tree>),
    String(StringLit<'tree>),
    IfStatement(IfStatement<'tree>),
    IfJmpStatement(IfJmpStatement<'tree>),
    WhileStatement(WhileStatement<'tree>),
    RepeatStatement(RepeatStatement<'tree>),
    UntilStatement(UntilStatement<'tree>),
    ProcCall(ProcCall<'tree>),
    SliceLiteral(SliceLit<'tree>),
    HexLiteral(HexLit<'tree>),
    StackRef(StackRef<'tree>),
    StackOp(StackOp<'tree>),
    InstructionBlock(InstructionBlock<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> InstructionExpr<'tree> {
    #[must_use]
    pub fn text(&self, source: &'tree str) -> &'tree str {
        self.syntax().utf8_text(source.as_bytes()).unwrap_or("")
    }

    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            InstructionExpr::Identifier(node) => node.0,
            InstructionExpr::NegativeIdentifier(node) => node.0,
            InstructionExpr::Number(node) => node.0,
            InstructionExpr::String(node) => node.0,
            InstructionExpr::IfStatement(node) => node.0,
            InstructionExpr::IfJmpStatement(node) => node.0,
            InstructionExpr::WhileStatement(node) => node.0,
            InstructionExpr::RepeatStatement(node) => node.0,
            InstructionExpr::UntilStatement(node) => node.0,
            InstructionExpr::ProcCall(node) => node.0,
            InstructionExpr::SliceLiteral(node) => node.0,
            InstructionExpr::HexLiteral(node) => node.0,
            InstructionExpr::StackRef(node) => node.0,
            InstructionExpr::StackOp(node) => node.0,
            InstructionExpr::InstructionBlock(node) => node.0,
            InstructionExpr::Unmapped(node) => node.0,
        }
    }
}

impl<'tree> TryFromNode<'tree> for InstructionExpr<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        Ok(Self::from(node))
    }
}

impl<'tree> AstNode<'tree> for InstructionExpr<'tree> {
    fn syntax(&self) -> Node<'tree> {
        self.syntax()
    }
}

impl<'t> From<Node<'t>> for InstructionExpr<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"identifier" => InstructionExpr::Identifier(Ident(node)),
            b"negative_identifier" => InstructionExpr::NegativeIdentifier(NegativeIdent(node)),
            b"number" => InstructionExpr::Number(NumberLit(node)),
            b"string" => InstructionExpr::String(StringLit(node)),
            b"if_statement" => InstructionExpr::IfStatement(IfStatement(node)),
            b"ifjmp_statement" => InstructionExpr::IfJmpStatement(IfJmpStatement(node)),
            b"while_statement" => InstructionExpr::WhileStatement(WhileStatement(node)),
            b"repeat_statement" => InstructionExpr::RepeatStatement(RepeatStatement(node)),
            b"until_statement" => InstructionExpr::UntilStatement(UntilStatement(node)),
            b"proc_call" => InstructionExpr::ProcCall(ProcCall(node)),
            b"slice_literal" => InstructionExpr::SliceLiteral(SliceLit(node)),
            b"hex_literal" => InstructionExpr::HexLiteral(HexLit(node)),
            b"stack_ref" => InstructionExpr::StackRef(StackRef(node)),
            b"stack_op" => InstructionExpr::StackOp(StackOp(node)),
            b"instruction_block" => InstructionExpr::InstructionBlock(InstructionBlock(node)),
            _ => InstructionExpr::Unmapped(RawNode::new(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Instruction<'tree>(pub Node<'tree>);
impl_ast_node!(Instruction, "instruction");

impl<'tree> Instruction<'tree> {
    #[must_use]
    pub fn value(&self) -> Option<InstructionExpr<'tree>> {
        self.0.named_child(0).map(InstructionExpr::from)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstructionBlock<'tree>(pub Node<'tree>);
impl_ast_node!(InstructionBlock, "instruction_block");

impl<'tree> InstructionBlock<'tree> {
    #[must_use]
    pub fn instructions(&self) -> AstChildren<'tree, Instruction<'tree>> {
        AstChildren::new(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct IfStatement<'tree>(pub Node<'tree>);
impl_ast_node!(IfStatement, "if_statement");

impl<'tree> IfStatement<'tree> {
    #[must_use]
    pub fn instructions(&self) -> AstChildren<'tree, Instruction<'tree>> {
        AstChildren::new(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct IfJmpStatement<'tree>(pub Node<'tree>);
impl_ast_node!(IfJmpStatement, "ifjmp_statement");

impl<'tree> IfJmpStatement<'tree> {
    #[must_use]
    pub fn instructions(&self) -> AstChildren<'tree, Instruction<'tree>> {
        AstChildren::new(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct WhileStatement<'tree>(pub Node<'tree>);
impl_ast_node!(WhileStatement, "while_statement");

impl<'tree> WhileStatement<'tree> {
    #[must_use]
    pub fn instructions(&self) -> AstChildren<'tree, Instruction<'tree>> {
        AstChildren::new(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RepeatStatement<'tree>(pub Node<'tree>);
impl_ast_node!(RepeatStatement, "repeat_statement");

impl<'tree> RepeatStatement<'tree> {
    #[must_use]
    pub fn instructions(&self) -> AstChildren<'tree, Instruction<'tree>> {
        AstChildren::new(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct UntilStatement<'tree>(pub Node<'tree>);
impl_ast_node!(UntilStatement, "until_statement");

impl<'tree> UntilStatement<'tree> {
    #[must_use]
    pub fn instructions(&self) -> AstChildren<'tree, Instruction<'tree>> {
        AstChildren::new(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ProcCall<'tree>(pub Node<'tree>);
impl_ast_node!(ProcCall, "proc_call");

impl<'tree> ProcCall<'tree> {
    #[must_use]
    pub fn name(&self) -> Option<Ident<'tree>> {
        self.0.named_child(0).map(Ident)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NegativeIdent<'tree>(pub Node<'tree>);
impl_ast_node!(NegativeIdent, "negative_identifier");

impl<'tree> NegativeIdent<'tree> {
    #[must_use]
    pub fn value(&self) -> Option<Ident<'tree>> {
        self.0.named_child(0).map(Ident)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StackOp<'tree>(pub Node<'tree>);
impl_ast_node!(StackOp, "stack_op");

impl<'tree> StackOp<'tree> {
    #[must_use]
    pub fn stack_indices(&self) -> AstChildren<'tree, StackIndex<'tree>> {
        AstChildren::new(self.0)
    }

    #[must_use]
    pub fn stack_refs(&self) -> AstChildren<'tree, StackRef<'tree>> {
        AstChildren::new(self.0)
    }

    #[must_use]
    pub fn operation(&self) -> Option<Ident<'tree>> {
        let mut cursor = self.0.walk();
        self.0
            .named_children(&mut cursor)
            .filter(|child| child.kind_bytes() == b"identifier")
            .map(Ident)
            .last()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Ident<'tree>(pub Node<'tree>);
impl_ast_node!(Ident, "identifier");

#[derive(Clone, Copy, Debug)]
pub struct NumberLit<'tree>(pub Node<'tree>);
impl_ast_node!(NumberLit, "number");

#[derive(Clone, Copy, Debug)]
pub struct StringLit<'tree>(pub Node<'tree>);
impl_ast_node!(StringLit, "string");

#[derive(Clone, Copy, Debug)]
pub struct SliceLit<'tree>(pub Node<'tree>);
impl_ast_node!(SliceLit, "slice_literal");

#[derive(Clone, Copy, Debug)]
pub struct HexLit<'tree>(pub Node<'tree>);
impl_ast_node!(HexLit, "hex_literal");

#[derive(Clone, Copy, Debug)]
pub struct StackRef<'tree>(pub Node<'tree>);
impl_ast_node!(StackRef, "stack_ref");

#[derive(Clone, Copy, Debug)]
pub struct StackIndex<'tree>(pub Node<'tree>);
impl_ast_node!(StackIndex, "stack_index");
