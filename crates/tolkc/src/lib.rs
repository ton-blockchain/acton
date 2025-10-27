//! This crates provides a Tolk language compiler.
//!
//! ## Example
//!
//! ```
//! let compilation_result = tolkc::compile(Path::new(&tmp_test_filename));
//! match compilation_result {
//!     tolkc::CompilerInternalResult::Success(result) => {
//!         // ... use result.code_boc64
//!     }
//!     tolkc::CompilerInternalResult::Error(error) => {
//!         eprintln!("Cannot compile test file {}", error.message); // :(
//!     }
//! }
//! ```

pub mod compiler;
pub mod source_map;

pub use compiler::{Compiler, CompilerInternalResult, CompilerResult, compile, compile_fast};
