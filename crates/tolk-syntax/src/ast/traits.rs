use crate::ast::expressions::Ident;
use crate::ast::node::AstChildren;
use crate::ast::top_level::{AnnotationList, TypeParameters};
use crate::ast::{FuncBody, Parameter, Type};

pub use ton_syntax::ast::{
    AstNode, AstNodeBytesKind, HasName, HasTreeSitterKind, InvalidNodeKindError, TryFromNode,
};

/// Trait for AST nodes that can have generic type parameters.
pub trait HasGenericParams<'tree> {
    fn type_parameters(&self) -> Option<TypeParameters<'tree>>;
}

/// Trait for AST nodes that can have annotations (e.g., `@pure`).
pub trait HasAnnotations<'tree> {
    fn annotations(&self) -> Option<AnnotationList<'tree>>;
}

/// Trait for AST nodes that represent function-like constructs (functions, methods, get methods).
pub trait FunctionLike<'tree>: HasName<'tree, Name = Ident<'tree>> + AstNode<'tree> {
    /// Returns the return type of the function, if explicitly declared.
    fn return_type(&self) -> Option<Type<'tree>>;
    /// Returns the body of the function.
    fn body(&self) -> Option<FuncBody<'tree>>;
    /// Returns the parameters of the function.
    fn parameters(&self) -> AstChildren<'tree, Parameter<'tree>>;
}
