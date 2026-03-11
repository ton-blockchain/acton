use memchr::memchr;
use std::ffi::CStr;
use std::fmt;
use tree_sitter::ffi::ts_node_type;

/// Trait for AST nodes that correspond to a specific tree-sitter node kind.
pub trait HasTreeSitterKind {
    /// The tree-sitter kind name for this AST node.
    const TREE_SITTER_KIND: &'static str;
}

/// Error returned when a node kind does not match the expected kind.
#[derive(Debug, Clone)]
pub struct InvalidNodeKindError {
    pub expected: &'static str,
    pub actual: String,
}

impl fmt::Display for InvalidNodeKindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Expected node kind {}, but got {}",
            self.expected, self.actual
        )
    }
}

impl std::error::Error for InvalidNodeKindError {}

pub trait TryFromNode<'tree>: Sized {
    type Error;

    /// Attempts to convert a tree-sitter node into this AST type.
    fn try_from_node(node: tree_sitter::Node<'tree>) -> Result<Self, Self::Error>;
}

/// The base trait for all AST nodes.
pub trait AstNode<'tree>: TryFromNode<'tree> + Clone {
    /// Returns the underlying tree-sitter node.
    fn syntax(&self) -> tree_sitter::Node<'tree>;

    /// Returns the source text associated with this AST node.
    fn text<'a>(&self, source: &'a str) -> &'a str {
        self.syntax()
            .utf8_text(source.as_bytes())
            .unwrap_or("<invalid utf8>")
    }

    /// Checks if the text of this AST node matches the expected string.
    fn text_matches(&self, source: &str, expected: &str) -> bool {
        let syntax = self.syntax();
        let start = syntax.start_byte();
        let end = syntax.end_byte();
        let width = end - start;
        if width != expected.len() {
            return false;
        }

        if end > source.len() || start > end {
            return false;
        }

        &source.as_bytes()[start..end] == expected.as_bytes()
    }

    /// Returns the first child node with the given field name, converted to the specified AST type.
    fn field<T>(&self, name: &str) -> Option<T>
    where
        T: From<tree_sitter::Node<'tree>>,
    {
        self.syntax().child_by_field_name(name).map(Into::into)
    }

    /// Returns the length of the text of this AST node.
    fn text_length(&self) -> usize {
        let syntax = self.syntax();
        syntax.end_byte() - syntax.start_byte()
    }

    /// Returns position of first occurrence of `ch` or None.
    fn text_contains(&self, source: &str, ch: u8) -> Option<usize> {
        let syntax = self.syntax();
        let start = syntax.start_byte();
        let end = syntax.end_byte();
        let slice = &source.as_bytes()[start..end];
        memchr(ch, slice)
    }
}

/// Trait for AST nodes that have a name identifier.
pub trait HasName<'tree> {
    type Name: AstNode<'tree>;

    fn name(&self) -> Option<Self::Name>;
}

/// A macro to implement [`TryFromNode`] and [`AstNode`] for a given type.
///
/// Takes the type name and the expected tree-sitter node kind as arguments.
#[macro_export]
macro_rules! impl_ast_node {
    ($name:ident, $kind:literal) => {
        impl<'tree> $crate::ast::traits::TryFromNode<'tree> for $name<'tree> {
            type Error = $crate::ast::traits::InvalidNodeKindError;

            fn try_from_node(node: tree_sitter::Node<'tree>) -> Result<Self, Self::Error> {
                use $crate::ast::traits::AstNodeBytesKind;
                let expected: &str =
                    <Self as $crate::ast::traits::HasTreeSitterKind>::TREE_SITTER_KIND;
                if node.kind_bytes() == expected.as_bytes() {
                    Ok(Self::from(node))
                } else {
                    Err($crate::ast::traits::InvalidNodeKindError {
                        expected,
                        actual: node.kind().to_string(),
                    })
                }
            }
        }

        impl<'tree> From<tree_sitter::Node<'tree>> for $name<'tree> {
            fn from(n: tree_sitter::Node<'tree>) -> Self {
                Self(n)
            }
        }

        impl<'tree> $crate::ast::traits::AstNode<'tree> for $name<'tree> {
            fn syntax(&self) -> tree_sitter::Node<'tree> {
                self.0
            }
        }

        impl<'tree> $crate::ast::traits::HasTreeSitterKind for $name<'tree> {
            const TREE_SITTER_KIND: &'static str = $kind;
        }
    };
}

// Implement AstNode for raw tree-sitter Node to provide convenience methods.
impl<'tree> TryFromNode<'tree> for tree_sitter::Node<'tree> {
    type Error = std::convert::Infallible;

    fn try_from_node(node: tree_sitter::Node<'tree>) -> Result<Self, Self::Error> {
        Ok(node)
    }
}

impl<'tree> AstNode<'tree> for tree_sitter::Node<'tree> {
    fn syntax(&self) -> tree_sitter::Node<'tree> {
        *self
    }
}

pub trait AstNodeBytesKind {
    fn kind_bytes(&self) -> &[u8];
}

impl<'tree> AstNodeBytesKind for tree_sitter::Node<'tree> {
    fn kind_bytes(&self) -> &[u8] {
        // SAFETY: we know that `ts_node_type` returns a valid C-string.
        #[allow(unsafe_code)]
        let t = unsafe { CStr::from_ptr(ts_node_type(self.into_raw())) };
        t.to_bytes()
    }
}
