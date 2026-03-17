use crate::ast::node::{AstChildren, RawNode};
use crate::ast::traits::{AstNode, InvalidNodeKindError, TryFromNode};
use crate::{AstNodeBytesKind, impl_ast_node};
use tree_sitter::Node;

#[derive(Clone, Copy, Debug)]
pub enum Key<'tree> {
    Bare(BareKey<'tree>),
    Quoted(QuotedKey<'tree>),
    Dotted(DottedKey<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> Key<'tree> {
    #[must_use]
    pub fn text(&self, source: &'tree str) -> &'tree str {
        self.syntax().utf8_text(source.as_bytes()).unwrap_or("")
    }

    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            Key::Bare(n) => n.0,
            Key::Quoted(n) => n.0,
            Key::Dotted(n) => n.0,
            Key::Unmapped(n) => n.0,
        }
    }
}

impl<'tree> TryFromNode<'tree> for Key<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        match node.kind_bytes() {
            b"bare_key" | b"quoted_key" | b"dotted_key" => Ok(Self::from(node)),
            _ => Err(InvalidNodeKindError {
                expected: "bare_key|quoted_key|dotted_key",
                actual: node.kind().to_string(),
            }),
        }
    }
}

impl<'tree> AstNode<'tree> for Key<'tree> {
    fn syntax(&self) -> Node<'tree> {
        self.syntax()
    }
}

impl<'t> From<Node<'t>> for Key<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"bare_key" => Key::Bare(BareKey(node)),
            b"quoted_key" => Key::Quoted(QuotedKey(node)),
            b"dotted_key" => Key::Dotted(DottedKey(node)),
            _ => Key::Unmapped(RawNode::new(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Value<'tree> {
    String(StringLit<'tree>),
    Integer(IntegerLit<'tree>),
    Float(FloatLit<'tree>),
    Boolean(BooleanLit<'tree>),
    OffsetDateTime(OffsetDateTime<'tree>),
    LocalDateTime(LocalDateTime<'tree>),
    LocalDate(LocalDate<'tree>),
    LocalTime(LocalTime<'tree>),
    Array(Array<'tree>),
    InlineTable(InlineTable<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> Value<'tree> {
    #[must_use]
    pub fn text(&self, source: &'tree str) -> &'tree str {
        self.syntax().utf8_text(source.as_bytes()).unwrap_or("")
    }

    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            Value::String(n) => n.0,
            Value::Integer(n) => n.0,
            Value::Float(n) => n.0,
            Value::Boolean(n) => n.0,
            Value::OffsetDateTime(n) => n.0,
            Value::LocalDateTime(n) => n.0,
            Value::LocalDate(n) => n.0,
            Value::LocalTime(n) => n.0,
            Value::Array(n) => n.0,
            Value::InlineTable(n) => n.0,
            Value::Unmapped(n) => n.0,
        }
    }
}

impl<'tree> TryFromNode<'tree> for Value<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        match node.kind_bytes() {
            b"string" | b"integer" | b"float" | b"boolean" | b"offset_date_time"
            | b"local_date_time" | b"local_date" | b"local_time" | b"array" | b"inline_table" => {
                Ok(Self::from(node))
            }
            _ => Err(InvalidNodeKindError {
                expected: "string|integer|float|boolean|offset_date_time|local_date_time|local_date|local_time|array|inline_table",
                actual: node.kind().to_string(),
            }),
        }
    }
}

impl<'tree> AstNode<'tree> for Value<'tree> {
    fn syntax(&self) -> Node<'tree> {
        self.syntax()
    }
}

impl<'t> From<Node<'t>> for Value<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"string" => Value::String(StringLit(node)),
            b"integer" => Value::Integer(IntegerLit(node)),
            b"float" => Value::Float(FloatLit(node)),
            b"boolean" => Value::Boolean(BooleanLit(node)),
            b"offset_date_time" => Value::OffsetDateTime(OffsetDateTime(node)),
            b"local_date_time" => Value::LocalDateTime(LocalDateTime(node)),
            b"local_date" => Value::LocalDate(LocalDate(node)),
            b"local_time" => Value::LocalTime(LocalTime(node)),
            b"array" => Value::Array(Array(node)),
            b"inline_table" => Value::InlineTable(InlineTable(node)),
            _ => Value::Unmapped(RawNode::new(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Pair<'tree>(pub Node<'tree>);
impl_ast_node!(Pair, "pair");

impl<'tree> Pair<'tree> {
    #[must_use]
    pub fn key(&self) -> Option<Key<'tree>> {
        let mut cursor = self.0.walk();
        self.0
            .named_children(&mut cursor)
            .find_map(|child| Key::try_from_node(child).ok())
    }

    #[must_use]
    pub fn value(&self) -> Option<Value<'tree>> {
        let mut cursor = self.0.walk();
        self.0
            .named_children(&mut cursor)
            .find_map(|child| Value::try_from_node(child).ok())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DottedKey<'tree>(pub Node<'tree>);
impl_ast_node!(DottedKey, "dotted_key");

impl<'tree> DottedKey<'tree> {
    pub fn parts(&self) -> AstChildren<'tree, Key<'tree>> {
        AstChildren::new(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BareKey<'tree>(pub Node<'tree>);
impl_ast_node!(BareKey, "bare_key");

#[derive(Clone, Copy, Debug)]
pub struct QuotedKey<'tree>(pub Node<'tree>);
impl_ast_node!(QuotedKey, "quoted_key");

#[derive(Clone, Copy, Debug)]
pub struct StringLit<'tree>(pub Node<'tree>);
impl_ast_node!(StringLit, "string");

#[derive(Clone, Copy, Debug)]
pub struct IntegerLit<'tree>(pub Node<'tree>);
impl_ast_node!(IntegerLit, "integer");

#[derive(Clone, Copy, Debug)]
pub struct FloatLit<'tree>(pub Node<'tree>);
impl_ast_node!(FloatLit, "float");

#[derive(Clone, Copy, Debug)]
pub struct BooleanLit<'tree>(pub Node<'tree>);
impl_ast_node!(BooleanLit, "boolean");

#[derive(Clone, Copy, Debug)]
pub struct OffsetDateTime<'tree>(pub Node<'tree>);
impl_ast_node!(OffsetDateTime, "offset_date_time");

#[derive(Clone, Copy, Debug)]
pub struct LocalDateTime<'tree>(pub Node<'tree>);
impl_ast_node!(LocalDateTime, "local_date_time");

#[derive(Clone, Copy, Debug)]
pub struct LocalDate<'tree>(pub Node<'tree>);
impl_ast_node!(LocalDate, "local_date");

#[derive(Clone, Copy, Debug)]
pub struct LocalTime<'tree>(pub Node<'tree>);
impl_ast_node!(LocalTime, "local_time");

#[derive(Clone, Copy, Debug)]
pub struct Array<'tree>(pub Node<'tree>);
impl_ast_node!(Array, "array");

impl<'tree> Array<'tree> {
    pub fn values(&self) -> AstChildren<'tree, Value<'tree>> {
        AstChildren::new(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InlineTable<'tree>(pub Node<'tree>);
impl_ast_node!(InlineTable, "inline_table");

impl<'tree> InlineTable<'tree> {
    pub fn pairs(&self) -> AstChildren<'tree, Pair<'tree>> {
        AstChildren::new(self.0)
    }
}
