//! Stores resolution information for names used within a file.
//!
//! This module defines the mapping from name usages to their corresponding
//! definitions, whether they are global symbols or local variables.

use crate::file_index::{FileId, Span, SymbolId};
use std::sync::Arc;

/// Represents a usage of a name in the source code.
#[derive(Debug, Clone)]
pub struct NameUse {
    /// The start byte of the declaration where this name is used.
    pub decl: u32,
    /// The span of the name usage itself.
    pub span: Span,
    /// Whether the name is expected to be a value, a type, or if it's ambiguous.
    pub kind: NameUseKind,
    /// The normalized name that was used.
    pub name: Arc<str>,
    /// What the name was resolved to.
    pub resolved: Resolved,
}

/// Categorizes the expected nature of a name usage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameUseKind {
    /// Usage in a value context (e.g., variable access, function call).
    Value,
    /// Usage in a value context but only for local variables and parameters.
    LocalValue,
    /// Usage in a type context (e.g., type annotation, struct field type).
    Type,
    /// Ambiguous usage where it could be either (e.g., in match patterns).
    Mixed,
}

/// The result of resolving a name.
#[derive(Debug, Clone)]
pub enum Resolved {
    /// Resolved to a top-level symbol (global variable, function, etc.).
    Global(SymbolId),
    /// Resolved to a local definition (parameter, local variable).
    Local(LocalDefId),
    /// Could not be resolved.
    Unresolved,
}

/// Unique identifier for a local definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalDefId {
    /// The ID of the file where the local is defined.
    pub file_id: FileId,
    /// Byte offset of the definition in the file.
    pub local: u32,
}

impl LocalDefId {
    /// Creates a new `LocalDefId`.
    #[must_use]
    pub const fn new(file_id: FileId, local: u32) -> Self {
        Self { file_id, local }
    }
}

/// Information about a local definition (parameter or variable).
#[derive(Debug, Clone)]
pub struct LocalDef {
    /// Unique identifier for this local definition.
    pub id: LocalDefId,
    /// Normalized name of the local.
    pub name: Arc<str>,
    /// Span of the definition identifier.
    pub def_span: Span,
    /// Specific kind of the local definition.
    pub kind: LocalDefKind,
}

/// Distinguishes between different kinds of local definitions.
#[derive(Debug, Clone, Copy)]
pub enum LocalDefKind {
    /// A function or method parameter.
    Param {
        has_type: bool,
        /// Whether the parameter is declared as mutable.
        is_mutable: bool,
        is_self: bool,
        in_asm_or_builtin: bool,
    },
    /// A local variable.
    Var {
        has_type: bool,
        /// Whether the variable is declared as mutable.
        is_mutable: bool,
    },
    /// A variable captured in a catch clause.
    Catch,
    /// A type parameter for a generic function or type.
    TypeParameter,
}

/// Index of all name resolutions and local definitions in a file.
#[derive(Debug)]
pub struct FileResolveIndex {
    /// ID of the file this index belongs to.
    pub file_id: FileId,
    /// List of all name usages in the file, sorted by span for fast searching.
    pub uses: Vec<NameUse>,
    /// List of all local definitions in the file.
    pub locals: Vec<LocalDef>,
}

impl FileResolveIndex {
    /// Finds the name usage at the given byte offset using binary search.
    #[must_use]
    pub fn find_use(&self, pos: usize) -> Option<&NameUse> {
        let pos = pos as u32;
        self.uses
            .binary_search_by(|u| {
                if pos < u.span.start {
                    std::cmp::Ordering::Greater
                } else if pos >= u.span.end {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .ok()
            .map(|idx| &self.uses[idx])
    }

    /// Returns an iterator over all usages of the given local definition.
    pub fn local_usages_of(&self, local_id: LocalDefId) -> impl Iterator<Item = &NameUse> {
        self.uses
            .iter()
            .filter(move |u| matches!(u.resolved, Resolved::Local(id) if id == local_id))
    }

    /// Returns an iterator over all usages of the given global symbol.
    pub fn global_usages_of(&self, symbol_id: SymbolId) -> impl Iterator<Item = &NameUse> {
        self.uses
            .iter()
            .filter(move |u| matches!(u.resolved, Resolved::Global(id) if id == symbol_id))
    }

    #[must_use]
    pub fn find_local(&self, id: LocalDefId) -> Option<&LocalDef> {
        self.locals.iter().find(|local| local.id == id)
    }

    #[must_use]
    pub fn find_local_at(&self, offset: usize) -> Option<&LocalDef> {
        self.locals
            .iter()
            .find(|local| local.def_span.contains(offset))
    }
}
