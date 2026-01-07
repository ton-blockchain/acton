use crate::BlockStatement;
use crate::node::{NodeFieldExt, RawNode};
use crate::types::{InstantiationTList, Type};
use tree_sitter::Node;

#[derive(Clone, Copy, Debug)]
pub struct Ident<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for Ident<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> Ident<'tree> {
    pub fn text(&self, source: &'tree str) -> &'tree str {
        self.0
            .utf8_text(source.as_bytes())
            .unwrap_or("<invalid utf8>")
    }

    pub fn raw_node(&self) -> &Node<'tree> {
        &self.0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StringLiteral<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for StringLiteral<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> StringLiteral<'tree> {
    pub fn text(&self, source: &'tree str) -> &'tree str {
        self.0
            .utf8_text(source.as_bytes())
            .unwrap_or("<invalid utf8>")
    }

    pub fn content(&self, source: &'tree str) -> &'tree str {
        self.text(source).trim_matches('"')
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NumberLiteral<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for NumberLiteral<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> NumberLiteral<'tree> {
    pub fn text(&self, source: &'tree str) -> &'tree str {
        self.0
            .utf8_text(source.as_bytes())
            .unwrap_or("<invalid utf8>")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BooleanLiteral<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for BooleanLiteral<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> BooleanLiteral<'tree> {
    pub fn value(&self, source: &'tree str) -> bool {
        self.0.utf8_text(source.as_bytes()).unwrap_or("false") == "true"
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NumericIndex<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for NumericIndex<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> NumericIndex<'tree> {
    pub fn value(&self, source: &'tree str) -> &'tree str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Expression<'tree> {
    Assignment(Assignment<'tree>),
    SetAssignment(SetAssignment<'tree>),
    TernaryOperator(TernaryOperator<'tree>),
    BinaryOperator(BinaryOperator<'tree>),
    UnaryOperator(UnaryOperator<'tree>),
    LazyExpression(LazyExpression<'tree>),
    CastAsOperator(CastAsOperator<'tree>),
    IsTypeOperator(IsTypeOperator<'tree>),
    NotNullOperator(NotNullOperator<'tree>),
    DotAccess(DotAccess<'tree>),
    FunctionCall(FunctionCall<'tree>),
    GenericInstantiation(GenericInstantiation<'tree>),
    ParenthesizedExpression(ParenthesizedExpression<'tree>),
    MatchExpression(MatchExpression<'tree>),
    ObjectLiteral(ObjectLiteral<'tree>),
    TensorExpression(TensorExpression<'tree>),
    TypedTuple(TypedTuple<'tree>),
    LambdaExpression(LambdaExpression<'tree>),
    NumberLiteral(NumberLiteral<'tree>),
    StringLiteral(StringLiteral<'tree>),
    BooleanLiteral(BooleanLiteral<'tree>),
    NullLiteral(NullLiteral<'tree>),
    Underscore(Underscore<'tree>),
    Ident(Ident<'tree>),
    NumericIndex(NumericIndex<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> Expression<'tree> {
    pub fn text(&self, source: &'tree str) -> &'tree str {
        self.raw_node().utf8_text(source.as_bytes()).unwrap_or("")
    }

    pub fn raw_node(&self) -> Node<'tree> {
        match self {
            Expression::Assignment(n) => n.0,
            Expression::SetAssignment(n) => n.0,
            Expression::TernaryOperator(n) => n.0,
            Expression::BinaryOperator(n) => n.0,
            Expression::UnaryOperator(n) => n.0,
            Expression::LazyExpression(n) => n.0,
            Expression::CastAsOperator(n) => n.0,
            Expression::IsTypeOperator(n) => n.0,
            Expression::NotNullOperator(n) => n.0,
            Expression::DotAccess(n) => n.0,
            Expression::FunctionCall(n) => n.0,
            Expression::GenericInstantiation(n) => n.0,
            Expression::ParenthesizedExpression(n) => n.0,
            Expression::MatchExpression(n) => n.0,
            Expression::ObjectLiteral(n) => n.0,
            Expression::TensorExpression(n) => n.0,
            Expression::TypedTuple(n) => n.0,
            Expression::LambdaExpression(n) => n.0,
            Expression::NumberLiteral(n) => n.0,
            Expression::StringLiteral(n) => n.0,
            Expression::BooleanLiteral(n) => n.0,
            Expression::NullLiteral(n) => n.0,
            Expression::Underscore(n) => n.0,
            Expression::Ident(n) => n.0,
            Expression::NumericIndex(n) => n.0,
            Expression::Unmapped(n) => n.0,
        }
    }
}

impl<'t> From<Node<'t>> for Expression<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind() {
            "assignment" => Expression::Assignment(Assignment(node)),
            "set_assignment" => Expression::SetAssignment(SetAssignment(node)),
            "ternary_operator" => Expression::TernaryOperator(TernaryOperator(node)),
            "binary_operator" => Expression::BinaryOperator(BinaryOperator(node)),
            "unary_operator" => Expression::UnaryOperator(UnaryOperator(node)),
            "lazy_expression" => Expression::LazyExpression(LazyExpression(node)),
            "cast_as_operator" => Expression::CastAsOperator(CastAsOperator(node)),
            "is_type_operator" => Expression::IsTypeOperator(IsTypeOperator(node)),
            "not_null_operator" => Expression::NotNullOperator(NotNullOperator(node)),
            "dot_access" => Expression::DotAccess(DotAccess(node)),
            "function_call" => Expression::FunctionCall(FunctionCall(node)),
            "generic_instantiation" => Expression::GenericInstantiation(GenericInstantiation(node)),
            "parenthesized_expression" => {
                Expression::ParenthesizedExpression(ParenthesizedExpression(node))
            }
            "match_expression" => Expression::MatchExpression(MatchExpression(node)),
            "object_literal" => Expression::ObjectLiteral(ObjectLiteral(node)),
            "tensor_expression" => Expression::TensorExpression(TensorExpression(node)),
            "typed_tuple" => Expression::TypedTuple(TypedTuple(node)),
            "lambda_expression" => Expression::LambdaExpression(LambdaExpression(node)),
            "number_literal" => Expression::NumberLiteral(NumberLiteral(node)),
            "string_literal" => Expression::StringLiteral(StringLiteral(node)),
            "boolean_literal" => Expression::BooleanLiteral(BooleanLiteral(node)),
            "null_literal" => Expression::NullLiteral(NullLiteral(node)),
            "underscore" => Expression::Underscore(Underscore(node)),
            "identifier" => Expression::Ident(Ident(node)),
            "numeric_index" => Expression::NumericIndex(NumericIndex(node)),
            _ => Expression::Unmapped(RawNode::new(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Assignment<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for Assignment<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> Assignment<'tree> {
    pub fn left(&self) -> Option<Expression<'tree>> {
        self.0.field("left")
    }

    pub fn right(&self) -> Option<Expression<'tree>> {
        self.0.field("right")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SetAssignment<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for SetAssignment<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> SetAssignment<'tree> {
    pub fn left(&self) -> Option<Expression<'tree>> {
        self.0.field("left")
    }

    pub fn operator_name(&self, source: &'tree str) -> &'tree str {
        let Some(op_child): Option<Node<'tree>> = self.0.field("operator_name") else {
            return "";
        };
        op_child.utf8_text(source.as_bytes()).unwrap_or("")
    }

    pub fn right(&self) -> Option<Expression<'tree>> {
        self.0.field("right")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TernaryOperator<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for TernaryOperator<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> TernaryOperator<'tree> {
    pub fn condition(&self) -> Option<Expression<'tree>> {
        self.0.field("condition")
    }

    pub fn consequence(&self) -> Option<Expression<'tree>> {
        self.0.field("consequence")
    }

    pub fn alternative(&self) -> Option<Expression<'tree>> {
        self.0.field("alternative")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BinaryOperator<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for BinaryOperator<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> BinaryOperator<'tree> {
    pub fn left(&self) -> Option<Expression<'tree>> {
        self.0.child(0).map(Into::into)
    }

    pub fn operator(&self) -> Option<Node<'tree>> {
        self.0.field("operator_name")
    }

    pub fn operator_name(&self, source: &'tree str) -> &'tree str {
        let Some(op_child): Option<Node<'tree>> = self.0.field("operator_name") else {
            return "";
        };
        op_child.utf8_text(source.as_bytes()).unwrap_or("")
    }

    pub fn right(&self) -> Option<Expression<'tree>> {
        self.0.child(self.0.child_count() - 1).map(Into::into)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct UnaryOperator<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for UnaryOperator<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> UnaryOperator<'tree> {
    pub fn operator(&self) -> Option<Node<'tree>> {
        self.0.field("operator_name")
    }

    pub fn operator_name(&self, source: &'tree str) -> &'tree str {
        let Some(op_child): Option<Node<'tree>> = self.0.field("operator_name") else {
            return "";
        };
        op_child.utf8_text(source.as_bytes()).unwrap_or("")
    }

    pub fn argument(&self) -> Option<Expression<'tree>> {
        self.0.field("argument")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LazyExpression<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for LazyExpression<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> LazyExpression<'tree> {
    pub fn expr(&self) -> Option<Expression<'tree>> {
        self.0.field("argument")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CastAsOperator<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for CastAsOperator<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> CastAsOperator<'tree> {
    pub fn expr(&self) -> Option<Expression<'tree>> {
        self.0.field("expr")
    }

    pub fn casted_to(&self) -> Option<Type<'tree>> {
        self.0.field("casted_to")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct IsTypeOperator<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for IsTypeOperator<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> IsTypeOperator<'tree> {
    pub fn expr(&self) -> Option<Expression<'tree>> {
        self.0.field("expr")
    }

    pub fn operator(&self) -> Option<Node<'tree>> {
        self.0.field("operator")
    }

    pub fn operator_name(&self, source: &'tree str) -> &'tree str {
        let Some(op_child): Option<Node<'tree>> = self.0.field("operator") else {
            return "";
        };
        op_child.utf8_text(source.as_bytes()).unwrap_or("")
    }

    pub fn rhs_type(&self) -> Option<Type<'tree>> {
        self.0.field("rhs_type")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NotNullOperator<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for NotNullOperator<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> NotNullOperator<'tree> {
    pub fn inner(&self) -> Option<Expression<'tree>> {
        self.0.field("inner")
    }

    pub fn raw_node(&self) -> &Node<'tree> {
        &self.0
    }
}

#[derive(Clone, Copy, Debug)]
pub enum DotAccessField<'tree> {
    Ident(Ident<'tree>),
    NumericIndex(NumericIndex<'tree>),
}

impl<'t> From<Node<'t>> for DotAccessField<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind() {
            "identifier" => DotAccessField::Ident(Ident(node)),
            "numeric_index" => DotAccessField::NumericIndex(NumericIndex(node)),
            _ => panic!("Unexpected dot access field kind: {}", node.kind()),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DotAccess<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for DotAccess<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> DotAccess<'tree> {
    pub fn obj(&self) -> Option<Expression<'tree>> {
        self.0.field("obj")
    }

    pub fn field(&self) -> Option<DotAccessField<'tree>> {
        self.0.field("field")
    }

    pub fn raw_node(&self) -> &Node<'tree> {
        &self.0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FunctionCall<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for FunctionCall<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> FunctionCall<'tree> {
    pub fn callee(&self) -> Option<Expression<'tree>> {
        self.0.field("callee")
    }

    pub fn arguments(&self) -> Vec<CallArgument<'tree>> {
        let Some(args) = self.0.field::<ArgumentList>("arguments") else {
            return vec![];
        };
        args.arguments()
    }

    pub fn callee_qualifier(&self) -> Option<Node<'tree>> {
        let callee = self.callee()?;
        match callee {
            Expression::DotAccess(dot_access) => dot_access.0.child_by_field_name("obj"),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GenericInstantiation<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for GenericInstantiation<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> GenericInstantiation<'tree> {
    pub fn expr(&self) -> Option<Expression<'tree>> {
        self.0.field("expr")
    }

    pub fn instantiation_ts(&self) -> Option<InstantiationTList<'tree>> {
        self.0.field("instantiationTs")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ParenthesizedExpression<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for ParenthesizedExpression<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> ParenthesizedExpression<'tree> {
    pub fn inner(&self) -> Option<Expression<'tree>> {
        self.0.field("inner")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MatchExpression<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for MatchExpression<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum MatchExpr<'tree> {
    Expression(Expression<'tree>),
    LocalVarsDeclaration(crate::statements::LocalVarsDeclaration<'tree>),
}

impl<'t> From<Node<'t>> for MatchExpr<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind() {
            "local_vars_declaration" => {
                MatchExpr::LocalVarsDeclaration(crate::statements::LocalVarsDeclaration(node))
            }
            _ => MatchExpr::Expression(Expression::from(node)),
        }
    }
}

impl<'tree> MatchExpression<'tree> {
    pub fn expr(&self) -> Option<MatchExpr<'tree>> {
        self.0.field("expr")
    }

    pub fn body(&self) -> Option<MatchBody<'tree>> {
        self.0.field("body")
    }

    pub fn raw_node(&self) -> &Node<'tree> {
        &self.0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ObjectLiteral<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for ObjectLiteral<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> ObjectLiteral<'tree> {
    pub fn typ(&self) -> Option<Type<'tree>> {
        self.0.field("type")
    }

    pub fn arguments(&self) -> Vec<InstanceArgument<'tree>> {
        let Some(body) = self.0.field::<ObjectLiteralBody>("arguments") else {
            return vec![];
        };
        body.arguments()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TensorExpression<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for TensorExpression<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> TensorExpression<'tree> {
    pub fn elements(&self) -> Vec<Expression<'tree>> {
        let mut cursor = self.0.walk();
        self.0
            .children(&mut cursor)
            .filter(|n| n.is_named())
            .map(|n| n.into())
            .collect()
    }
}

impl<'tree> TypedTuple<'tree> {
    pub fn elements(&self) -> Vec<Expression<'tree>> {
        let mut cursor = self.0.walk();
        self.0
            .children(&mut cursor)
            .filter(|n| n.is_named())
            .map(|n| n.into())
            .collect()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TypedTuple<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for TypedTuple<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LambdaExpression<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for LambdaExpression<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> LambdaExpression<'tree> {
    pub fn parameters(&self) -> Vec<LambdaParameter<'tree>> {
        let mut cursor = self.0.walk();
        self.0
            .children(&mut cursor)
            .filter(|n| n.kind() == "lambda_parameter")
            .map(LambdaParameter)
            .collect()
    }

    pub fn body(&self) -> Option<BlockStatement<'tree>> {
        self.0.field("body")
    }

    pub fn return_type(&self) -> Option<Type<'tree>> {
        self.0.field("return_type")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LambdaParameter<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for LambdaParameter<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> LambdaParameter<'tree> {
    pub fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }

    pub fn typ(&self) -> Option<Type<'tree>> {
        self.0.field("type")
    }

    pub fn mutate(&self) -> bool {
        self.0.field::<Ident>("mutate").is_some()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NullLiteral<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for NullLiteral<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Underscore<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for Underscore<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ArgumentList<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for ArgumentList<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> ArgumentList<'tree> {
    pub fn arguments(&self) -> Vec<CallArgument<'tree>> {
        let mut cursor = self.0.walk();
        self.0
            .children(&mut cursor)
            .filter(|n| n.kind() == "call_argument")
            .map(CallArgument)
            .collect()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CallArgument<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for CallArgument<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> CallArgument<'tree> {
    pub fn mutate(&self) -> bool {
        self.0
            .child(0)
            .map(|n| n.kind() == "mutate")
            .unwrap_or(false)
    }

    pub fn expr(&self) -> Option<Expression<'tree>> {
        self.0.field("expr")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MatchBody<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for MatchBody<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> MatchBody<'tree> {
    pub fn arms(&self) -> Vec<MatchArm<'tree>> {
        let mut cursor = self.0.walk();
        self.0
            .children(&mut cursor)
            .filter(|n| n.kind() == "match_arm")
            .map(MatchArm)
            .collect()
    }
}

#[derive(Clone, Copy, Debug)]
pub enum MatchPattern<'tree> {
    Type(Type<'tree>),
    Expression(Expression<'tree>),
    Else,
}

impl<'t> From<Node<'t>> for MatchPattern<'t> {
    fn from(node: Node<'t>) -> Self {
        if let Some(pattern_type) = node.child_by_field_name("pattern_type") {
            MatchPattern::Type(pattern_type.into())
        } else if let Some(pattern_expr) = node.child_by_field_name("pattern_expr") {
            MatchPattern::Expression(pattern_expr.into())
        } else {
            MatchPattern::Else
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MatchArm<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for MatchArm<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> MatchArm<'tree> {
    pub fn pattern(&self) -> MatchPattern<'tree> {
        if let Some(pattern_type) = self.0.field("pattern_type") {
            MatchPattern::Type(pattern_type)
        } else if let Some(pattern_expr) = self.0.field("pattern_expr") {
            MatchPattern::Expression(pattern_expr)
        } else {
            MatchPattern::Else
        }
    }

    pub fn body(&self) -> Option<MatchArmBody<'tree>> {
        if let Some(block) = self.0.field("block") {
            return Some(MatchArmBody::BlockStatement(block));
        }
        if let Some(ret) = self.0.field("return") {
            return Some(MatchArmBody::ReturnStatement(ret));
        }
        if let Some(throw) = self.0.field("throw") {
            return Some(MatchArmBody::ThrowStatement(throw));
        }
        if let Some(expr) = self.0.field("expr") {
            return Some(MatchArmBody::Expression(expr));
        }
        None
    }
}

#[derive(Clone, Copy, Debug)]
pub enum MatchArmBody<'tree> {
    BlockStatement(crate::statements::BlockStatement<'tree>),
    ReturnStatement(crate::statements::ReturnStatement<'tree>),
    ThrowStatement(crate::statements::ThrowStatement<'tree>),
    Expression(Expression<'tree>),
}

impl<'t> From<Node<'t>> for MatchArmBody<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind() {
            "block_statement" => {
                MatchArmBody::BlockStatement(crate::statements::BlockStatement(node))
            }
            "return_statement" => {
                MatchArmBody::ReturnStatement(crate::statements::ReturnStatement(node))
            }
            "throw_statement" => {
                MatchArmBody::ThrowStatement(crate::statements::ThrowStatement(node))
            }
            _ => MatchArmBody::Expression(Expression::from(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ObjectLiteralBody<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for ObjectLiteralBody<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> ObjectLiteralBody<'tree> {
    pub fn arguments(&self) -> Vec<InstanceArgument<'tree>> {
        let mut cursor = self.0.walk();
        self.0
            .children(&mut cursor)
            .filter(|n| n.kind() == "instance_argument")
            .map(InstanceArgument)
            .collect()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstanceArgument<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for InstanceArgument<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> InstanceArgument<'tree> {
    pub fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }

    pub fn value(&self) -> Option<Expression<'tree>> {
        self.0.field("value")
    }
}
