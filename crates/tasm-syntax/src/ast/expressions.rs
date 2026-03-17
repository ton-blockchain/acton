use crate::ast::node::{AstChildren, RawNode};
use crate::ast::top_level::Instructions;
use crate::{AstNodeBytesKind, impl_ast_node};
use ton_syntax::ast::{AstNode, InvalidNodeKindError, TryFromNode};
use tree_sitter::Node;

#[derive(Clone, Copy, Debug)]
pub enum Expr<'tree> {
    IntegerLit(IntegerLit<'tree>),
    DataLiteral(DataLiteral<'tree>),
    Code(Code<'tree>),
    Dictionary(Dictionary<'tree>),
    StackElement(StackElement<'tree>),
    ControlRegister(ControlRegister<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> Expr<'tree> {
    #[must_use]
    pub fn text(&self, source: &'tree str) -> &'tree str {
        self.syntax().utf8_text(source.as_bytes()).unwrap_or("")
    }

    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            Expr::IntegerLit(n) => n.0,
            Expr::DataLiteral(n) => n.0,
            Expr::Code(n) => n.0,
            Expr::Dictionary(n) => n.0,
            Expr::StackElement(n) => n.0,
            Expr::ControlRegister(n) => n.0,
            Expr::Unmapped(n) => n.0,
        }
    }
}

impl<'tree> TryFromNode<'tree> for Expr<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        Ok(Self::from(node))
    }
}

impl<'tree> AstNode<'tree> for Expr<'tree> {
    fn syntax(&self) -> Node<'tree> {
        self.syntax()
    }
}

impl<'t> From<Node<'t>> for Expr<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"integer_literal" => Expr::IntegerLit(IntegerLit(node)),
            b"data_literal" => Expr::DataLiteral(DataLiteral(node)),
            b"code" => Expr::Code(Code(node)),
            b"dictionary" => Expr::Dictionary(Dictionary(node)),
            b"stack_element" => Expr::StackElement(StackElement(node)),
            b"control_register" => Expr::ControlRegister(ControlRegister(node)),
            _ => Expr::Unmapped(RawNode::new(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum DataLit<'tree> {
    Hex(HexLit<'tree>),
    Bin(BinLit<'tree>),
    Boc(BocLit<'tree>),
    String(StringLit<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> DataLit<'tree> {
    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            DataLit::Hex(n) => n.0,
            DataLit::Bin(n) => n.0,
            DataLit::Boc(n) => n.0,
            DataLit::String(n) => n.0,
            DataLit::Unmapped(n) => n.0,
        }
    }
}

impl<'t> From<Node<'t>> for DataLit<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"hex_literal" => DataLit::Hex(HexLit(node)),
            b"bin_literal" => DataLit::Bin(BinLit(node)),
            b"boc_literal" => DataLit::Boc(BocLit(node)),
            b"string_literal" => DataLit::String(StringLit(node)),
            _ => DataLit::Unmapped(RawNode::new(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Argument<'tree>(pub Node<'tree>);

impl_ast_node!(Argument, "argument");

impl<'tree> Argument<'tree> {
    #[must_use]
    pub fn expr(&self) -> Option<Expr<'tree>> {
        self.0.named_child(0).map(Expr::from)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DataLiteral<'tree>(pub Node<'tree>);

impl_ast_node!(DataLiteral, "data_literal");

impl<'tree> DataLiteral<'tree> {
    #[must_use]
    pub fn value(&self) -> Option<DataLit<'tree>> {
        self.0.named_child(0).map(DataLit::from)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Code<'tree>(pub Node<'tree>);

impl_ast_node!(Code, "code");

impl<'tree> Code<'tree> {
    #[must_use]
    pub fn instructions(&self) -> Option<Instructions<'tree>> {
        self.0.field("instructions")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Dictionary<'tree>(pub Node<'tree>);

impl_ast_node!(Dictionary, "dictionary");

impl<'tree> Dictionary<'tree> {
    pub fn entries(&self) -> AstChildren<'tree, DictionaryEntry<'tree>> {
        AstChildren::new(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DictionaryEntry<'tree>(pub Node<'tree>);

impl_ast_node!(DictionaryEntry, "dictionary_entry");

impl<'tree> DictionaryEntry<'tree> {
    #[must_use]
    pub fn id(&self) -> Option<IntegerLit<'tree>> {
        self.0.field("id")
    }

    #[must_use]
    pub fn code(&self) -> Option<Code<'tree>> {
        self.0.field("code")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Ident<'tree>(pub Node<'tree>);
impl_ast_node!(Ident, "identifier");

#[derive(Clone, Copy, Debug)]
pub struct IntegerLit<'tree>(pub Node<'tree>);
impl_ast_node!(IntegerLit, "integer_literal");

#[derive(Clone, Copy, Debug)]
pub struct HexLit<'tree>(pub Node<'tree>);
impl_ast_node!(HexLit, "hex_literal");

#[derive(Clone, Copy, Debug)]
pub struct BinLit<'tree>(pub Node<'tree>);
impl_ast_node!(BinLit, "bin_literal");

#[derive(Clone, Copy, Debug)]
pub struct BocLit<'tree>(pub Node<'tree>);
impl_ast_node!(BocLit, "boc_literal");

#[derive(Clone, Copy, Debug)]
pub struct StringLit<'tree>(pub Node<'tree>);
impl_ast_node!(StringLit, "string_literal");

#[derive(Clone, Copy, Debug)]
pub struct StackElement<'tree>(pub Node<'tree>);
impl_ast_node!(StackElement, "stack_element");

#[derive(Clone, Copy, Debug)]
pub struct ControlRegister<'tree>(pub Node<'tree>);
impl_ast_node!(ControlRegister, "control_register");
