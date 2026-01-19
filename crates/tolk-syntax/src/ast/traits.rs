use crate::ast::expressions::Ident;
use crate::ast::node::AstChildren;
use crate::ast::top_level::{AnnotationList, TypeParameters};
use crate::ast::{FuncBody, Parameter, Type};
use std::ffi::CStr;
use std::fmt;
use tree_sitter::ffi::ts_node_type;

/// Trait for AST nodes that have a name identifier.
pub trait HasName<'tree> {
    fn name(&self) -> Option<Ident<'tree>>;
}

/// Trait for AST nodes that can have generic type parameters.
pub trait HasGenericParams<'tree> {
    fn type_parameters(&self) -> Option<TypeParameters<'tree>>;
}

/// Trait for AST nodes that can have annotations (e.g., `@pure`).
pub trait HasAnnotations<'tree> {
    fn annotations(&self) -> Option<AnnotationList<'tree>>;
}

/// Trait for AST nodes that represent function-like constructs (functions, methods, get methods).
pub trait FunctionLike<'tree>: HasName<'tree> + AstNode<'tree> {
    /// Returns the return type of the function, if explicitly declared.
    fn return_type(&self) -> Option<Type<'tree>>;
    /// Returns the body of the function.
    fn body(&self) -> Option<FuncBody<'tree>>;
    /// Returns the parameters of the function.
    fn parameters(&self) -> AstChildren<'tree, Parameter<'tree>>;
}

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

/// The base trait for all AST nodes in this crate.
pub trait AstNode<'tree>: TryFromNode<'tree> + Clone {
    /// Returns the underlying tree-sitter node.
    fn syntax(&self) -> tree_sitter::Node<'tree>;

    /// Returns the source text associated with this AST node.
    ///
    /// # Parameters
    ///
    /// * `source` — The full source code string from which this node was parsed.
    ///
    /// # Returns
    ///
    /// A string slice containing the text of this node. If the node's byte range
    /// is not valid UTF-8 within the source, returns `"<invalid utf8>"`.
    fn text<'a>(&self, source: &'a str) -> &'a str {
        self.syntax()
            .utf8_text(source.as_bytes())
            .unwrap_or("<invalid utf8>")
    }

    /// Checks if the text of this AST node matches the expected string.
    ///
    /// This method is more efficient than calling `self.text(source) == expected`
    /// because it:
    /// 1. Performs a fast-path length check before comparing contents.
    /// 2. Avoids UTF-8 validation and string allocation by comparing raw bytes.
    ///
    /// # Parameters
    ///
    /// * `source` — The full source code string from which this node was parsed.
    /// * `expected` — The string to compare against.
    ///
    /// # Returns
    ///
    /// `true` if the node's text matches `expected`, `false` otherwise.
    fn text_matches(&self, source: &str, expected: &str) -> bool {
        let syntax = self.syntax();
        let start = syntax.start_byte();
        let end = syntax.end_byte();
        let width = end - start;
        if width != expected.len() {
            // fast path, width of node is not equal to width of expected string
            return false;
        }

        if end > source.len() || start > end {
            return false;
        }

        // don't create an actual string for substring and just compare bytes
        &source.as_bytes()[start..end] == expected.as_bytes()
    }

    /// Returns the first child node with the given field name, converted to the specified AST type.
    ///
    /// This is a convenience wrapper around tree-sitter's `child_by_field_name` that
    /// automatically performs the conversion from a raw `Node` to an AST type.
    ///
    /// # Parameters
    ///
    /// * `name` — The field name as defined in the tree-sitter grammar.
    ///
    /// # Returns
    ///
    /// `Some(T)` if a child with the given name exists, `None` otherwise.
    fn field<T>(&self, name: &str) -> Option<T>
    where
        T: From<tree_sitter::Node<'tree>>,
    {
        self.syntax().child_by_field_name(name).map(Into::into)
    }

    /// Returns the length of the text of this AST node.
    ///
    /// # Returns
    ///
    /// The length of the text of this AST node.
    fn text_length(&self) -> usize {
        let syntax = self.syntax();
        syntax.end_byte() - syntax.start_byte()
    }
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

        impl<'tree> HasTreeSitterKind for $name<'tree> {
            const TREE_SITTER_KIND: &'static str = $kind;
        }
    };
}

// Implement AstNode for raw tree-sitter Node to provide span() method
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
