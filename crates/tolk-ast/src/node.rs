use crate::top_level::{Import, TopLevel};
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use tree_sitter::{Node, Tree};

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub tree: Tree,
    pub source: Arc<str>,
}

impl PartialOrd for SourceFile {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SourceFile {
    fn cmp(&self, other: &Self) -> Ordering {
        self.source.cmp(&other.source)
    }
}

impl Eq for SourceFile {}

impl PartialEq for SourceFile {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
    }
}

impl Hash for SourceFile {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.source.hash(state);
    }
}

impl SourceFile {
    pub fn new(tree: Tree, source: String) -> SourceFile {
        SourceFile {
            tree,
            source: Arc::from(source),
        }
    }

    pub fn top_levels(&self) -> Vec<TopLevel<'_>> {
        let root = self.tree.root_node();
        let mut walker = root.walk();
        root.children(&mut walker)
            .map(Into::into)
            .collect::<Vec<_>>()
    }

    pub fn top_levels_iter(&self) -> impl Iterator<Item = TopLevel<'_>> {
        let root = self.tree.root_node();
        (0..root.child_count())
            .filter_map(move |i| root.child(i))
            .map(Into::into)
    }

    pub fn imports(&self) -> Vec<Import<'_>> {
        let root = self.tree.root_node();
        let mut walker = root.walk();
        let children = root.children(&mut walker);
        children
            .filter(|c| c.kind() == "import_directive")
            .map(Into::into)
            .collect::<Vec<_>>()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RawNode<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for RawNode<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> RawNode<'tree> {
    pub fn new(node: Node<'tree>) -> Self {
        Self(node)
    }

    pub fn text(&self, source: &'tree str) -> &'tree str {
        self.0
            .utf8_text(source.as_bytes())
            .unwrap_or("<invalid utf8>")
    }
}

pub trait NodeFieldExt<'t> {
    fn field<T>(&self, name: &str) -> Option<T>
    where
        T: From<Node<'t>>;

    fn field_by_id<T>(&self, id: u16) -> Option<T>
    where
        T: From<Node<'t>>;
}

impl<'t> NodeFieldExt<'t> for Node<'t> {
    fn field<T>(&self, name: &str) -> Option<T>
    where
        T: From<Node<'t>>,
    {
        self.child_by_field_name(name).map(Into::into)
    }

    fn field_by_id<T>(&self, id: u16) -> Option<T>
    where
        T: From<Node<'t>>,
    {
        self.child_by_field_id(id).map(Into::into)
    }
}
