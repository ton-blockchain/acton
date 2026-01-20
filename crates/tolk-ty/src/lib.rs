//! # Tolk Type Inference Library
//!
//! This crate provides a comprehensive type inference system for the Tolk programming language.
//! It analyzes Tolk source code to infer types for expressions, functions, and global declarations,
//! while performing control flow analysis to handle smart casts and conditional type narrowing.
//!
//! ## Core Components
//!
//! - [`TypeDb`]: Central type database managing global type definitions and symbol resolution
//! - [`TypeInterner`]: Efficient type interning system for memory-optimized type storage
//! - [`TyId`]: Opaque identifier for interned types
//! - [`infer`]: Main entry point for running type inference on top-level declarations
//! - [`InferenceResult`]: Contains inferred types and resolved references for a declaration
//!
//! ## Example Usage
//!
//! ```rust,no_run
//! use tolk_resolver::file_db::FileDb;
//! use tolk_resolver::project_index::ProjectIndex;
//! use tolk_ty::{TypeDb, TypeInterner, infer};
//! use std::path::PathBuf;
//!
//! fn analyze_tolk_contract() -> Result<(), Box<dyn std::error::Error>> {
//!     // Initialize the file database and process source files
//!     let stdlib_path = PathBuf::from("path/to/tolk-stdlib");
//!     let file_db = FileDb::new(stdlib_path.clone(), None);
//!
//!     // Add your Tolk source files to the database
//!     let root_path = PathBuf::from("contracts/my_contract.tolk");
//!     let file_info = file_db.process(&root_path)?;
//!
//!     // Build the project index with optional stdlib
//!     let mut index = ProjectIndex::builder(&file_db, root_path)
//!         .with_stdlib(stdlib_path)
//!         .build()?;
//!
//!     // Resolve symbols across the project
//!     tolk_resolver::resolve(&file_db, &mut index);
//!
//!     // Create the type interner and database
//!     let mut interner = TypeInterner::new();
//!     let mut type_db = TypeDb::new(&mut interner, &file_db, &index);
//!
//!     // Run type inference on each top-level declaration
//!     for decl in file_info.source().top_levels() {
//!         if let Some(index_decl) = file_info.find_declaration(&decl) {
//!             // Infer types for this declaration
//!             let result = infer(&mut type_db, file_info.id(), index_decl.id, &decl);
//!
//!             // Access inferred types and control flow information
//!             println!("Inferred {} types for declaration",
//!                      result.expression_types.len());
//!
//!             // The result contains:
//!             // - expression_types: Map of expression spans to their inferred types
//!             // - flow_context: Control flow information for smart casts
//!             // - resolved_refs: Symbol resolution information
//!         }
//!     }
//!
//!     // Query specific types from the type database
//!     // Note: This is just an example — you'd get symbol_id from your actual code
//!     let symbol_id = None; // Placeholder
//!     if let Some(symbol_id) = symbol_id {
//!         if let Some(ty) = type_db.get_top_level_type(None, symbol_id) {
//!             println!("Symbol type: {}", interner.display(ty));
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```

pub(crate) mod expression_inference;
pub(crate) mod flow_inference;
pub(crate) mod generics_helpers;
pub(crate) mod overload_resolution;
pub(crate) mod statement_inference;
pub(crate) mod type_db;
pub(crate) mod type_formatter;
pub(crate) mod type_inference;
pub(crate) mod type_interner;
pub(crate) mod type_substitutor;
pub(crate) mod type_unify;
pub(crate) mod types;

pub use flow_inference::InferenceResult;
pub use type_db::TypeDb;
pub use type_inference::infer;
pub use type_interner::{TyId, TypeInterner};
pub use types::{AddressKind, IntTy, TyData};
