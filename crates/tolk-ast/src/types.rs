use crate::expressions::NullLiteral;
use crate::node::{NodeFieldExt, RawNode};
use tree_sitter::Node;

#[derive(Clone, Copy, Debug)]
pub enum Type<'tree> {
    TypeIdentifier(TypeIdentifier<'tree>),
    TypeInstantiatedTs(TypeInstantiatedTs<'tree>),
    TensorType(TensorType<'tree>),
    TupleType(TupleType<'tree>),
    ParenthesizedType(ParenthesizedType<'tree>),
    FunCallableType(FunCallableType<'tree>),
    NullableType(NullableType<'tree>),
    UnionType(UnionType<'tree>),
    NullLiteral(NullLiteral<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> Type<'tree> {
    pub fn text(&self, source: &'tree str) -> &'tree str {
        self.raw_node().utf8_text(source.as_bytes()).unwrap_or("")
    }

    pub fn raw_node(&self) -> Node<'tree> {
        match self {
            Type::TypeIdentifier(n) => n.0,
            Type::TypeInstantiatedTs(n) => n.0,
            Type::TensorType(n) => n.0,
            Type::TupleType(n) => n.0,
            Type::ParenthesizedType(n) => n.0,
            Type::FunCallableType(n) => n.0,
            Type::NullableType(n) => n.0,
            Type::UnionType(n) => n.0,
            Type::NullLiteral(n) => n.0,
            Type::Unmapped(n) => n.0,
        }
    }
}

impl<'t> From<Node<'t>> for Type<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind() {
            "type_identifier" => Type::TypeIdentifier(TypeIdentifier(node)),
            "type_instantiatedTs" => Type::TypeInstantiatedTs(TypeInstantiatedTs(node)),
            "tensor_type" => Type::TensorType(TensorType(node)),
            "tuple_type" => Type::TupleType(TupleType(node)),
            "parenthesized_type" => Type::ParenthesizedType(ParenthesizedType(node)),
            "fun_callable_type" => Type::FunCallableType(FunCallableType(node)),
            "nullable_type" => Type::NullableType(NullableType(node)),
            "union_type" => Type::UnionType(UnionType(node)),
            "null_literal" => Type::NullLiteral(NullLiteral(node)),
            _ => Type::Unmapped(RawNode::new(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TypeIdentifier<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for TypeIdentifier<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> TypeIdentifier<'tree> {
    pub fn text(&self, source: &'tree str) -> &'tree str {
        self.0
            .utf8_text(source.as_bytes())
            .unwrap_or("<invalid utf8>")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TypeInstantiatedTs<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for TypeInstantiatedTs<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> TypeInstantiatedTs<'tree> {
    pub fn name(&self) -> Option<TypeIdentifier<'tree>> {
        self.0.field("name")
    }

    pub fn arguments(&self) -> Option<InstantiationTList<'tree>> {
        self.0.field("arguments")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TensorType<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for TensorType<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> TensorType<'tree> {
    pub fn element_types(&self) -> Vec<Type<'tree>> {
        let mut cursor = self.0.walk();
        self.0
            .children(&mut cursor)
            .filter(|n| n.is_named())
            .map(Into::into)
            .collect()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TupleType<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for TupleType<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> TupleType<'tree> {
    pub fn element_types(&self) -> Vec<Type<'tree>> {
        let mut cursor = self.0.walk();
        self.0
            .children(&mut cursor)
            .filter(|n| n.is_named())
            .map(Into::into)
            .collect()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ParenthesizedType<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for ParenthesizedType<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> ParenthesizedType<'tree> {
    pub fn inner(&self) -> Option<Type<'tree>> {
        self.0.field("inner")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FunCallableType<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for FunCallableType<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> FunCallableType<'tree> {
    pub fn param_types(&self) -> Option<Type<'tree>> {
        self.0.field("param_types")
    }

    pub fn return_type(&self) -> Option<Type<'tree>> {
        self.0.field("return_type")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NullableType<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for NullableType<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> NullableType<'tree> {
    pub fn inner(&self) -> Option<Type<'tree>> {
        self.0.field("inner")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct UnionType<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for UnionType<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> UnionType<'tree> {
    pub fn lhs(&self) -> Option<Type<'tree>> {
        self.0.field("lhs")
    }

    pub fn rhs(&self) -> Option<Type<'tree>> {
        self.0.field("rhs")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstantiationTList<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for InstantiationTList<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> InstantiationTList<'tree> {
    pub fn types(&self) -> Vec<Type<'tree>> {
        let mut cursor = self.0.walk();
        self.0
            .children(&mut cursor)
            .filter(|n| !matches!(n.kind(), "<" | ">" | ","))
            .map(Into::into)
            .collect()
    }
}
