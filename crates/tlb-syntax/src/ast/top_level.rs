use crate::ast::expressions::{
    BuiltinField, CondExpr, CurlyExpression, Identifier, TypeIdentifier, TypeParameter,
};
use crate::ast::node::{AstChildren, RawNode};
use crate::impl_ast_node;
use ton_syntax::ast::{AstNode, AstNodeBytesKind, HasName, InvalidNodeKindError, TryFromNode};
use tree_sitter::Node;

#[derive(Clone, Copy, Debug)]
pub enum TopLevel<'tree> {
    Declaration(Declaration<'tree>),
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
            TopLevel::Unmapped(node) => node.0,
        }
    }
}

impl<'tree> TryFromNode<'tree> for TopLevel<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        match node.kind_bytes() {
            b"declaration" => Ok(Self::from(node)),
            _ => Err(InvalidNodeKindError {
                expected: "declaration",
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

impl<'tree> HasName<'tree> for TopLevel<'tree> {
    type Name = Identifier<'tree>;

    fn name(&self) -> Option<Identifier<'tree>> {
        match self {
            TopLevel::Declaration(node) => node.name(),
            TopLevel::Unmapped(_) => None,
        }
    }
}

impl<'t> From<Node<'t>> for TopLevel<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"declaration" => TopLevel::Declaration(Declaration(node)),
            _ => TopLevel::Unmapped(RawNode::new(node)),
        }
    }
}

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
}

#[derive(Clone, Copy, Debug)]
pub struct Declaration<'tree>(pub Node<'tree>);
impl_ast_node!(Declaration, "declaration");

impl<'tree> Declaration<'tree> {
    #[must_use]
    pub fn constructor(&self) -> Option<Constructor<'tree>> {
        self.0.field("constructor")
    }

    #[must_use]
    pub fn combinator(&self) -> Option<Combinator<'tree>> {
        self.0.field("combinator")
    }

    #[must_use]
    pub fn fields(&self) -> AstChildren<'tree, Field<'tree>> {
        AstChildren::new(self.0)
    }
}

impl<'tree> HasName<'tree> for Declaration<'tree> {
    type Name = Identifier<'tree>;

    fn name(&self) -> Option<Identifier<'tree>> {
        self.constructor()
            .and_then(|constructor| constructor.name())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Constructor<'tree>(pub Node<'tree>);
impl_ast_node!(Constructor, "constructor_");

impl<'tree> Constructor<'tree> {
    #[must_use]
    pub fn name(&self) -> Option<Identifier<'tree>> {
        self.0.field("name")
    }

    #[must_use]
    pub fn tag(&self) -> Option<ConstructorTag<'tree>> {
        self.0.field("tag")
    }
}

impl<'tree> HasName<'tree> for Constructor<'tree> {
    type Name = Identifier<'tree>;

    fn name(&self) -> Option<Identifier<'tree>> {
        self.name()
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ConstructorTag<'tree> {
    Identifier(Identifier<'tree>),
    BinaryNumber(crate::ast::expressions::BinaryNumberLit<'tree>),
    Hex(crate::ast::expressions::HexLit<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> ConstructorTag<'tree> {
    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            ConstructorTag::Identifier(node) => node.0,
            ConstructorTag::BinaryNumber(node) => node.0,
            ConstructorTag::Hex(node) => node.0,
            ConstructorTag::Unmapped(node) => node.0,
        }
    }
}

impl<'t> From<Node<'t>> for ConstructorTag<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"identifier" => ConstructorTag::Identifier(Identifier(node)),
            b"binary_number" => {
                ConstructorTag::BinaryNumber(crate::ast::expressions::BinaryNumberLit(node))
            }
            b"hex" => ConstructorTag::Hex(crate::ast::expressions::HexLit(node)),
            _ => ConstructorTag::Unmapped(RawNode::new(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Combinator<'tree>(pub Node<'tree>);
impl_ast_node!(Combinator, "combinator");

impl<'tree> Combinator<'tree> {
    #[must_use]
    pub fn name(&self) -> Option<TypeIdentifier<'tree>> {
        self.0.field("name")
    }

    #[must_use]
    pub fn params(&self) -> AstChildren<'tree, TypeParameter<'tree>> {
        AstChildren::new(self.0)
    }
}

impl<'tree> HasName<'tree> for Combinator<'tree> {
    type Name = TypeIdentifier<'tree>;

    fn name(&self) -> Option<TypeIdentifier<'tree>> {
        self.name()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Field<'tree>(pub Node<'tree>);
impl_ast_node!(Field, "field");

impl<'tree> Field<'tree> {
    #[must_use]
    pub fn value(&self) -> Option<FieldKind<'tree>> {
        self.0.named_child(0).map(FieldKind::from)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum FieldKind<'tree> {
    FieldBuiltin(FieldBuiltin<'tree>),
    FieldCurlyExpr(FieldCurlyExpr<'tree>),
    FieldAnonymous(FieldAnonymous<'tree>),
    FieldNamed(FieldNamed<'tree>),
    FieldExpr(FieldExpr<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> FieldKind<'tree> {
    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            FieldKind::FieldBuiltin(node) => node.0,
            FieldKind::FieldCurlyExpr(node) => node.0,
            FieldKind::FieldAnonymous(node) => node.0,
            FieldKind::FieldNamed(node) => node.0,
            FieldKind::FieldExpr(node) => node.0,
            FieldKind::Unmapped(node) => node.0,
        }
    }
}

impl<'t> From<Node<'t>> for FieldKind<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"field_builtin" => FieldKind::FieldBuiltin(FieldBuiltin(node)),
            b"field_curly_expr" => FieldKind::FieldCurlyExpr(FieldCurlyExpr(node)),
            b"field_anonymous" => FieldKind::FieldAnonymous(FieldAnonymous(node)),
            b"field_named" => FieldKind::FieldNamed(FieldNamed(node)),
            b"field_expr" => FieldKind::FieldExpr(FieldExpr(node)),
            _ => FieldKind::Unmapped(RawNode::new(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FieldBuiltin<'tree>(pub Node<'tree>);
impl_ast_node!(FieldBuiltin, "field_builtin");

impl<'tree> FieldBuiltin<'tree> {
    #[must_use]
    pub fn name(&self) -> Option<Identifier<'tree>> {
        self.0.field("name")
    }

    #[must_use]
    pub fn field(&self) -> Option<BuiltinField<'tree>> {
        self.0.field("field")
    }
}

impl<'tree> HasName<'tree> for FieldBuiltin<'tree> {
    type Name = Identifier<'tree>;

    fn name(&self) -> Option<Identifier<'tree>> {
        self.name()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FieldCurlyExpr<'tree>(pub Node<'tree>);
impl_ast_node!(FieldCurlyExpr, "field_curly_expr");

impl<'tree> FieldCurlyExpr<'tree> {
    #[must_use]
    pub fn expr(&self) -> Option<CurlyExpression<'tree>> {
        self.0.field("expr")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FieldAnonymous<'tree>(pub Node<'tree>);
impl_ast_node!(FieldAnonymous, "field_anonymous");

impl<'tree> FieldAnonymous<'tree> {
    #[must_use]
    pub fn value(&self) -> Option<FieldAnonymousKind<'tree>> {
        self.0.named_child(0).map(FieldAnonymousKind::from)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum FieldAnonymousKind<'tree> {
    FieldAnonRef(FieldAnonRef<'tree>),
    FieldNamedAnonRef(FieldNamedAnonRef<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> FieldAnonymousKind<'tree> {
    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            FieldAnonymousKind::FieldAnonRef(node) => node.0,
            FieldAnonymousKind::FieldNamedAnonRef(node) => node.0,
            FieldAnonymousKind::Unmapped(node) => node.0,
        }
    }
}

impl<'t> From<Node<'t>> for FieldAnonymousKind<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"field_anon_ref" => FieldAnonymousKind::FieldAnonRef(FieldAnonRef(node)),
            b"field_named_anon_ref" => {
                FieldAnonymousKind::FieldNamedAnonRef(FieldNamedAnonRef(node))
            }
            _ => FieldAnonymousKind::Unmapped(RawNode::new(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FieldNamed<'tree>(pub Node<'tree>);
impl_ast_node!(FieldNamed, "field_named");

impl<'tree> FieldNamed<'tree> {
    #[must_use]
    pub fn name(&self) -> Option<Identifier<'tree>> {
        self.0.field("name")
    }

    #[must_use]
    pub fn expr(&self) -> Option<CondExpr<'tree>> {
        self.0.field("expr")
    }
}

impl<'tree> HasName<'tree> for FieldNamed<'tree> {
    type Name = Identifier<'tree>;

    fn name(&self) -> Option<Identifier<'tree>> {
        self.name()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FieldExpr<'tree>(pub Node<'tree>);
impl_ast_node!(FieldExpr, "field_expr");

impl<'tree> FieldExpr<'tree> {
    #[must_use]
    pub fn expr(&self) -> Option<CondExpr<'tree>> {
        self.0.named_child(0).map(CondExpr::from)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FieldAnonRef<'tree>(pub Node<'tree>);
impl_ast_node!(FieldAnonRef, "field_anon_ref");

impl<'tree> FieldAnonRef<'tree> {
    #[must_use]
    pub fn fields(&self) -> AstChildren<'tree, Field<'tree>> {
        AstChildren::new(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FieldNamedAnonRef<'tree>(pub Node<'tree>);
impl_ast_node!(FieldNamedAnonRef, "field_named_anon_ref");

impl<'tree> FieldNamedAnonRef<'tree> {
    #[must_use]
    pub fn name(&self) -> Option<Identifier<'tree>> {
        self.0.named_child(0).map(Identifier)
    }

    #[must_use]
    pub fn anon_ref(&self) -> Option<FieldAnonRef<'tree>> {
        self.0.named_child(1).map(FieldAnonRef)
    }
}

impl<'tree> HasName<'tree> for FieldNamedAnonRef<'tree> {
    type Name = Identifier<'tree>;

    fn name(&self) -> Option<Identifier<'tree>> {
        self.name()
    }
}
