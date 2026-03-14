use crate::ast::expressions::{Argument, Code, DataLiteral, Ident};
use crate::ast::node::{AstChildren, RawNode};
use crate::{AstNodeBytesKind, impl_ast_node};
use ton_syntax::ast::{AstNode, HasName, InvalidNodeKindError, TryFromNode};
use tree_sitter::Node;

#[derive(Clone, Copy, Debug)]
pub enum TopLevel<'tree> {
    Instruction(Instruction<'tree>),
    ExplicitRef(ExplicitRef<'tree>),
    EmbedSlice(EmbedSlice<'tree>),
    Exotic(Exotic<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> TopLevel<'tree> {
    #[must_use]
    pub fn text(&self, source: &'tree str) -> &'tree str {
        self.syntax().utf8_text(source.as_bytes()).unwrap_or("")
    }

    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            TopLevel::Instruction(n) => n.0,
            TopLevel::ExplicitRef(n) => n.0,
            TopLevel::EmbedSlice(n) => n.0,
            TopLevel::Exotic(n) => n.0,
            TopLevel::Unmapped(n) => n.0,
        }
    }
}

impl<'tree> TryFromNode<'tree> for TopLevel<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        Ok(Self::from(node))
    }
}

impl<'tree> AstNode<'tree> for TopLevel<'tree> {
    fn syntax(&self) -> Node<'tree> {
        self.syntax()
    }
}

impl<'tree> HasName<'tree> for TopLevel<'tree> {
    type Name = Ident<'tree>;

    fn name(&self) -> Option<Ident<'tree>> {
        match self {
            TopLevel::Instruction(node) => node.name(),
            _ => None,
        }
    }
}

impl<'t> From<Node<'t>> for TopLevel<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"instruction" => TopLevel::Instruction(Instruction(node)),
            b"explicit_ref" => TopLevel::ExplicitRef(ExplicitRef(node)),
            b"embed_slice" => TopLevel::EmbedSlice(EmbedSlice(node)),
            b"exotic" => TopLevel::Exotic(Exotic(node)),
            _ => TopLevel::Unmapped(RawNode::new(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Instructions<'tree>(pub Node<'tree>);
impl_ast_node!(Instructions, "instructions");

impl<'tree> Instructions<'tree> {
    pub fn items(&self) -> AstChildren<'tree, TopLevel<'tree>> {
        AstChildren::new(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Instruction<'tree>(pub Node<'tree>);
impl_ast_node!(Instruction, "instruction");

impl<'tree> Instruction<'tree> {
    #[must_use]
    pub fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }

    pub fn args(&self) -> AstChildren<'tree, Argument<'tree>> {
        AstChildren::new(self.0)
    }
}

impl<'tree> HasName<'tree> for Instruction<'tree> {
    type Name = Ident<'tree>;

    fn name(&self) -> Option<Ident<'tree>> {
        self.name()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ExplicitRef<'tree>(pub Node<'tree>);
impl_ast_node!(ExplicitRef, "explicit_ref");

impl<'tree> ExplicitRef<'tree> {
    #[must_use]
    pub fn code(&self) -> Option<Code<'tree>> {
        self.0.field("code")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EmbedSlice<'tree>(pub Node<'tree>);
impl_ast_node!(EmbedSlice, "embed_slice");

impl<'tree> EmbedSlice<'tree> {
    #[must_use]
    pub fn data(&self) -> Option<DataLiteral<'tree>> {
        self.0.field("data")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Exotic<'tree>(pub Node<'tree>);
impl_ast_node!(Exotic, "exotic");

impl<'tree> Exotic<'tree> {
    #[must_use]
    pub fn lib(&self) -> Option<ExoticLib<'tree>> {
        self.0.child_by_field_name("lib").map(ExoticLib::from)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ExoticLib<'tree> {
    ExoticLibrary(ExoticLibrary<'tree>),
    DefaultExotic(DefaultExotic<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> ExoticLib<'tree> {
    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            ExoticLib::ExoticLibrary(n) => n.0,
            ExoticLib::DefaultExotic(n) => n.0,
            ExoticLib::Unmapped(n) => n.0,
        }
    }
}

impl<'t> From<Node<'t>> for ExoticLib<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"exotic_library" => ExoticLib::ExoticLibrary(ExoticLibrary(node)),
            b"default_exotic" => ExoticLib::DefaultExotic(DefaultExotic(node)),
            _ => ExoticLib::Unmapped(RawNode::new(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ExoticLibrary<'tree>(pub Node<'tree>);
impl_ast_node!(ExoticLibrary, "exotic_library");

impl<'tree> ExoticLibrary<'tree> {
    #[must_use]
    pub fn data(&self) -> Option<DataLiteral<'tree>> {
        self.0.field("data")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DefaultExotic<'tree>(pub Node<'tree>);
impl_ast_node!(DefaultExotic, "default_exotic");

impl<'tree> DefaultExotic<'tree> {
    #[must_use]
    pub fn data(&self) -> Option<DataLiteral<'tree>> {
        self.0.field("data")
    }
}
