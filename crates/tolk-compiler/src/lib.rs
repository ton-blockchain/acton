//! This crates provides a Tolk language compiler.
//!
//! ## Example
//!
//! ```no_run
//! use std::path::Path;
//!
//! let tmp_test_filename = "file.tolk";
//! let compilation_result = tolk_compiler::compile(Path::new(&tmp_test_filename), false);
//! match compilation_result {
//!     tolk_compiler::CompilerResult::Success(result) => {
//!         // ... use result.code_boc64
//!     }
//!     tolk_compiler::CompilerResult::Error(error) => {
//!         eprintln!("Cannot compile test file {}", error.message); // :(
//!     }
//! }
//! ```

use ton_objs as _;

pub mod compiler;
mod version;

pub use compiler::{Compiler, CompilerInternalResult, CompilerResult, compile, prime_debug_cp0};
pub use tolk_source_map::SourceMap;
pub use tolk_source_map::{abi, debug_marks_dict, dynamic_unpack, source_map, types_kernel};
pub use version::{NativeTolkVersion, native_tolk_version};
