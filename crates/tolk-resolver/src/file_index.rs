//! Basic types and structures for indexing a single Tolk source file.
//!
//! This module defines the core metadata extracted from a source file,
//! such as declarations, imports, and source spans.

use crate::resolve_index::LocalDefId;
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::hash::Hash;
use std::path::PathBuf;
use std::sync::Arc;
use tolk_syntax::{AstNode, FunctionLike, HasGenericParams, HasName, ast};
use tree_sitter::Node;

/// Represents a byte range in the source code.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Span {
    /// Start byte offset (inclusive).
    pub start: u32,
    /// End byte offset (exclusive).
    pub end: u32,
}

impl Display for Span {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.start, self.end)
    }
}

impl Span {
    /// A dummy span used for missing or synthesized nodes.
    const DUMMY: Span = Span {
        start: u32::MAX,
        end: u32::MAX,
    };

    /// Creates a span from a tree-sitter node.
    pub fn from_syntax(node: &Node) -> Self {
        Span {
            start: node.start_byte() as u32,
            end: node.end_byte() as u32,
        }
    }

    /// Creates a span from an AST node.
    pub fn from_node<'a, Node: AstNode<'a>>(node: &Node) -> Self {
        let syntax = node.syntax();
        Self::from_syntax(&syntax)
    }

    /// Creates a span from an optional tree-sitter node, returning `DUMMY` if `None`.
    pub fn from_opt_syntax(node: &Option<Node>) -> Self {
        let Some(node) = node else {
            return Self::DUMMY;
        };
        Self::from_syntax(node)
    }

    /// Creates a span from local definition ID and its length.
    ///
    /// `LocalDefId.local` represents byte offset, so we need a length
    /// to build a span.
    pub fn from_def_id(id: LocalDefId, length: u32) -> Self {
        Self {
            start: id.local,
            end: id.local + length,
        }
    }

    /// Checks if the given byte offset is within this span.
    pub fn contains(&self, offset: usize) -> bool {
        self.start() <= offset && offset <= self.end()
    }

    /// Returns the start offset as a `usize`.
    pub fn start(&self) -> usize {
        self.start as usize
    }

    /// Returns the end offset as a `usize`.
    pub fn end(&self) -> usize {
        self.end as usize
    }

    /// Returns length of this span.
    pub fn len(&self) -> usize {
        self.end as usize - self.start as usize
    }

    /// Returns true if span length equals to zero.
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

/// Extension trait to easily get a `Span` from an AST node.
pub trait AstNodeSpanExt<'tree> {
    /// Returns the byte span of this AST node.
    ///
    /// The span represents the range of bytes in the source code that this
    /// AST node occupies, from `start` (inclusive) to `end` (exclusive).
    fn span(&self) -> Span;
}

impl<'tree, T> AstNodeSpanExt<'tree> for T
where
    T: AstNode<'tree>,
{
    fn span(&self) -> Span {
        Span::from_node(self)
    }
}

/// Extension trait to easily get a `Span` from an optional tree-sitter node.
pub trait OptionalSyntaxNodeSpanExt<'tree> {
    /// Returns the byte span of this optional tree-sitter node.
    /// Returns a dummy span if `None`.
    fn span(&self) -> Span;
}

impl<'tree> OptionalSyntaxNodeSpanExt<'tree> for Option<Node<'tree>> {
    fn span(&self) -> Span {
        Span::from_opt_syntax(self)
    }
}

/// Unique identifier for a top-level symbol in the project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolId {
    /// The ID of the file where the symbol is defined.
    pub file_id: FileId,
    /// Local identifier within the file.
    pub local_id: u32,
}

/// Information about a top-level declaration (function, struct, etc.).
#[derive(Debug, Clone)]
pub struct Symbol {
    /// Unique identifier for this symbol.
    pub id: SymbolId,
    /// Normalized name of the symbol.
    pub name: Arc<str>,
    /// Fully qualified name (e.g. `MyType.method`).
    pub fqn: Arc<str>,
    /// Specific kind of the declaration.
    pub kind: SymbolKind,
    /// Span of the name identifier (useful for "go to definition").
    pub name_span: Span,
    /// Span of the entire declaration body (useful for "document symbols").
    pub body_span: Span,
    /// Span of the associated documentation comment (if any).
    pub doc_span: Option<Span>,
}

#[derive(Debug, Clone)]
pub struct Parameter {
    pub is_mutate: bool,
}

/// Distinguishes between different kinds of top-level declarations.
#[derive(Debug, Clone)]
pub enum SymbolKind {
    /// A global variable.
    GlobalVariable,
    /// A top-level function.
    Function {
        /// Whether the function has an explicit return type.
        has_return_type: bool,
        parameters: Vec<Parameter>,
    },
    /// A method defined on a type.
    Method {
        /// Whether the method has an explicit return type.
        has_return_type: bool,
        /// Whether the method can modify `self`.
        is_mutable: bool,
        /// Whether the method is an instance method.
        is_instance: bool,
        parameters: Vec<Parameter>,
    },
    /// A GET-method (for TON smart contracts).
    GetMethod {
        /// Whether the method has an explicit return type.
        has_return_type: bool,
        parameters: Vec<Parameter>,
    },
    /// A struct definition.
    Struct {
        /// Fields of the struct.
        fields: Vec<Symbol>,
        is_generic: bool,
    },
    /// A field within a struct.
    StructField,
    /// An enum definition.
    Enum {
        /// Members of the enum.
        members: Vec<Symbol>,
    },
    /// A member within an enum.
    EnumMember,
    /// A global constant.
    Constant,
    /// A type alias definition.
    TypeAlias {
        /// When `type slice = builtin`
        is_builtin: bool,
    },
}

impl Symbol {
    /// Returns `true` if this declaration defines a type (struct, enum, or alias).
    pub fn is_type(&self) -> bool {
        matches!(
            self.kind,
            SymbolKind::TypeAlias { .. } | SymbolKind::Struct { .. } | SymbolKind::Enum { .. }
        )
    }
}

/// Represents an import statement.
#[derive(Debug, Clone)]
pub struct Import {
    /// Path string as it appears in the source code.
    pub path: Arc<str>,
    /// Span of the entire import declaration.
    pub span: Span,
}

/// Unique identifier for a file in the project.
pub type FileId = u32;

/// A processed index of a single Tolk source file.
#[derive(Debug, Clone)]
pub struct FileIndex {
    /// Unique identifier for this file.
    pub id: FileId,
    /// Absolute path to the file.
    pub path: PathBuf,
    /// List of files imported by this file.
    pub imports: Vec<Import>,
    /// List of top-level declarations in this file.
    pub decls: Vec<Symbol>,
    /// Mapping from local_id of the [`SymbolId`] to index in tree root children.
    pub symbol_id_to_decl_index: BTreeMap<u32, usize>, // SymbolId.local_id to idx in top levels
}

impl FileIndex {
    /// Builds a `FileIndex` from a parsed `SourceFile`.
    ///
    /// # Panics
    ///
    /// Panics in debug builds if the path is not absolute.
    pub fn build(file_id: FileId, path: PathBuf, file: &ast::SourceFile) -> FileIndex {
        debug_assert!(path.is_absolute()); // for stable ID

        let mut decls = vec![];
        let mut imports = vec![];

        let mut local_id: u32 = 0;

        let mut symbol_id_to_decl_index = BTreeMap::new();

        for (idx, decl) in file.top_levels().enumerate() {
            if matches!(
                decl,
                tolk_syntax::TopLevel::TolkRequiredVersion(_)
                    | tolk_syntax::TopLevel::EmptyStmt(_)
                    | tolk_syntax::TopLevel::Unmapped(_)
            ) {
                continue;
            }

            symbol_id_to_decl_index.insert(local_id, idx);

            let name: Arc<str> = Arc::from(decl.name_text(&file.source));
            let name_span = decl.name().map(|n| n.syntax()).span();
            let fqn = name.clone();
            let id = SymbolId { file_id, local_id };
            let body_span = decl.span();
            let doc_span = None; // TODO: future work
            match decl {
                tolk_syntax::TopLevel::GlobalVar(_) => decls.push(Symbol {
                    id,
                    name,
                    fqn,
                    kind: SymbolKind::GlobalVariable,
                    name_span,
                    body_span,
                    doc_span,
                }),
                tolk_syntax::TopLevel::Constant(_) => decls.push(Symbol {
                    id,
                    name,
                    fqn,
                    kind: SymbolKind::Constant,
                    name_span,
                    body_span,
                    doc_span,
                }),
                tolk_syntax::TopLevel::TypeAlias(decl) => decls.push(Symbol {
                    id,
                    name,
                    fqn,
                    kind: SymbolKind::TypeAlias {
                        is_builtin: matches!(
                            decl.underlying_type(),
                            Some(ast::TypeAliasUnderlyingType::BuiltinSpecifier(_))
                        ),
                    },
                    name_span,
                    body_span,
                    doc_span,
                }),
                tolk_syntax::TopLevel::Struct(decl) => {
                    let struct_name = name.clone();
                    let fields = decl.body().map(|b| b.fields()).unwrap_or_default();
                    let fields = fields
                        .filter_map(|f| {
                            let name_ident = f.name()?;
                            let name = Arc::from(name_ident.text(&file.source));
                            let name_span = name_ident.span();
                            let fqn = Arc::from(format!("{}.{}", struct_name, name));
                            local_id += 1;
                            let id = SymbolId { file_id, local_id };
                            let doc_span = None;
                            Some(Symbol {
                                id,
                                name,
                                fqn,
                                kind: SymbolKind::StructField,
                                name_span,
                                body_span,
                                doc_span,
                            })
                        })
                        .collect();
                    decls.push(Symbol {
                        id,
                        name,
                        fqn,
                        kind: SymbolKind::Struct {
                            fields,
                            is_generic: decl.type_parameters().is_some(),
                        },
                        name_span,
                        body_span,
                        doc_span,
                    })
                }
                tolk_syntax::TopLevel::Enum(decl) => {
                    let enum_name = name.clone();
                    let members = decl.body().map(|b| b.members()).unwrap_or_default();
                    let members = members
                        .filter_map(|f| {
                            let name_ident = f.name()?;
                            let name = Arc::from(name_ident.text(&file.source));
                            let name_span = name_ident.span();
                            let fqn = Arc::from(format!("{}.{}", enum_name, name));
                            local_id += 1;
                            let id = SymbolId { file_id, local_id };
                            let doc_span = None;
                            Some(Symbol {
                                id,
                                name,
                                fqn,
                                kind: SymbolKind::EnumMember,
                                name_span,
                                body_span,
                                doc_span,
                            })
                        })
                        .collect();
                    decls.push(Symbol {
                        id,
                        name,
                        fqn,
                        kind: SymbolKind::Enum { members },
                        name_span,
                        body_span,
                        doc_span,
                    })
                }
                tolk_syntax::TopLevel::Func(func) => {
                    let has_return_type = func.return_type().is_some();
                    decls.push(Symbol {
                        id,
                        name,
                        fqn,
                        kind: SymbolKind::Function {
                            has_return_type,
                            parameters: func
                                .parameters()
                                .map(|p| Parameter {
                                    is_mutate: p.mutate(),
                                })
                                .collect(),
                        },
                        name_span,
                        body_span,
                        doc_span,
                    })
                }
                tolk_syntax::TopLevel::Method(func) => {
                    let sources = file.source.as_ref();
                    let mut parameters = func.parameters_ext(sources, false);
                    let first_param = parameters.next();
                    let is_mutable = first_param.is_some_and(|param| {
                        let mutate = param.mutate();
                        if mutate
                            && let Some(name) = param.name()
                            && name.0.utf8_text(sources.as_ref()) == Ok("self")
                        {
                            return true;
                        }

                        false
                    });
                    let is_instance = func.is_instance(sources);
                    let has_return_type = func.return_type().is_some();

                    let fqn = if let Some(receiver) = func.receiver_type() {
                        Arc::from(format!("{}.{}", receiver.text(sources), name))
                    } else {
                        fqn
                    };

                    decls.push(Symbol {
                        id,
                        name,
                        fqn,
                        kind: SymbolKind::Method {
                            has_return_type,
                            is_mutable,
                            is_instance,
                            parameters: func
                                .parameters()
                                .map(|p| Parameter {
                                    is_mutate: p.mutate(),
                                })
                                .collect(),
                        },
                        name_span,
                        body_span,
                        doc_span,
                    })
                }
                tolk_syntax::TopLevel::GetMethod(func) => {
                    let has_return_type = func.return_type().is_some();
                    decls.push(Symbol {
                        id,
                        name,
                        fqn,
                        kind: SymbolKind::GetMethod {
                            has_return_type,
                            parameters: func
                                .parameters()
                                .map(|p| Parameter {
                                    is_mutate: p.mutate(),
                                })
                                .collect(),
                        },
                        name_span,
                        body_span,
                        doc_span,
                    })
                }
                tolk_syntax::TopLevel::Import(import) => {
                    let Some(path) = import.path() else {
                        continue;
                    };
                    let path = path.content(&file.source);
                    imports.push(Import {
                        path: Arc::from(path),
                        span: import.span(),
                    })
                }
                tolk_syntax::TopLevel::TolkRequiredVersion(_) => continue,
                tolk_syntax::TopLevel::EmptyStmt(_) => continue,
                tolk_syntax::TopLevel::Unmapped(_) => continue,
            };

            local_id += 1;
        }

        FileIndex {
            id: file_id,
            path,
            imports,
            decls,
            symbol_id_to_decl_index,
        }
    }
}
