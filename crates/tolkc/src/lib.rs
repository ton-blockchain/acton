//! This crates provides a Tolk language compiler.
//!
//! ## Example
//!
//! ```no_run
//! use std::path::Path;
//!
//! let tmp_test_filename = "file.tolk";
//! let compilation_result = tolkc::compile(Path::new(&tmp_test_filename), false);
//! match compilation_result {
//!     tolkc::CompilerResult::Success(result) => {
//!         // ... use result.code_boc64
//!     }
//!     tolkc::CompilerResult::Error(error) => {
//!         eprintln!("Cannot compile test file {}", error.message); // :(
//!     }
//! }
//! ```

use ton_objs as _;

pub mod abi;
pub mod compiler;
mod version;

pub use compiler::{Compiler, CompilerInternalResult, CompilerResult, compile, compile_fast};
pub use version::{NativeTolkVersion, native_tolk_version};
