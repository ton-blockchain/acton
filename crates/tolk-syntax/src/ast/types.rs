use crate::ast::expressions::NullLit;
use crate::ast::node::{AstChildren, RawNode};
use crate::ast::{AstNode, InvalidNodeKindError, TryFromNode};
use crate::{AstNodeBytesKind, impl_ast_node};
use tree_sitter::Node;

#[derive(Clone, Copy, Debug)]
pub enum Type<'tree> {
    TypeIdent(TypeIdent<'tree>),
    TypeInstantiatedTs(TypeInstantiatedTs<'tree>),
    TensorType(TensorType<'tree>),
    TupleType(TupleType<'tree>),
    ParenthesizedType(ParenthesizedType<'tree>),
    FunCallableType(FunCallableType<'tree>),
    NullableType(NullableType<'tree>),
    UnionType(UnionType<'tree>),
    NullLit(NullLit<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> Type<'tree> {
    #[must_use]
    pub fn text(&self, source: &'tree str) -> &'tree str {
        self.syntax().utf8_text(source.as_bytes()).unwrap_or("")
    }

    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            Type::TypeIdent(n) => n.0,
            Type::TypeInstantiatedTs(n) => n.0,
            Type::TensorType(n) => n.0,
            Type::TupleType(n) => n.0,
            Type::ParenthesizedType(n) => n.0,
            Type::FunCallableType(n) => n.0,
            Type::NullableType(n) => n.0,
            Type::UnionType(n) => n.0,
            Type::NullLit(n) => n.0,
            Type::Unmapped(n) => n.0,
        }
    }
}

impl<'t> From<Node<'t>> for Type<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"type_identifier" => Type::TypeIdent(TypeIdent(node)),
            b"type_instantiatedTs" => Type::TypeInstantiatedTs(TypeInstantiatedTs(node)),
            b"tensor_type" => Type::TensorType(TensorType(node)),
            b"tuple_type" => Type::TupleType(TupleType(node)),
            b"parenthesized_type" => Type::ParenthesizedType(ParenthesizedType(node)),
            b"fun_callable_type" => Type::FunCallableType(FunCallableType(node)),
            b"nullable_type" => Type::NullableType(NullableType(node)),
            b"union_type" => Type::UnionType(UnionType(node)),
            b"null_literal" => Type::NullLit(NullLit(node)),
            _ => Type::Unmapped(RawNode::new(node)),
        }
    }
}

impl<'tree> TryFromNode<'tree> for Type<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        let res = Self::from(node);
        match res {
            Type::Unmapped(_) => Err(InvalidNodeKindError {
                expected: "type",
                actual: node.kind().to_string(),
            }),
            _ => Ok(res),
        }
    }
}

impl<'tree> AstNode<'tree> for Type<'tree> {
    fn syntax(&self) -> Node<'tree> {
        self.syntax()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TypeIdent<'tree>(pub Node<'tree>);

impl_ast_node!(TypeIdent, "type_identifier");

impl<'tree> TypeIdent<'tree> {
    #[must_use]
    pub fn text(&self, source: &'tree str) -> &'tree str {
        self.0
            .utf8_text(source.as_bytes())
            .unwrap_or("<invalid utf8>")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TypeInstantiatedTs<'tree>(pub Node<'tree>);

impl_ast_node!(TypeInstantiatedTs, "type_instantiatedTs");

impl<'tree> TypeInstantiatedTs<'tree> {
    #[must_use]
    pub fn name(&self) -> Option<TypeIdent<'tree>> {
        self.0.field("name")
    }

    #[must_use]
    pub fn arguments(&self) -> Option<InstantiationTList<'tree>> {
        self.0.field("arguments")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TensorType<'tree>(pub Node<'tree>);

impl_ast_node!(TensorType, "tensor_type");

impl<'tree> TensorType<'tree> {
    pub fn elements(&self) -> AstChildren<'tree, Type<'tree>> {
        AstChildren::new(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TupleType<'tree>(pub Node<'tree>);

impl_ast_node!(TupleType, "tuple_type");

impl<'tree> TupleType<'tree> {
    pub fn elements(&self) -> AstChildren<'tree, Type<'tree>> {
        AstChildren::new(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ParenthesizedType<'tree>(pub Node<'tree>);

impl_ast_node!(ParenthesizedType, "parenthesized_type");

impl<'tree> ParenthesizedType<'tree> {
    #[must_use]
    pub fn inner(&self) -> Option<Type<'tree>> {
        self.0.field("inner")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FunCallableType<'tree>(pub Node<'tree>);

impl_ast_node!(FunCallableType, "fun_callable_type");

impl<'tree> FunCallableType<'tree> {
    #[must_use]
    pub fn param_types(&self) -> Option<Type<'tree>> {
        self.0.field("param_types")
    }

    #[must_use]
    pub fn return_type(&self) -> Option<Type<'tree>> {
        self.0.field("return_type")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NullableType<'tree>(pub Node<'tree>);

impl_ast_node!(NullableType, "nullable_type");

impl<'tree> NullableType<'tree> {
    #[must_use]
    pub fn inner(&self) -> Option<Type<'tree>> {
        self.0.field("inner")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct UnionType<'tree>(pub Node<'tree>);

impl_ast_node!(UnionType, "union_type");

impl<'tree> UnionType<'tree> {
    #[must_use]
    pub fn lhs(&self) -> Option<Type<'tree>> {
        self.0.field("lhs")
    }

    #[must_use]
    pub fn rhs(&self) -> Option<Type<'tree>> {
        self.0.field("rhs")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstantiationTList<'tree>(pub Node<'tree>);

impl_ast_node!(InstantiationTList, "instantiation_t_list");

impl<'tree> InstantiationTList<'tree> {
    pub fn types(&self) -> AstChildren<'tree, Type<'tree>> {
        AstChildren::new(self.0)
    }
}
