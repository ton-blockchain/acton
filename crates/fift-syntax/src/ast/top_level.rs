use crate::ast::expressions::{Ident, Instruction, NumberLit};
use crate::ast::node::{AstChildren, RawNode};
use crate::impl_ast_node;
use ton_syntax::ast::{AstNode, AstNodeBytesKind, HasName, InvalidNodeKindError, TryFromNode};
use tree_sitter::Node;

#[derive(Clone, Copy, Debug)]
pub enum TopLevel<'tree> {
    Declaration(Declaration<'tree>),
    Definition(Definition<'tree>),
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
            TopLevel::Declaration(node) => node.0,
            TopLevel::Definition(node) => node.0,
            TopLevel::Unmapped(node) => node.0,
        }
    }
}

impl<'tree> TryFromNode<'tree> for TopLevel<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        match node.kind_bytes() {
            b"declaration" | b"definition" => Ok(Self::from(node)),
            _ => Err(InvalidNodeKindError {
                expected: "declaration|definition",
                actual: node.kind().to_string(),
            }),
        }
    }
}

impl<'tree> AstNode<'tree> for TopLevel<'tree> {
    fn syntax(&self) -> Node<'tree> {
        self.syntax()
    }
}

impl<'t> From<Node<'t>> for TopLevel<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"declaration" => TopLevel::Declaration(Declaration(node)),
            b"definition" => TopLevel::Definition(Definition(node)),
            _ => TopLevel::Unmapped(RawNode::new(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct IncludeDirective<'tree>(pub Node<'tree>);
impl_ast_node!(IncludeDirective, "include_directive");

#[derive(Clone, Copy, Debug)]
pub struct Program<'tree>(pub Node<'tree>);
impl_ast_node!(Program, "program");

impl<'tree> Program<'tree> {
    #[must_use]
    pub fn items(&self) -> AstChildren<'tree, TopLevel<'tree>> {
        AstChildren::new(self.0)
    }

    #[must_use]
    pub fn declarations(&self) -> AstChildren<'tree, Declaration<'tree>> {
        AstChildren::new(self.0)
    }

    #[must_use]
    pub fn definitions(&self) -> AstChildren<'tree, Definition<'tree>> {
        AstChildren::new(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Declaration<'tree>(pub Node<'tree>);
impl_ast_node!(Declaration, "declaration");

impl<'tree> Declaration<'tree> {
    #[must_use]
    pub fn kind(&self) -> Option<DeclarationKind<'tree>> {
        self.0.named_child(0).map(DeclarationKind::from)
    }
}

impl<'tree> HasName<'tree> for Declaration<'tree> {
    type Name = Ident<'tree>;

    fn name(&self) -> Option<Ident<'tree>> {
        self.kind().and_then(|kind| kind.name())
    }
}

#[derive(Clone, Copy, Debug)]
pub enum DeclarationKind<'tree> {
    ProcDeclaration(ProcDeclaration<'tree>),
    MethodDeclaration(MethodDeclaration<'tree>),
    GlobalVar(GlobalVar<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> DeclarationKind<'tree> {
    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            DeclarationKind::ProcDeclaration(node) => node.0,
            DeclarationKind::MethodDeclaration(node) => node.0,
            DeclarationKind::GlobalVar(node) => node.0,
            DeclarationKind::Unmapped(node) => node.0,
        }
    }

    #[must_use]
    pub fn name(&self) -> Option<Ident<'tree>> {
        match self {
            DeclarationKind::ProcDeclaration(node) => node.name(),
            DeclarationKind::MethodDeclaration(node) => node.name(),
            DeclarationKind::GlobalVar(node) => node.name(),
            DeclarationKind::Unmapped(_) => None,
        }
    }
}

impl<'t> From<Node<'t>> for DeclarationKind<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"proc_declaration" => DeclarationKind::ProcDeclaration(ProcDeclaration(node)),
            b"method_declaration" => DeclarationKind::MethodDeclaration(MethodDeclaration(node)),
            b"global_var" => DeclarationKind::GlobalVar(GlobalVar(node)),
            _ => DeclarationKind::Unmapped(RawNode::new(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ProcDeclaration<'tree>(pub Node<'tree>);
impl_ast_node!(ProcDeclaration, "proc_declaration");

impl<'tree> ProcDeclaration<'tree> {
    #[must_use]
    pub fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }
}

impl<'tree> HasName<'tree> for ProcDeclaration<'tree> {
    type Name = Ident<'tree>;

    fn name(&self) -> Option<Ident<'tree>> {
        self.name()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MethodDeclaration<'tree>(pub Node<'tree>);
impl_ast_node!(MethodDeclaration, "method_declaration");

impl<'tree> MethodDeclaration<'tree> {
    #[must_use]
    pub fn id(&self) -> Option<NumberLit<'tree>> {
        self.0.field("id")
    }

    #[must_use]
    pub fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }
}

impl<'tree> HasName<'tree> for MethodDeclaration<'tree> {
    type Name = Ident<'tree>;

    fn name(&self) -> Option<Ident<'tree>> {
        self.name()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GlobalVar<'tree>(pub Node<'tree>);
impl_ast_node!(GlobalVar, "global_var");

impl<'tree> GlobalVar<'tree> {
    #[must_use]
    pub fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }
}

impl<'tree> HasName<'tree> for GlobalVar<'tree> {
    type Name = Ident<'tree>;

    fn name(&self) -> Option<Ident<'tree>> {
        self.name()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Definition<'tree>(pub Node<'tree>);
impl_ast_node!(Definition, "definition");

impl<'tree> Definition<'tree> {
    #[must_use]
    pub fn kind(&self) -> Option<DefinitionKind<'tree>> {
        self.0.named_child(0).map(DefinitionKind::from)
    }
}

impl<'tree> HasName<'tree> for Definition<'tree> {
    type Name = Ident<'tree>;

    fn name(&self) -> Option<Ident<'tree>> {
        self.kind().and_then(|kind| kind.name())
    }
}

#[derive(Clone, Copy, Debug)]
pub enum DefinitionKind<'tree> {
    ProcDefinition(ProcDefinition<'tree>),
    ProcInlineDefinition(ProcInlineDefinition<'tree>),
    ProcRefDefinition(ProcRefDefinition<'tree>),
    MethodDefinition(MethodDefinition<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> DefinitionKind<'tree> {
    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            DefinitionKind::ProcDefinition(node) => node.0,
            DefinitionKind::ProcInlineDefinition(node) => node.0,
            DefinitionKind::ProcRefDefinition(node) => node.0,
            DefinitionKind::MethodDefinition(node) => node.0,
            DefinitionKind::Unmapped(node) => node.0,
        }
    }

    #[must_use]
    pub fn name(&self) -> Option<Ident<'tree>> {
        match self {
            DefinitionKind::ProcDefinition(node) => node.name(),
            DefinitionKind::ProcInlineDefinition(node) => node.name(),
            DefinitionKind::ProcRefDefinition(node) => node.name(),
            DefinitionKind::MethodDefinition(node) => node.name(),
            DefinitionKind::Unmapped(_) => None,
        }
    }
}

impl<'t> From<Node<'t>> for DefinitionKind<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"proc_definition" => DefinitionKind::ProcDefinition(ProcDefinition(node)),
            b"proc_inline_definition" => {
                DefinitionKind::ProcInlineDefinition(ProcInlineDefinition(node))
            }
            b"proc_ref_definition" => DefinitionKind::ProcRefDefinition(ProcRefDefinition(node)),
            b"method_definition" => DefinitionKind::MethodDefinition(MethodDefinition(node)),
            _ => DefinitionKind::Unmapped(RawNode::new(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ProcDefinition<'tree>(pub Node<'tree>);
impl_ast_node!(ProcDefinition, "proc_definition");

impl<'tree> ProcDefinition<'tree> {
    #[must_use]
    pub fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }

    #[must_use]
    pub fn instructions(&self) -> AstChildren<'tree, Instruction<'tree>> {
        AstChildren::new(self.0)
    }
}

impl<'tree> HasName<'tree> for ProcDefinition<'tree> {
    type Name = Ident<'tree>;

    fn name(&self) -> Option<Ident<'tree>> {
        self.name()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ProcInlineDefinition<'tree>(pub Node<'tree>);
impl_ast_node!(ProcInlineDefinition, "proc_inline_definition");

impl<'tree> ProcInlineDefinition<'tree> {
    #[must_use]
    pub fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }

    #[must_use]
    pub fn instructions(&self) -> AstChildren<'tree, Instruction<'tree>> {
        AstChildren::new(self.0)
    }
}

impl<'tree> HasName<'tree> for ProcInlineDefinition<'tree> {
    type Name = Ident<'tree>;

    fn name(&self) -> Option<Ident<'tree>> {
        self.name()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ProcRefDefinition<'tree>(pub Node<'tree>);
impl_ast_node!(ProcRefDefinition, "proc_ref_definition");

impl<'tree> ProcRefDefinition<'tree> {
    #[must_use]
    pub fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }

    #[must_use]
    pub fn instructions(&self) -> AstChildren<'tree, Instruction<'tree>> {
        AstChildren::new(self.0)
    }
}

impl<'tree> HasName<'tree> for ProcRefDefinition<'tree> {
    type Name = Ident<'tree>;

    fn name(&self) -> Option<Ident<'tree>> {
        self.name()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MethodDefinition<'tree>(pub Node<'tree>);
impl_ast_node!(MethodDefinition, "method_definition");

impl<'tree> MethodDefinition<'tree> {
    #[must_use]
    pub fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }

    #[must_use]
    pub fn instructions(&self) -> AstChildren<'tree, Instruction<'tree>> {
        AstChildren::new(self.0)
    }
}

impl<'tree> HasName<'tree> for MethodDefinition<'tree> {
    type Name = Ident<'tree>;

    fn name(&self) -> Option<Ident<'tree>> {
        self.name()
    }
}
