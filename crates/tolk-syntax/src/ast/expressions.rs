use crate::ast::node::{AstChildren, RawNode};
use crate::ast::traits::HasName;
use crate::ast::types::{InstantiationTList, Type};
use crate::ast::{AstNode, Block};
use crate::ast::{InvalidNodeKindError, TryFromNode};
use crate::{AstNodeBytesKind, impl_ast_node};
use tree_sitter::Node;

#[derive(Clone, Copy, Debug)]
pub struct Ident<'tree>(pub Node<'tree>);

impl_ast_node!(Ident, "identifier");

impl<'tree> Ident<'tree> {
    #[must_use]
    pub fn normalized_name(&self, source: &'tree str) -> &'tree str {
        self.text(source).trim_matches('`')
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StringLit<'tree>(pub Node<'tree>);

impl_ast_node!(StringLit, "string_literal");

impl<'tree> StringLit<'tree> {
    #[must_use]
    pub fn content(&self, source: &'tree str) -> &'tree str {
        self.text(source).trim_matches('"')
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NumberLit<'tree>(pub Node<'tree>);

impl_ast_node!(NumberLit, "number_literal");

#[derive(Clone, Copy, Debug)]
pub struct BoolLit<'tree>(pub Node<'tree>);

impl_ast_node!(BoolLit, "boolean_literal");

impl<'tree> BoolLit<'tree> {
    #[must_use]
    pub fn value(&self) -> bool {
        let width = self.0.end_byte() - self.0.start_byte();
        width == 4 // "true".len()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NumericIndex<'tree>(pub Node<'tree>);

impl_ast_node!(NumericIndex, "numeric_index");

impl<'tree> NumericIndex<'tree> {
    #[must_use]
    pub fn value(&self, source: &'tree str) -> &'tree str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Expr<'tree> {
    VarDeclLhs(VarDeclLhs<'tree>),
    Assign(Assign<'tree>),
    SetAssign(SetAssign<'tree>),
    Ternary(Ternary<'tree>),
    Bin(Bin<'tree>),
    Unary(Unary<'tree>),
    Lazy(Lazy<'tree>),
    AsCast(AsCast<'tree>),
    IsType(IsType<'tree>),
    NotNull(NotNull<'tree>),
    DotAccess(DotAccess<'tree>),
    Call(Call<'tree>),
    Instantiation(Instantiation<'tree>),
    Paren(Paren<'tree>),
    Match(Match<'tree>),
    ObjectLit(ObjectLit<'tree>),
    Tensor(Tensor<'tree>),
    Tuple(Tuple<'tree>),
    Lambda(Lambda<'tree>),
    NumberLit(NumberLit<'tree>),
    StringLit(StringLit<'tree>),
    BoolLit(BoolLit<'tree>),
    NullLit(NullLit<'tree>),
    Ident(Ident<'tree>),
    Underscore(Underscore<'tree>),
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
            Expr::VarDeclLhs(n) => n.0,
            Expr::Assign(n) => n.0,
            Expr::SetAssign(n) => n.0,
            Expr::Ternary(n) => n.0,
            Expr::Bin(n) => n.0,
            Expr::Unary(n) => n.0,
            Expr::Lazy(n) => n.0,
            Expr::AsCast(n) => n.0,
            Expr::IsType(n) => n.0,
            Expr::NotNull(n) => n.0,
            Expr::DotAccess(n) => n.0,
            Expr::Call(n) => n.0,
            Expr::Instantiation(n) => n.0,
            Expr::Paren(n) => n.0,
            Expr::Match(n) => n.0,
            Expr::ObjectLit(n) => n.0,
            Expr::Tensor(n) => n.0,
            Expr::Tuple(n) => n.0,
            Expr::Lambda(n) => n.0,
            Expr::NumberLit(n) => n.0,
            Expr::StringLit(n) => n.0,
            Expr::BoolLit(n) => n.0,
            Expr::NullLit(n) => n.0,
            Expr::Underscore(n) => n.0,
            Expr::Ident(n) => n.0,
            Expr::Unmapped(n) => n.0,
        }
    }
}

impl<'t> From<Node<'t>> for Expr<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"var_declaration_lhs" => Expr::VarDeclLhs(VarDeclLhs(node)),
            b"assignment" => Expr::Assign(Assign(node)),
            b"set_assignment" => Expr::SetAssign(SetAssign(node)),
            b"ternary_operator" => Expr::Ternary(Ternary(node)),
            b"binary_operator" => Expr::Bin(Bin(node)),
            b"unary_operator" => Expr::Unary(Unary(node)),
            b"lazy_expression" => Expr::Lazy(Lazy(node)),
            b"cast_as_operator" => Expr::AsCast(AsCast(node)),
            b"is_type_operator" => Expr::IsType(IsType(node)),
            b"not_null_operator" => Expr::NotNull(NotNull(node)),
            b"dot_access" => Expr::DotAccess(DotAccess(node)),
            b"function_call" => Expr::Call(Call(node)),
            b"generic_instantiation" => Expr::Instantiation(Instantiation(node)),
            b"parenthesized_expression" => Expr::Paren(Paren(node)),
            b"match_expression" => Expr::Match(Match(node)),
            b"object_literal" => Expr::ObjectLit(ObjectLit(node)),
            b"tensor_expression" => Expr::Tensor(Tensor(node)),
            b"typed_tuple" => Expr::Tuple(Tuple(node)),
            b"lambda_expression" => Expr::Lambda(Lambda(node)),
            b"number_literal" => Expr::NumberLit(NumberLit(node)),
            b"string_literal" => Expr::StringLit(StringLit(node)),
            b"boolean_literal" => Expr::BoolLit(BoolLit(node)),
            b"null_literal" => Expr::NullLit(NullLit(node)),
            b"underscore" => Expr::Underscore(Underscore(node)),
            b"identifier" => Expr::Ident(Ident(node)),
            _ => Expr::Unmapped(RawNode::new(node)),
        }
    }
}

impl<'tree> TryFromNode<'tree> for Expr<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        let res = Self::from(node);
        match res {
            Expr::Unmapped(_) => Err(InvalidNodeKindError {
                expected: "expression",
                actual: node.kind().to_string(),
            }),
            _ => Ok(res),
        }
    }
}

impl<'tree> AstNode<'tree> for Expr<'tree> {
    fn syntax(&self) -> Node<'tree> {
        self.syntax()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Assign<'tree>(pub Node<'tree>);

impl_ast_node!(Assign, "assignment");

impl<'tree> Assign<'tree> {
    #[must_use]
    pub fn is_lhs(&self, node: &Node<'tree>) -> bool {
        self.left().is_some_and(|l| {
            let n = l.syntax();
            node.start_byte() >= n.start_byte() && node.end_byte() <= n.end_byte()
        })
    }

    #[must_use]
    pub fn left(&self) -> Option<Expr<'tree>> {
        self.0.field("left")
    }

    #[must_use]
    pub fn right(&self) -> Option<Expr<'tree>> {
        self.0.field("right")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SetAssign<'tree>(pub Node<'tree>);

impl_ast_node!(SetAssign, "set_assignment");

impl<'tree> SetAssign<'tree> {
    #[must_use]
    pub fn is_lhs(&self, node: &Node<'tree>) -> bool {
        self.left().is_some_and(|l| {
            let n = l.syntax();
            node.start_byte() >= n.start_byte() && node.end_byte() <= n.end_byte()
        })
    }

    #[must_use]
    pub fn left(&self) -> Option<Expr<'tree>> {
        self.0.field("left")
    }

    #[must_use]
    pub fn operator_name(&self, source: &'tree str) -> &'tree str {
        let Some(op_child): Option<Node<'tree>> = self.0.field("operator_name") else {
            return "";
        };
        op_child.utf8_text(source.as_bytes()).unwrap_or("")
    }

    #[must_use]
    pub fn right(&self) -> Option<Expr<'tree>> {
        self.0.field("right")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Ternary<'tree>(pub Node<'tree>);

impl_ast_node!(Ternary, "ternary_operator");

impl<'tree> Ternary<'tree> {
    #[must_use]
    pub fn condition(&self) -> Option<Expr<'tree>> {
        self.0.field("condition")
    }

    #[must_use]
    pub fn consequence(&self) -> Option<Expr<'tree>> {
        self.0.field("consequence")
    }

    #[must_use]
    pub fn alternative(&self) -> Option<Expr<'tree>> {
        self.0.field("alternative")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Bin<'tree>(pub Node<'tree>);

impl_ast_node!(Bin, "binary_operator");

impl<'tree> Bin<'tree> {
    pub fn left(&self) -> Option<Expr<'tree>> {
        self.0.child(0).map(Into::into)
    }

    #[must_use]
    pub fn operator(&self) -> Option<Node<'tree>> {
        self.0.field("operator_name")
    }

    #[must_use]
    pub fn operator_name(&self, source: &'tree str) -> &'tree str {
        let Some(op_child): Option<Node<'tree>> = self.0.field("operator_name") else {
            return "";
        };
        op_child.utf8_text(source.as_bytes()).unwrap_or("")
    }

    pub fn right(&self) -> Option<Expr<'tree>> {
        self.0.child(self.0.child_count() - 1).map(Into::into)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Unary<'tree>(pub Node<'tree>);

impl_ast_node!(Unary, "unary_operator");

impl<'tree> Unary<'tree> {
    #[must_use]
    pub fn operator(&self) -> Option<Node<'tree>> {
        self.0.field("operator_name")
    }

    #[must_use]
    pub fn operator_name(&self, source: &'tree str) -> &'tree str {
        let Some(op_child): Option<Node<'tree>> = self.0.field("operator_name") else {
            return "";
        };
        op_child.utf8_text(source.as_bytes()).unwrap_or("")
    }

    #[must_use]
    pub fn argument(&self) -> Option<Expr<'tree>> {
        self.0.field("argument")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Lazy<'tree>(pub Node<'tree>);

impl_ast_node!(Lazy, "lazy_expression");

impl<'tree> Lazy<'tree> {
    #[must_use]
    pub fn expr(&self) -> Option<Expr<'tree>> {
        self.0.field("argument")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AsCast<'tree>(pub Node<'tree>);

impl_ast_node!(AsCast, "cast_as_operator");

impl<'tree> AsCast<'tree> {
    #[must_use]
    pub fn expr(&self) -> Option<Expr<'tree>> {
        self.0.field("expr")
    }

    #[must_use]
    pub fn casted_to(&self) -> Option<Type<'tree>> {
        self.0.field("casted_to")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct IsType<'tree>(pub Node<'tree>);

impl_ast_node!(IsType, "is_type_operator");

impl<'tree> IsType<'tree> {
    #[must_use]
    pub fn expr(&self) -> Option<Expr<'tree>> {
        self.0.field("expr")
    }

    #[must_use]
    pub fn operator(&self) -> Option<Node<'tree>> {
        self.0.field("operator")
    }

    #[must_use]
    pub fn operator_name(&self, source: &'tree str) -> &'tree str {
        let Some(op_child): Option<Node<'tree>> = self.0.field("operator") else {
            return "";
        };
        op_child.utf8_text(source.as_bytes()).unwrap_or("")
    }

    #[must_use]
    pub fn rhs_type(&self) -> Option<Type<'tree>> {
        self.0.field("rhs_type")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NotNull<'tree>(pub Node<'tree>);

impl_ast_node!(NotNull, "not_null_operator");

impl<'tree> NotNull<'tree> {
    #[must_use]
    pub fn inner(&self) -> Option<Expr<'tree>> {
        self.0.field("inner")
    }
}

#[derive(Clone, Copy, Debug)]
pub enum DotAccessField<'tree> {
    Ident(Ident<'tree>),
    NumericIndex(NumericIndex<'tree>),
}

impl<'tree> TryFromNode<'tree> for DotAccessField<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        match node.kind_bytes() {
            b"identifier" => Ok(DotAccessField::Ident(Ident(node))),
            b"numeric_index" => Ok(DotAccessField::NumericIndex(NumericIndex(node))),
            _ => Err(InvalidNodeKindError {
                expected: "identifier or numeric_index",
                actual: node.kind().to_string(),
            }),
        }
    }
}

impl<'tree> AstNode<'tree> for DotAccessField<'tree> {
    fn syntax(&self) -> Node<'tree> {
        match self {
            DotAccessField::Ident(node) => node.0,
            DotAccessField::NumericIndex(node) => node.0,
        }
    }
}

impl<'t> From<Node<'t>> for DotAccessField<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"identifier" => DotAccessField::Ident(Ident(node)),
            b"numeric_index" => DotAccessField::NumericIndex(NumericIndex(node)),
            _ => panic!("Unexpected dot access field kind: {}", node.kind()),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DotAccess<'tree>(pub Node<'tree>);

impl_ast_node!(DotAccess, "dot_access");

impl<'tree> DotAccess<'tree> {
    #[must_use]
    pub fn is_obj(&self, node: &Node<'tree>) -> bool {
        self.obj().is_some_and(|o| {
            let n = o.syntax();
            node.start_byte() >= n.start_byte() && node.end_byte() <= n.end_byte()
        })
    }

    #[must_use]
    pub fn obj(&self) -> Option<Expr<'tree>> {
        self.0.field("obj")
    }

    #[must_use]
    pub fn field(&self) -> Option<DotAccessField<'tree>> {
        self.0.field("field")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Call<'tree>(pub Node<'tree>);

impl_ast_node!(Call, "function_call");

impl<'tree> Call<'tree> {
    #[must_use]
    pub fn callee(&self) -> Option<Expr<'tree>> {
        self.0.field("callee")
    }

    #[must_use]
    pub fn callee_identifier(&self) -> Option<Node<'tree>> {
        let callee = self.callee()?;
        match callee {
            Expr::DotAccess(dot_access) => Some(dot_access.field()?.syntax()),
            Expr::Instantiation(inst) => match inst.expr()? {
                Expr::DotAccess(dot_access) => Some(dot_access.field()?.syntax()),
                Expr::Ident(ident) => Some(ident.syntax()),
                _ => None,
            },
            Expr::Ident(ident) => Some(ident.syntax()),
            _ => None,
        }
    }

    #[must_use]
    pub fn arguments(&self) -> AstChildren<'tree, CallArgument<'tree>> {
        self.0
            .field::<ArgumentList<'_>>("arguments")
            .map(|args| args.arguments())
            .unwrap_or_default()
    }

    #[must_use]
    pub fn callee_qualifier(&self) -> Option<Expr<'tree>> {
        let callee = self.callee()?;
        match callee {
            Expr::DotAccess(dot_access) => {
                Expr::try_from_node(dot_access.0.child_by_field_name("obj")?).ok()
            }
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Instantiation<'tree>(pub Node<'tree>);

impl_ast_node!(Instantiation, "generic_instantiation");

impl<'tree> Instantiation<'tree> {
    #[must_use]
    pub fn expr(&self) -> Option<Expr<'tree>> {
        self.0.field("expr")
    }

    #[must_use]
    pub fn instantiation_ts(&self) -> Option<InstantiationTList<'tree>> {
        self.0.field("instantiationTs")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Paren<'tree>(pub Node<'tree>);

impl_ast_node!(Paren, "parenthesized_expression");

impl<'tree> Paren<'tree> {
    #[must_use]
    pub fn inner(&self) -> Option<Expr<'tree>> {
        self.0.field("inner")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Match<'tree>(pub Node<'tree>);

impl_ast_node!(Match, "match_expression");

impl<'tree> Match<'tree> {
    #[must_use]
    pub fn expr(&self) -> Option<Expr<'tree>> {
        self.0.field("expr")
    }

    #[must_use]
    pub fn arms(&self) -> AstChildren<'tree, MatchArm<'tree>> {
        self.0
            .field::<MatchBody<'_>>("body")
            .map(|body| body.arms())
            .unwrap_or_default()
    }

    #[must_use]
    pub fn body(&self) -> Option<MatchBody<'tree>> {
        self.0.field("body")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ObjectLit<'tree>(pub Node<'tree>);

impl_ast_node!(ObjectLit, "object_literal");

impl<'tree> ObjectLit<'tree> {
    #[must_use]
    pub fn typ(&self) -> Option<Type<'tree>> {
        self.0.field("type")
    }

    #[must_use]
    pub fn arguments(&self) -> AstChildren<'tree, InstanceArg<'tree>> {
        self.0
            .field::<ObjectLiteralBody<'_>>("arguments")
            .map(|body| body.arguments())
            .unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Tensor<'tree>(pub Node<'tree>);

impl_ast_node!(Tensor, "tensor_expression");

impl<'tree> Tensor<'tree> {
    #[must_use]
    pub fn elements(&self) -> AstChildren<'tree, Expr<'tree>> {
        AstChildren::new(self.0)
    }
}

impl<'tree> Tuple<'tree> {
    #[must_use]
    pub fn typ(&self) -> Option<Type<'tree>> {
        self.0.field("type")
    }

    #[must_use]
    pub fn elements(&self) -> AstChildren<'tree, Expr<'tree>> {
        AstChildren::new(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Tuple<'tree>(pub Node<'tree>);

impl_ast_node!(Tuple, "typed_tuple");

#[derive(Clone, Copy, Debug)]
pub struct Lambda<'tree>(pub Node<'tree>);

impl_ast_node!(Lambda, "lambda_expression");

impl<'tree> Lambda<'tree> {
    pub fn parameters(&self) -> AstChildren<'tree, LambdaParameter<'tree>> {
        self.0
            .child_by_field_name("parameters")
            .map(AstChildren::new)
            .unwrap_or_default()
    }

    #[must_use]
    pub fn body(&self) -> Option<Block<'tree>> {
        self.0.field("body")
    }

    #[must_use]
    pub fn return_type(&self) -> Option<Type<'tree>> {
        self.0.field("return_type")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LambdaParameter<'tree>(pub Node<'tree>);

impl<'tree> TryFromNode<'tree> for LambdaParameter<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        if node.kind_bytes() == b"lambda_parameter" || node.kind_bytes() == b"parameter_declaration"
        {
            Ok(Self(node))
        } else {
            Err(InvalidNodeKindError {
                expected: "lambda_parameter or parameter_declaration",
                actual: node.kind().to_string(),
            })
        }
    }
}

impl<'tree> From<Node<'tree>> for LambdaParameter<'tree> {
    fn from(n: Node<'tree>) -> Self {
        Self(n)
    }
}

impl<'tree> AstNode<'tree> for LambdaParameter<'tree> {
    fn syntax(&self) -> Node<'tree> {
        self.0
    }
}

impl<'tree> LambdaParameter<'tree> {
    #[must_use]
    pub fn typ(&self) -> Option<Type<'tree>> {
        self.0.field("type")
    }

    #[must_use]
    pub fn default(&self) -> Option<Expr<'tree>> {
        self.0.field("default")
    }

    #[must_use]
    pub fn mutate(&self) -> bool {
        self.0.field::<Ident<'_>>("mutate").is_some()
    }
}

impl<'tree> HasName<'tree> for LambdaParameter<'tree> {
    type Name = Ident<'tree>;

    fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VarKind {
    Var,
    Val,
}

impl VarKind {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            VarKind::Var => "var",
            VarKind::Val => "val",
        }
    }
}

impl<'tree> From<Node<'tree>> for VarKind {
    fn from(node: Node<'tree>) -> Self {
        match node.kind_bytes() {
            b"val" => VarKind::Val,
            _ => VarKind::Var,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct VarDeclLhs<'tree>(pub Node<'tree>);

impl_ast_node!(VarDeclLhs, "var_declaration_lhs");

impl<'tree> VarDeclLhs<'tree> {
    pub fn kind(&self) -> VarKind {
        self.kind_node().map_or(VarKind::Var, VarKind::from)
    }

    pub fn kind_node(&self) -> Option<Node<'tree>> {
        self.0.field("kind")
    }

    #[must_use]
    pub fn pattern(&self) -> Option<VarDeclPattern<'tree>> {
        let mut cursor = self.0.walk();
        self.0
            .children(&mut cursor)
            .find(|n| {
                matches!(
                    n.kind_bytes(),
                    b"tuple_vars_declaration" | b"tensor_vars_declaration" | b"var_declaration"
                )
            })
            .map(Into::into)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum VarDeclPattern<'tree> {
    TupleVars(TupleVars<'tree>),
    TensorVars(TensorVars<'tree>),
    VarDecl(VarDecl<'tree>),
}

impl<'t> From<Node<'t>> for VarDeclPattern<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"tuple_vars_declaration" => VarDeclPattern::TupleVars(TupleVars(node)),
            b"tensor_vars_declaration" => VarDeclPattern::TensorVars(TensorVars(node)),
            b"var_declaration" => VarDeclPattern::VarDecl(VarDecl(node)),
            _ => panic!("Unexpected var declaration pattern kind: {}", node.kind()),
        }
    }
}

impl<'tree> TryFromNode<'tree> for VarDeclPattern<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        match node.kind_bytes() {
            b"tuple_vars_declaration" => Ok(VarDeclPattern::TupleVars(TupleVars(node))),
            b"tensor_vars_declaration" => Ok(VarDeclPattern::TensorVars(TensorVars(node))),
            b"var_declaration" => Ok(VarDeclPattern::VarDecl(VarDecl(node))),
            _ => Err(InvalidNodeKindError {
                expected: "tuple_vars_declaration, tensor_vars_declaration, or var_declaration",
                actual: node.kind().to_string(),
            }),
        }
    }
}

impl<'tree> AstNode<'tree> for VarDeclPattern<'tree> {
    fn syntax(&self) -> Node<'tree> {
        match self {
            VarDeclPattern::TupleVars(t) => t.0,
            VarDeclPattern::TensorVars(t) => t.0,
            VarDeclPattern::VarDecl(v) => v.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TupleVars<'tree>(pub Node<'tree>);

impl_ast_node!(TupleVars, "tuple_vars_declaration");

impl<'tree> TupleVars<'tree> {
    pub fn vars(&self) -> AstChildren<'tree, VarDeclPattern<'tree>> {
        AstChildren::new(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TensorVars<'tree>(pub Node<'tree>);

impl_ast_node!(TensorVars, "tensor_vars_declaration");

impl<'tree> TensorVars<'tree> {
    pub fn vars(&self) -> AstChildren<'tree, VarDeclPattern<'tree>> {
        AstChildren::new(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct VarDecl<'tree>(pub Node<'tree>);

impl_ast_node!(VarDecl, "var_declaration");

impl<'tree> VarDecl<'tree> {
    #[must_use]
    pub fn typ(&self) -> Option<Type<'tree>> {
        self.0.field("type")
    }

    #[must_use]
    pub fn is_redefinition(&self) -> bool {
        self.0.field::<Ident<'_>>("redef").is_some()
    }
}

impl<'tree> HasName<'tree> for VarDecl<'tree> {
    type Name = Ident<'tree>;

    fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NullLit<'tree>(pub Node<'tree>);

impl_ast_node!(NullLit, "null_literal");

#[derive(Clone, Copy, Debug)]
pub struct Underscore<'tree>(pub Node<'tree>);

impl_ast_node!(Underscore, "underscore");

#[derive(Clone, Copy, Debug)]
pub struct ArgumentList<'tree>(pub Node<'tree>);

impl_ast_node!(ArgumentList, "argument_list");

impl<'tree> ArgumentList<'tree> {
    pub fn arguments(&self) -> AstChildren<'tree, CallArgument<'tree>> {
        AstChildren::new(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CallArgument<'tree>(pub Node<'tree>);

impl_ast_node!(CallArgument, "call_argument");

impl<'tree> CallArgument<'tree> {
    #[must_use]
    pub fn mutate(&self) -> bool {
        self.0.child(0).is_some_and(|n| n.kind_bytes() == b"mutate")
    }

    #[must_use]
    pub fn expr(&self) -> Option<Expr<'tree>> {
        self.0.field("expr")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MatchBody<'tree>(pub Node<'tree>);

impl_ast_node!(MatchBody, "match_body");

impl<'tree> MatchBody<'tree> {
    pub fn arms(&self) -> AstChildren<'tree, MatchArm<'tree>> {
        AstChildren::new(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum MatchPattern<'tree> {
    Type(Type<'tree>),
    Expr(Expr<'tree>),
    Else,
}

impl<'t> From<Node<'t>> for MatchPattern<'t> {
    fn from(node: Node<'t>) -> Self {
        if let Some(pattern_type) = node.child_by_field_name("pattern_type") {
            MatchPattern::Type(pattern_type.into())
        } else if let Some(pattern_expr) = node.child_by_field_name("pattern_expr") {
            MatchPattern::Expr(pattern_expr.into())
        } else {
            MatchPattern::Else
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MatchArm<'tree>(pub Node<'tree>);

impl_ast_node!(MatchArm, "match_arm");

impl<'tree> MatchArm<'tree> {
    #[must_use]
    pub fn pattern(&self) -> MatchPattern<'tree> {
        if let Some(pattern_type) = self.0.field("pattern_type") {
            MatchPattern::Type(pattern_type)
        } else if let Some(pattern_expr) = self.0.field("pattern_expr") {
            MatchPattern::Expr(pattern_expr)
        } else {
            MatchPattern::Else
        }
    }

    #[must_use]
    pub fn body(&self) -> Option<MatchArmBody<'tree>> {
        if let Some(block) = self.0.field("block") {
            return Some(MatchArmBody::Block(block));
        }
        if let Some(ret) = self.0.field("return") {
            return Some(MatchArmBody::Return(ret));
        }
        if let Some(throw) = self.0.field("throw") {
            return Some(MatchArmBody::Throw(throw));
        }
        if let Some(expr) = self.0.field("expr") {
            return Some(MatchArmBody::Expr(expr));
        }
        None
    }
}

#[derive(Clone, Copy, Debug)]
pub enum MatchArmBody<'tree> {
    Block(Block<'tree>),
    Return(crate::ast::statements::Return<'tree>),
    Throw(crate::ast::statements::Throw<'tree>),
    Expr(Expr<'tree>),
}

impl<'t> From<Node<'t>> for MatchArmBody<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"block_statement" => MatchArmBody::Block(Block(node)),
            b"return_statement" => MatchArmBody::Return(crate::ast::statements::Return(node)),
            b"throw_statement" => MatchArmBody::Throw(crate::ast::statements::Throw(node)),
            _ => MatchArmBody::Expr(Expr::from(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ObjectLiteralBody<'tree>(pub Node<'tree>);

impl_ast_node!(ObjectLiteralBody, "object_literal_body");

impl<'tree> ObjectLiteralBody<'tree> {
    pub fn arguments(&self) -> AstChildren<'tree, InstanceArg<'tree>> {
        AstChildren::new(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstanceArg<'tree>(pub Node<'tree>);

impl_ast_node!(InstanceArg, "instance_argument");

impl<'tree> InstanceArg<'tree> {
    #[must_use]
    pub fn value(&self) -> Option<Expr<'tree>> {
        self.0.field("value")
    }
}

impl<'tree> HasName<'tree> for InstanceArg<'tree> {
    type Name = Ident<'tree>;

    fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }
}
