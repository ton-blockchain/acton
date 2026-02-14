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
use tolk_syntax::{AstNode, FunctionLike, HasAnnotations, HasGenericParams, HasName, ast};
use tree_sitter::Node;

/// Represents a byte range in the source code.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Ord, PartialOrd)]
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

    pub const fn from_offset(offset: usize) -> Self {
        Span {
            start: offset as u32,
            end: offset as u32 + 1,
        }
    }

    pub const fn file_start() -> Self {
        Span { start: 0, end: 0 }
    }

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
    pub const fn from_def_id(id: LocalDefId, length: u32) -> Self {
        Self {
            start: id.local,
            end: id.local + length,
        }
    }

    /// Checks if the given byte offset is within this span.
    pub const fn contains(&self, offset: usize) -> bool {
        self.start() <= offset && offset <= self.end()
    }

    /// Returns the start offset as a `usize`.
    pub const fn start(&self) -> usize {
        self.start as usize
    }

    /// Returns the end offset as a `usize`.
    pub const fn end(&self) -> usize {
        self.end as usize
    }

    /// Returns length of this span.
    pub const fn len(&self) -> usize {
        self.end as usize - self.start as usize
    }

    /// Returns true if span length equals to zero.
    pub const fn is_empty(&self) -> bool {
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
    /// If this symbol is deprecated.
    pub is_deprecated: bool,
    /// If this symbol is marked as `@pure`.
    pub is_pure: bool,
}

#[derive(Debug, Clone)]
pub struct Parameter {
    pub is_mutate: bool,
}

#[derive(Debug, Clone)]
pub struct TypeParameter {
    pub name: Arc<str>,
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
        type_parameters: Vec<TypeParameter>,
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
        type_parameters: Vec<TypeParameter>,
    },
    /// A GET-method (for TON smart contracts).
    GetMethod {
        /// Whether the method has an explicit return type.
        has_return_type: bool,
        parameters: Vec<Parameter>,
        type_parameters: Vec<TypeParameter>,
    },
    /// A struct definition.
    Struct {
        /// Fields of the struct.
        fields: Vec<Symbol>,
        is_generic: bool,
        type_parameters: Vec<TypeParameter>,
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
        type_parameters: Vec<TypeParameter>,
    },
}

impl Symbol {
    /// Returns `true` if this declaration defines a type (struct, enum, or alias).
    pub const fn is_type(&self) -> bool {
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

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum FileSource {
    Stdlib,
    Acton,
    Workspace,
}

/// A processed index of a single Tolk source file.
#[derive(Debug, Clone)]
pub struct FileIndex {
    /// Unique identifier for this file.
    pub id: FileId,
    /// Absolute path to the file.
    pub path: PathBuf,
    /// Where file is located.
    pub source_kind: FileSource,
    /// List of files imported by this file.
    pub imports: Vec<Import>,
    /// List of top-level declarations in this file.
    pub decls: Vec<Symbol>,
    /// Mapping from local_id of the [`SymbolId`] to index in tree root children.
    pub symbol_id_to_decl_index: BTreeMap<u32, usize>, // SymbolId.local_id to idx in top levels
    /// Sorted list of spans for top-level declarations, used for efficient lookup.
    pub body_spans: Vec<(Span, usize)>,
}

impl FileIndex {
    pub fn find_symbol_index_at_offset(&self, offset: usize) -> Option<usize> {
        let idx = self
            .body_spans
            .binary_search_by(|(span, _)| {
                if span.contains(offset) {
                    std::cmp::Ordering::Equal
                } else if (span.start as usize) > offset {
                    std::cmp::Ordering::Greater
                } else {
                    std::cmp::Ordering::Less
                }
            })
            .ok()?;
        Some(self.body_spans[idx].1)
    }

    /// Builds a `FileIndex` from a parsed `SourceFile`.
    ///
    /// # Panics
    ///
    /// Panics in debug builds if the path is not absolute.
    pub fn build(
        content: &str,
        file_id: FileId,
        path: PathBuf,
        file: &ast::SourceFile,
        source_kind: FileSource,
    ) -> FileIndex {
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
            let is_deprecated = Self::is_deprecated(content, decl);
            let is_pure = Self::is_pure(content, decl);

            match decl {
                tolk_syntax::TopLevel::GlobalVar(_) => decls.push(Symbol {
                    id,
                    name,
                    fqn,
                    kind: SymbolKind::GlobalVariable,
                    name_span,
                    body_span,
                    doc_span,
                    is_deprecated,
                    is_pure,
                }),
                tolk_syntax::TopLevel::Constant(_) => decls.push(Symbol {
                    id,
                    name,
                    fqn,
                    kind: SymbolKind::Constant,
                    name_span,
                    body_span,
                    doc_span,
                    is_deprecated,
                    is_pure,
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
                        type_parameters: Self::extract_type_parameters(file, decl),
                    },
                    name_span,
                    body_span,
                    doc_span,
                    is_deprecated,
                    is_pure,
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
                                is_deprecated: false,
                                is_pure: false,
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
                            type_parameters: Self::extract_type_parameters(file, decl),
                        },
                        name_span,
                        body_span,
                        doc_span,
                        is_deprecated,
                        is_pure,
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
                                is_deprecated: false,
                                is_pure: false,
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
                        is_deprecated,
                        is_pure,
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
                            type_parameters: Self::extract_type_parameters(file, decl),
                        },
                        name_span,
                        body_span,
                        doc_span,
                        is_deprecated,
                        is_pure,
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
                            type_parameters: Self::extract_type_parameters(file, decl),
                        },
                        name_span,
                        body_span,
                        doc_span,
                        is_deprecated,
                        is_pure,
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
                            type_parameters: Self::extract_type_parameters(file, decl),
                        },
                        name_span,
                        body_span,
                        doc_span,
                        is_deprecated,
                        is_pure,
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

        let mut body_spans: Vec<(Span, usize)> = decls
            .iter()
            .enumerate()
            .map(|(i, d)| (d.body_span, i))
            .collect();
        body_spans.sort_by_key(|(s, _)| s.start);

        FileIndex {
            id: file_id,
            path,
            source_kind,
            imports,
            decls,
            symbol_id_to_decl_index,
            body_spans,
        }
    }

    fn extract_type_parameters<'a, Node: HasGenericParams<'a>>(
        file: &ast::SourceFile,
        decl: Node,
    ) -> Vec<TypeParameter> {
        decl.type_parameters()
            .map(|tp| {
                tp.parameters()
                    .flat_map(|p| {
                        let name_ident = p.name()?;
                        Some(TypeParameter {
                            name: Arc::from(name_ident.text(&file.source)),
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
            .into_iter()
            .collect()
    }

    fn is_deprecated<'a, Node: HasAnnotations<'a>>(content: &str, node: Node) -> bool {
        node.annotations().iter().any(|a| {
            a.annotations().any(|a| {
                a.name()
                    .is_some_and(|name| name.text_matches(content, "deprecated"))
            })
        })
    }

    fn is_pure<'a, Node: HasAnnotations<'a>>(content: &str, node: Node) -> bool {
        node.annotations().iter().any(|a| {
            a.annotations().any(|a| {
                a.name()
                    .is_some_and(|name| name.text_matches(content, "pure"))
            })
        })
    }
}
