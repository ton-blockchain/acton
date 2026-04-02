use crate::ast::node::{AstChildren, RawNode};
use crate::impl_ast_node;
use ton_syntax::ast::{AstNode, AstNodeBytesKind, InvalidNodeKindError, TryFromNode};
use tree_sitter::Node;

#[derive(Clone, Copy, Debug)]
pub enum SimpleExpr<'tree> {
    NegateExpr(NegateExpr<'tree>),
    BinaryExpression(BinaryExpression<'tree>),
    RefExpr(RefExpr<'tree>),
    ParensExpr(ParensExpr<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> SimpleExpr<'tree> {
    #[must_use]
    pub fn text(&self, source: &'tree str) -> &'tree str {
        self.syntax().utf8_text(source.as_bytes()).unwrap_or("")
    }

    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            SimpleExpr::NegateExpr(node) => node.0,
            SimpleExpr::BinaryExpression(node) => node.0,
            SimpleExpr::RefExpr(node) => node.0,
            SimpleExpr::ParensExpr(node) => node.0,
            SimpleExpr::Unmapped(node) => node.0,
        }
    }
}

impl<'tree> TryFromNode<'tree> for SimpleExpr<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        match node.kind_bytes() {
            b"negate_expr" | b"binary_expression" | b"ref_expr" | b"parens_expr" => {
                Ok(Self::from(node))
            }
            _ => Err(InvalidNodeKindError {
                expected: "negate_expr|binary_expression|ref_expr|parens_expr",
                actual: node.kind().to_string(),
            }),
        }
    }
}

impl<'tree> AstNode<'tree> for SimpleExpr<'tree> {
    fn syntax(&self) -> Node<'tree> {
        self.syntax()
    }
}

impl<'t> From<Node<'t>> for SimpleExpr<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"negate_expr" => SimpleExpr::NegateExpr(NegateExpr(node)),
            b"binary_expression" => SimpleExpr::BinaryExpression(BinaryExpression(node)),
            b"ref_expr" => SimpleExpr::RefExpr(RefExpr(node)),
            b"parens_expr" => SimpleExpr::ParensExpr(ParensExpr(node)),
            _ => SimpleExpr::Unmapped(RawNode::new(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TypeParameter<'tree>(pub Node<'tree>);
impl_ast_node!(TypeParameter, "type_parameter");

impl<'tree> TypeParameter<'tree> {
    #[must_use]
    pub fn expr(&self) -> Option<SimpleExpr<'tree>> {
        self.0.named_child(0).map(SimpleExpr::from)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NegateExpr<'tree>(pub Node<'tree>);
impl_ast_node!(NegateExpr, "negate_expr");

impl<'tree> NegateExpr<'tree> {
    #[must_use]
    pub fn operand(&self) -> Option<SimpleExpr<'tree>> {
        self.0.field("operand")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BinaryExpression<'tree>(pub Node<'tree>);
impl_ast_node!(BinaryExpression, "binary_expression");

impl<'tree> BinaryExpression<'tree> {
    #[must_use]
    pub fn left(&self) -> Option<SimpleExpr<'tree>> {
        self.0.field("left")
    }

    #[must_use]
    pub fn right(&self) -> Option<BinaryRightExpr<'tree>> {
        self.0.field("right")
    }
}

#[derive(Clone, Copy, Debug)]
pub enum BinaryRightExpr<'tree> {
    SimpleExpr(SimpleExpr<'tree>),
    BitSizeExpr(BitSizeExpr<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> BinaryRightExpr<'tree> {
    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            BinaryRightExpr::SimpleExpr(node) => node.syntax(),
            BinaryRightExpr::BitSizeExpr(node) => node.0,
            BinaryRightExpr::Unmapped(node) => node.0,
        }
    }
}

impl<'tree> TryFromNode<'tree> for BinaryRightExpr<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        match node.kind_bytes() {
            b"simple_expr" | b"bit_size_expr" => Ok(Self::from(node)),
            _ => Err(InvalidNodeKindError {
                expected: "simple_expr|bit_size_expr",
                actual: node.kind().to_string(),
            }),
        }
    }
}

impl<'tree> AstNode<'tree> for BinaryRightExpr<'tree> {
    fn syntax(&self) -> Node<'tree> {
        self.syntax()
    }
}

impl<'t> From<Node<'t>> for BinaryRightExpr<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"simple_expr" => BinaryRightExpr::SimpleExpr(SimpleExpr::from(node)),
            b"bit_size_expr" => BinaryRightExpr::BitSizeExpr(BitSizeExpr(node)),
            _ => BinaryRightExpr::Unmapped(RawNode::new(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RefExpr<'tree>(pub Node<'tree>);
impl_ast_node!(RefExpr, "ref_expr");

impl<'tree> RefExpr<'tree> {
    #[must_use]
    pub fn value(&self) -> Option<RefExprValue<'tree>> {
        self.0.named_child(0).map(RefExprValue::from)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum RefExprValue<'tree> {
    RefInner(RefInner<'tree>),
    ParensExpr(ParensExpr<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> RefExprValue<'tree> {
    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            RefExprValue::RefInner(node) => node.0,
            RefExprValue::ParensExpr(node) => node.0,
            RefExprValue::Unmapped(node) => node.0,
        }
    }
}

impl<'tree> TryFromNode<'tree> for RefExprValue<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        match node.kind_bytes() {
            b"ref_inner" | b"parens_expr" => Ok(Self::from(node)),
            _ => Err(InvalidNodeKindError {
                expected: "ref_inner|parens_expr",
                actual: node.kind().to_string(),
            }),
        }
    }
}

impl<'tree> AstNode<'tree> for RefExprValue<'tree> {
    fn syntax(&self) -> Node<'tree> {
        self.syntax()
    }
}

impl<'t> From<Node<'t>> for RefExprValue<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"ref_inner" => RefExprValue::RefInner(RefInner(node)),
            b"parens_expr" => RefExprValue::ParensExpr(ParensExpr(node)),
            _ => RefExprValue::Unmapped(RawNode::new(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RefInner<'tree>(pub Node<'tree>);
impl_ast_node!(RefInner, "ref_inner");

impl<'tree> RefInner<'tree> {
    #[must_use]
    pub fn value(&self) -> Option<RefInnerValue<'tree>> {
        self.0.named_child(0).map(RefInnerValue::from)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum RefInnerValue<'tree> {
    TypeIdentifier(TypeIdentifier<'tree>),
    Number(NumberLit<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> RefInnerValue<'tree> {
    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            RefInnerValue::TypeIdentifier(node) => node.0,
            RefInnerValue::Number(node) => node.0,
            RefInnerValue::Unmapped(node) => node.0,
        }
    }
}

impl<'tree> TryFromNode<'tree> for RefInnerValue<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        match node.kind_bytes() {
            b"type_identifier" | b"number" => Ok(Self::from(node)),
            _ => Err(InvalidNodeKindError {
                expected: "type_identifier|number",
                actual: node.kind().to_string(),
            }),
        }
    }
}

impl<'tree> AstNode<'tree> for RefInnerValue<'tree> {
    fn syntax(&self) -> Node<'tree> {
        self.syntax()
    }
}

impl<'t> From<Node<'t>> for RefInnerValue<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"type_identifier" => RefInnerValue::TypeIdentifier(TypeIdentifier(node)),
            b"number" => RefInnerValue::Number(NumberLit(node)),
            _ => RefInnerValue::Unmapped(RawNode::new(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ParensExpr<'tree>(pub Node<'tree>);
impl_ast_node!(ParensExpr, "parens_expr");

impl<'tree> ParensExpr<'tree> {
    #[must_use]
    pub fn expr(&self) -> Option<SimpleExpr<'tree>> {
        self.0.named_child(0).map(SimpleExpr::from)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum CondExpr<'tree> {
    CondDotAndQuestionExpr(CondDotAndQuestionExpr<'tree>),
    CondQuestionExpr(CondQuestionExpr<'tree>),
    CondTypeExpr(CondTypeExpr<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> CondExpr<'tree> {
    #[must_use]
    pub fn text(&self, source: &'tree str) -> &'tree str {
        self.syntax().utf8_text(source.as_bytes()).unwrap_or("")
    }

    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            CondExpr::CondDotAndQuestionExpr(node) => node.0,
            CondExpr::CondQuestionExpr(node) => node.0,
            CondExpr::CondTypeExpr(node) => node.0,
            CondExpr::Unmapped(node) => node.0,
        }
    }
}

impl<'tree> TryFromNode<'tree> for CondExpr<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        match node.kind_bytes() {
            b"cond_dot_and_question_expr" | b"cond_question_expr" | b"cond_type_expr" => {
                Ok(Self::from(node))
            }
            _ => Err(InvalidNodeKindError {
                expected: "cond_dot_and_question_expr|cond_question_expr|cond_type_expr",
                actual: node.kind().to_string(),
            }),
        }
    }
}

impl<'tree> AstNode<'tree> for CondExpr<'tree> {
    fn syntax(&self) -> Node<'tree> {
        self.syntax()
    }
}

impl<'t> From<Node<'t>> for CondExpr<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"cond_dot_and_question_expr" => {
                CondExpr::CondDotAndQuestionExpr(CondDotAndQuestionExpr(node))
            }
            b"cond_question_expr" => CondExpr::CondQuestionExpr(CondQuestionExpr(node)),
            b"cond_type_expr" => CondExpr::CondTypeExpr(CondTypeExpr(node)),
            _ => CondExpr::Unmapped(RawNode::new(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CondDotAndQuestionExpr<'tree>(pub Node<'tree>);
impl_ast_node!(CondDotAndQuestionExpr, "cond_dot_and_question_expr");

impl<'tree> CondDotAndQuestionExpr<'tree> {
    #[must_use]
    pub fn dotted(&self) -> Option<CondDottedValue<'tree>> {
        self.0.named_child(0).map(CondDottedValue::from)
    }

    #[must_use]
    pub fn expr(&self) -> Option<TypeExpr<'tree>> {
        self.0.named_child(1).map(TypeExpr::from)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum CondDottedValue<'tree> {
    CondDotted(CondDotted<'tree>),
    ParensCondDotted(ParensCondDotted<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> CondDottedValue<'tree> {
    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            CondDottedValue::CondDotted(node) => node.0,
            CondDottedValue::ParensCondDotted(node) => node.0,
            CondDottedValue::Unmapped(node) => node.0,
        }
    }
}

impl<'tree> TryFromNode<'tree> for CondDottedValue<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        match node.kind_bytes() {
            b"cond_dotted" | b"parens_cond_dotted" => Ok(Self::from(node)),
            _ => Err(InvalidNodeKindError {
                expected: "cond_dotted|parens_cond_dotted",
                actual: node.kind().to_string(),
            }),
        }
    }
}

impl<'tree> AstNode<'tree> for CondDottedValue<'tree> {
    fn syntax(&self) -> Node<'tree> {
        self.syntax()
    }
}

impl<'t> From<Node<'t>> for CondDottedValue<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"cond_dotted" => CondDottedValue::CondDotted(CondDotted(node)),
            b"parens_cond_dotted" => CondDottedValue::ParensCondDotted(ParensCondDotted(node)),
            _ => CondDottedValue::Unmapped(RawNode::new(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CondDotted<'tree>(pub Node<'tree>);
impl_ast_node!(CondDotted, "cond_dotted");

impl<'tree> CondDotted<'tree> {
    #[must_use]
    pub fn base(&self) -> Option<TypeExpr<'tree>> {
        self.0.named_child(0).map(TypeExpr::from)
    }

    #[must_use]
    pub fn number(&self) -> Option<NumberLit<'tree>> {
        self.0.named_child(1).map(NumberLit)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ParensCondDotted<'tree>(pub Node<'tree>);
impl_ast_node!(ParensCondDotted, "parens_cond_dotted");

impl<'tree> ParensCondDotted<'tree> {
    #[must_use]
    pub fn expr(&self) -> Option<CondDotted<'tree>> {
        self.0.named_child(0).map(CondDotted)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CondQuestionExpr<'tree>(pub Node<'tree>);
impl_ast_node!(CondQuestionExpr, "cond_question_expr");

impl<'tree> CondQuestionExpr<'tree> {
    #[must_use]
    pub fn left(&self) -> Option<TypeExpr<'tree>> {
        self.0.named_child(0).map(TypeExpr::from)
    }

    #[must_use]
    pub fn right(&self) -> Option<TypeExpr<'tree>> {
        self.0.named_child(1).map(TypeExpr::from)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CondTypeExpr<'tree>(pub Node<'tree>);
impl_ast_node!(CondTypeExpr, "cond_type_expr");

impl<'tree> CondTypeExpr<'tree> {
    #[must_use]
    pub fn expr(&self) -> Option<TypeExpr<'tree>> {
        self.0.named_child(0).map(TypeExpr::from)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum TypeExpr<'tree> {
    CellRefExpr(CellRefExpr<'tree>),
    BuiltinExpr(BuiltinExpr<'tree>),
    CombinatorExpr(CombinatorExpr<'tree>),
    SimpleExpr(SimpleExpr<'tree>),
    ArrayType(ArrayType<'tree>),
    ArrayMultiplier(ArrayMultiplier<'tree>),
    BitSizeExpr(BitSizeExpr<'tree>),
    ParensTypeExpr(ParensTypeExpr<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> TypeExpr<'tree> {
    #[must_use]
    pub fn text(&self, source: &'tree str) -> &'tree str {
        self.syntax().utf8_text(source.as_bytes()).unwrap_or("")
    }

    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            TypeExpr::CellRefExpr(node) => node.0,
            TypeExpr::BuiltinExpr(node) => node.syntax(),
            TypeExpr::CombinatorExpr(node) => node.0,
            TypeExpr::SimpleExpr(node) => node.syntax(),
            TypeExpr::ArrayType(node) => node.0,
            TypeExpr::ArrayMultiplier(node) => node.0,
            TypeExpr::BitSizeExpr(node) => node.0,
            TypeExpr::ParensTypeExpr(node) => node.0,
            TypeExpr::Unmapped(node) => node.0,
        }
    }
}

impl<'tree> TryFromNode<'tree> for TypeExpr<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        match node.kind_bytes() {
            b"cell_ref_expr" | b"builtin_expr" | b"builtin_one_arg" | b"builtin_zero_args"
            | b"combinator_expr" | b"simple_expr" | b"array_type" | b"array_multiplier"
            | b"bit_size_expr" | b"parens_type_expr" => Ok(Self::from(node)),
            _ => Err(InvalidNodeKindError {
                expected: "type_expr-compatible node",
                actual: node.kind().to_string(),
            }),
        }
    }
}

impl<'tree> AstNode<'tree> for TypeExpr<'tree> {
    fn syntax(&self) -> Node<'tree> {
        self.syntax()
    }
}

impl<'t> From<Node<'t>> for TypeExpr<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"cell_ref_expr" => TypeExpr::CellRefExpr(CellRefExpr(node)),
            b"builtin_expr" | b"builtin_one_arg" | b"builtin_zero_args" => {
                TypeExpr::BuiltinExpr(BuiltinExpr::from(node))
            }
            b"combinator_expr" => TypeExpr::CombinatorExpr(CombinatorExpr(node)),
            b"simple_expr" | b"negate_expr" | b"binary_expression" | b"ref_expr"
            | b"parens_expr" => TypeExpr::SimpleExpr(SimpleExpr::from(node)),
            b"array_type" => TypeExpr::ArrayType(ArrayType(node)),
            b"array_multiplier" => TypeExpr::ArrayMultiplier(ArrayMultiplier(node)),
            b"bit_size_expr" => TypeExpr::BitSizeExpr(BitSizeExpr(node)),
            b"parens_type_expr" => TypeExpr::ParensTypeExpr(ParensTypeExpr(node)),
            _ => TypeExpr::Unmapped(RawNode::new(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CellRefExpr<'tree>(pub Node<'tree>);
impl_ast_node!(CellRefExpr, "cell_ref_expr");

impl<'tree> CellRefExpr<'tree> {
    #[must_use]
    pub fn expr(&self) -> Option<CellRefTarget<'tree>> {
        self.0.field("expr")
    }
}

#[derive(Clone, Copy, Debug)]
pub enum CellRefTarget<'tree> {
    CellRefInner(CellRefInner<'tree>),
    ParensCellRef(ParensCellRef<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> CellRefTarget<'tree> {
    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            CellRefTarget::CellRefInner(node) => node.0,
            CellRefTarget::ParensCellRef(node) => node.0,
            CellRefTarget::Unmapped(node) => node.0,
        }
    }
}

impl<'tree> TryFromNode<'tree> for CellRefTarget<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        match node.kind_bytes() {
            b"cell_ref_inner" | b"parens_cell_ref" => Ok(Self::from(node)),
            _ => Err(InvalidNodeKindError {
                expected: "cell_ref_inner|parens_cell_ref",
                actual: node.kind().to_string(),
            }),
        }
    }
}

impl<'tree> AstNode<'tree> for CellRefTarget<'tree> {
    fn syntax(&self) -> Node<'tree> {
        self.syntax()
    }
}

impl<'t> From<Node<'t>> for CellRefTarget<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"cell_ref_inner" => CellRefTarget::CellRefInner(CellRefInner(node)),
            b"parens_cell_ref" => CellRefTarget::ParensCellRef(ParensCellRef(node)),
            _ => CellRefTarget::Unmapped(RawNode::new(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CellRefInner<'tree>(pub Node<'tree>);
impl_ast_node!(CellRefInner, "cell_ref_inner");

impl<'tree> CellRefInner<'tree> {
    #[must_use]
    pub fn value(&self) -> Option<CellRefInnerValue<'tree>> {
        self.0.named_child(0).map(CellRefInnerValue::from)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum CellRefInnerValue<'tree> {
    CombinatorExpr(CombinatorExpr<'tree>),
    TypeIdentifier(TypeIdentifier<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> CellRefInnerValue<'tree> {
    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            CellRefInnerValue::CombinatorExpr(node) => node.0,
            CellRefInnerValue::TypeIdentifier(node) => node.0,
            CellRefInnerValue::Unmapped(node) => node.0,
        }
    }
}

impl<'tree> TryFromNode<'tree> for CellRefInnerValue<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        match node.kind_bytes() {
            b"combinator_expr" | b"type_identifier" => Ok(Self::from(node)),
            _ => Err(InvalidNodeKindError {
                expected: "combinator_expr|type_identifier",
                actual: node.kind().to_string(),
            }),
        }
    }
}

impl<'tree> AstNode<'tree> for CellRefInnerValue<'tree> {
    fn syntax(&self) -> Node<'tree> {
        self.syntax()
    }
}

impl<'t> From<Node<'t>> for CellRefInnerValue<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"combinator_expr" => CellRefInnerValue::CombinatorExpr(CombinatorExpr(node)),
            b"type_identifier" => CellRefInnerValue::TypeIdentifier(TypeIdentifier(node)),
            _ => CellRefInnerValue::Unmapped(RawNode::new(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ParensCellRef<'tree>(pub Node<'tree>);
impl_ast_node!(ParensCellRef, "parens_cell_ref");

impl<'tree> ParensCellRef<'tree> {
    #[must_use]
    pub fn expr(&self) -> Option<CellRefInner<'tree>> {
        self.0.named_child(0).map(CellRefInner)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum BuiltinExpr<'tree> {
    BuiltinOneArg(BuiltinOneArg<'tree>),
    BuiltinZeroArgs(BuiltinZeroArgs<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> BuiltinExpr<'tree> {
    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            BuiltinExpr::BuiltinOneArg(node) => node.0,
            BuiltinExpr::BuiltinZeroArgs(node) => node.0,
            BuiltinExpr::Unmapped(node) => node.0,
        }
    }
}

impl<'tree> TryFromNode<'tree> for BuiltinExpr<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        match node.kind_bytes() {
            b"builtin_expr" | b"builtin_one_arg" | b"builtin_zero_args" => Ok(Self::from(node)),
            _ => Err(InvalidNodeKindError {
                expected: "builtin_expr|builtin_one_arg|builtin_zero_args",
                actual: node.kind().to_string(),
            }),
        }
    }
}

impl<'tree> AstNode<'tree> for BuiltinExpr<'tree> {
    fn syntax(&self) -> Node<'tree> {
        self.syntax()
    }
}

impl<'t> From<Node<'t>> for BuiltinExpr<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"builtin_expr" => node.named_child(0).map_or_else(
                || BuiltinExpr::Unmapped(RawNode::new(node)),
                BuiltinExpr::from,
            ),
            b"builtin_one_arg" => BuiltinExpr::BuiltinOneArg(BuiltinOneArg(node)),
            b"builtin_zero_args" => BuiltinExpr::BuiltinZeroArgs(BuiltinZeroArgs(node)),
            _ => BuiltinExpr::Unmapped(RawNode::new(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BuiltinOneArg<'tree>(pub Node<'tree>);
impl_ast_node!(BuiltinOneArg, "builtin_one_arg");

impl<'tree> BuiltinOneArg<'tree> {
    #[must_use]
    pub fn expr(&self) -> Option<RefExpr<'tree>> {
        self.0.field("expr")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BuiltinZeroArgs<'tree>(pub Node<'tree>);
impl_ast_node!(BuiltinZeroArgs, "builtin_zero_args");

#[derive(Clone, Copy, Debug)]
pub struct CombinatorExpr<'tree>(pub Node<'tree>);
impl_ast_node!(CombinatorExpr, "combinator_expr");

impl<'tree> CombinatorExpr<'tree> {
    #[must_use]
    pub fn name(&self) -> Option<TypeIdentifier<'tree>> {
        self.0.field("name")
    }

    #[must_use]
    pub fn params(&self) -> AstChildren<'tree, TypeExpr<'tree>> {
        AstChildren::new(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ParensTypeExpr<'tree>(pub Node<'tree>);
impl_ast_node!(ParensTypeExpr, "parens_type_expr");

impl<'tree> ParensTypeExpr<'tree> {
    #[must_use]
    pub fn expr(&self) -> Option<TypeExpr<'tree>> {
        self.0.named_child(0).map(TypeExpr::from)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ArrayType<'tree>(pub Node<'tree>);
impl_ast_node!(ArrayType, "array_type");

impl<'tree> ArrayType<'tree> {
    #[must_use]
    pub fn element_type(&self) -> Option<ArrayElementType<'tree>> {
        self.0.field("element_type")
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ArrayElementType<'tree> {
    TypeIdentifier(TypeIdentifier<'tree>),
    TypeExpr(TypeExpr<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> ArrayElementType<'tree> {
    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            ArrayElementType::TypeIdentifier(node) => node.0,
            ArrayElementType::TypeExpr(node) => node.syntax(),
            ArrayElementType::Unmapped(node) => node.0,
        }
    }
}

impl<'tree> TryFromNode<'tree> for ArrayElementType<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        match node.kind_bytes() {
            b"type_identifier" | b"type_expr" | b"cell_ref_expr" | b"builtin_expr"
            | b"builtin_one_arg" | b"builtin_zero_args" | b"combinator_expr" | b"simple_expr"
            | b"array_type" | b"array_multiplier" | b"bit_size_expr" | b"parens_type_expr" => {
                Ok(Self::from(node))
            }
            _ => Err(InvalidNodeKindError {
                expected: "type_identifier|type_expr-compatible node",
                actual: node.kind().to_string(),
            }),
        }
    }
}

impl<'tree> AstNode<'tree> for ArrayElementType<'tree> {
    fn syntax(&self) -> Node<'tree> {
        self.syntax()
    }
}

impl<'t> From<Node<'t>> for ArrayElementType<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"type_identifier" => ArrayElementType::TypeIdentifier(TypeIdentifier(node)),
            _ => ArrayElementType::TypeExpr(TypeExpr::from(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ArrayMultiplier<'tree>(pub Node<'tree>);
impl_ast_node!(ArrayMultiplier, "array_multiplier");

impl<'tree> ArrayMultiplier<'tree> {
    #[must_use]
    pub fn size(&self) -> Option<SimpleExpr<'tree>> {
        self.0.field("size")
    }

    #[must_use]
    pub fn ty(&self) -> Option<ArrayType<'tree>> {
        self.0.field("type")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BitSizeExpr<'tree>(pub Node<'tree>);
impl_ast_node!(BitSizeExpr, "bit_size_expr");

impl<'tree> BitSizeExpr<'tree> {
    #[must_use]
    pub fn size(&self) -> Option<BitSizeValue<'tree>> {
        self.0.field("size")
    }
}

#[derive(Clone, Copy, Debug)]
pub enum BitSizeValue<'tree> {
    Number(NumberLit<'tree>),
    ParensExpr(ParensExpr<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> BitSizeValue<'tree> {
    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            BitSizeValue::Number(node) => node.0,
            BitSizeValue::ParensExpr(node) => node.0,
            BitSizeValue::Unmapped(node) => node.0,
        }
    }
}

impl<'tree> TryFromNode<'tree> for BitSizeValue<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        match node.kind_bytes() {
            b"number" | b"parens_expr" => Ok(Self::from(node)),
            _ => Err(InvalidNodeKindError {
                expected: "number|parens_expr",
                actual: node.kind().to_string(),
            }),
        }
    }
}

impl<'tree> AstNode<'tree> for BitSizeValue<'tree> {
    fn syntax(&self) -> Node<'tree> {
        self.syntax()
    }
}

impl<'t> From<Node<'t>> for BitSizeValue<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"number" => BitSizeValue::Number(NumberLit(node)),
            b"parens_expr" => BitSizeValue::ParensExpr(ParensExpr(node)),
            _ => BitSizeValue::Unmapped(RawNode::new(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum CompareExpr<'tree> {
    BinaryExpression(BinaryExpression<'tree>),
    ParensCompareExpr(ParensCompareExpr<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> CompareExpr<'tree> {
    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            CompareExpr::BinaryExpression(node) => node.0,
            CompareExpr::ParensCompareExpr(node) => node.0,
            CompareExpr::Unmapped(node) => node.0,
        }
    }
}

impl<'tree> TryFromNode<'tree> for CompareExpr<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        match node.kind_bytes() {
            b"compare_expr" | b"binary_expression" | b"parens_compare_expr" => Ok(Self::from(node)),
            _ => Err(InvalidNodeKindError {
                expected: "compare_expr|binary_expression|parens_compare_expr",
                actual: node.kind().to_string(),
            }),
        }
    }
}

impl<'tree> AstNode<'tree> for CompareExpr<'tree> {
    fn syntax(&self) -> Node<'tree> {
        self.syntax()
    }
}

impl<'t> From<Node<'t>> for CompareExpr<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"compare_expr" => node.named_child(0).map_or_else(
                || CompareExpr::Unmapped(RawNode::new(node)),
                CompareExpr::from,
            ),
            b"binary_expression" => CompareExpr::BinaryExpression(BinaryExpression(node)),
            b"parens_compare_expr" => CompareExpr::ParensCompareExpr(ParensCompareExpr(node)),
            _ => CompareExpr::Unmapped(RawNode::new(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ParensCompareExpr<'tree>(pub Node<'tree>);
impl_ast_node!(ParensCompareExpr, "parens_compare_expr");

impl<'tree> ParensCompareExpr<'tree> {
    #[must_use]
    pub fn expr(&self) -> Option<CompareExpr<'tree>> {
        self.0.named_child(0).map(CompareExpr::from)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum CurlyExpression<'tree> {
    CompareExpr(CompareExpr<'tree>),
    Identifier(Identifier<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> CurlyExpression<'tree> {
    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            CurlyExpression::CompareExpr(node) => node.syntax(),
            CurlyExpression::Identifier(node) => node.0,
            CurlyExpression::Unmapped(node) => node.0,
        }
    }
}

impl<'tree> TryFromNode<'tree> for CurlyExpression<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        match node.kind_bytes() {
            b"curly_expression"
            | b"compare_expr"
            | b"binary_expression"
            | b"parens_compare_expr"
            | b"identifier" => Ok(Self::from(node)),
            _ => Err(InvalidNodeKindError {
                expected: "curly_expression|compare_expr|identifier",
                actual: node.kind().to_string(),
            }),
        }
    }
}

impl<'tree> AstNode<'tree> for CurlyExpression<'tree> {
    fn syntax(&self) -> Node<'tree> {
        self.syntax()
    }
}

impl<'t> From<Node<'t>> for CurlyExpression<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"curly_expression" => node.named_child(0).map_or_else(
                || CurlyExpression::Unmapped(RawNode::new(node)),
                CurlyExpression::from,
            ),
            b"compare_expr" | b"binary_expression" | b"parens_compare_expr" => {
                CurlyExpression::CompareExpr(CompareExpr::from(node))
            }
            b"identifier" => CurlyExpression::Identifier(Identifier(node)),
            _ => CurlyExpression::Unmapped(RawNode::new(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BuiltinField<'tree>(pub Node<'tree>);
impl_ast_node!(BuiltinField, "builtin_field");

#[derive(Clone, Copy, Debug)]
pub struct Identifier<'tree>(pub Node<'tree>);
impl_ast_node!(Identifier, "identifier");

#[derive(Clone, Copy, Debug)]
pub struct TypeIdentifier<'tree>(pub Node<'tree>);
impl_ast_node!(TypeIdentifier, "type_identifier");

#[derive(Clone, Copy, Debug)]
pub struct NumberLit<'tree>(pub Node<'tree>);
impl_ast_node!(NumberLit, "number");

#[derive(Clone, Copy, Debug)]
pub struct BinaryNumberLit<'tree>(pub Node<'tree>);
impl_ast_node!(BinaryNumberLit, "binary_number");

#[derive(Clone, Copy, Debug)]
pub struct HexLit<'tree>(pub Node<'tree>);
impl_ast_node!(HexLit, "hex");

#[derive(Clone, Copy, Debug)]
pub struct Comment<'tree>(pub Node<'tree>);
impl_ast_node!(Comment, "comment");
