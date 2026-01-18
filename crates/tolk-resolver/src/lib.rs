//! Symbol resolution and indexing for Tolk source files.
//!
//! This crate provides tools for building a project-wide index of symbols,
//! resolving imports, and performing local name resolution within files.
//! It is used by the linter and other tools to understand the structure
//! and connectivity of Tolk code.

pub mod file_db;
pub mod file_index;
pub mod project_index;
pub mod resolve_index;
pub mod symbol_resolver;

#[cfg(test)]
mod resolve_tests;

pub use file_db::{FileDb, FileInfo};
pub use file_index::{
    AstNodeSpanExt, FileId, FileIndex, Import, Span, Symbol, SymbolId, SymbolKind,
};
pub use project_index::{ProjectIndex, ProjectIndexBuilder, ResolvedImport};
pub use resolve_index::{FileResolveIndex, NameUse, NameUseKind, Resolved};
pub use symbol_resolver::{SymbolResolver, resolve, resolve_file};
