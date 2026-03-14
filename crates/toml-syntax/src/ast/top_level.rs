use crate::ast::expressions::{Key, Pair};
use crate::ast::node::{AstChildren, RawNode};
use crate::ast::{AstNode, InvalidNodeKindError, TryFromNode};
use crate::{AstNodeBytesKind, impl_ast_node};
use tree_sitter::Node;

#[derive(Clone, Copy, Debug)]
pub enum TopLevel<'tree> {
    Pair(Pair<'tree>),
    Table(Table<'tree>),
    TableArrayElement(TableArrayElement<'tree>),
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
            TopLevel::Pair(n) => n.0,
            TopLevel::Table(n) => n.0,
            TopLevel::TableArrayElement(n) => n.0,
            TopLevel::Unmapped(n) => n.0,
        }
    }
}

impl<'tree> TryFromNode<'tree> for TopLevel<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        match node.kind_bytes() {
            b"pair" | b"table" | b"table_array_element" => Ok(Self::from(node)),
            _ => Err(InvalidNodeKindError {
                expected: "pair|table|table_array_element",
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
            b"pair" => TopLevel::Pair(Pair(node)),
            b"table" => TopLevel::Table(Table(node)),
            b"table_array_element" => TopLevel::TableArrayElement(TableArrayElement(node)),
            _ => TopLevel::Unmapped(RawNode::new(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Document<'tree>(pub Node<'tree>);
impl_ast_node!(Document, "document");

impl<'tree> Document<'tree> {
    pub fn items(&self) -> AstChildren<'tree, TopLevel<'tree>> {
        AstChildren::new(self.0)
    }

    pub fn pairs(&self) -> AstChildren<'tree, Pair<'tree>> {
        AstChildren::new(self.0)
    }

    pub fn tables(&self) -> AstChildren<'tree, Table<'tree>> {
        AstChildren::new(self.0)
    }

    pub fn table_arrays(&self) -> AstChildren<'tree, TableArrayElement<'tree>> {
        AstChildren::new(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Table<'tree>(pub Node<'tree>);
impl_ast_node!(Table, "table");

impl<'tree> Table<'tree> {
    #[must_use]
    pub fn key(&self) -> Option<Key<'tree>> {
        let mut cursor = self.0.walk();
        self.0
            .named_children(&mut cursor)
            .find_map(|child| Key::try_from_node(child).ok())
    }

    pub fn pairs(&self) -> AstChildren<'tree, Pair<'tree>> {
        AstChildren::new(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TableArrayElement<'tree>(pub Node<'tree>);
impl_ast_node!(TableArrayElement, "table_array_element");

impl<'tree> TableArrayElement<'tree> {
    #[must_use]
    pub fn key(&self) -> Option<Key<'tree>> {
        let mut cursor = self.0.walk();
        self.0
            .named_children(&mut cursor)
            .find_map(|child| Key::try_from_node(child).ok())
    }

    pub fn pairs(&self) -> AstChildren<'tree, Pair<'tree>> {
        AstChildren::new(self.0)
    }
}
