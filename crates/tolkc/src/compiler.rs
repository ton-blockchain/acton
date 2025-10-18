use include_dir::{Dir, include_dir};
use serde::{Deserialize, Serialize};
use std::ffi::{CStr, CString, c_char};
use std::fs::{canonicalize, read_to_string};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Compiles passed file with Tolk compiler.
///
/// Returns successful result with `code_boc64` or error with `message`.
///
/// ## Example
///
/// ```
/// let compilation_result = tolkc::compile(Path::new(&tmp_test_filename));
/// match compilation_result {
///     tolkc::CompilerResult::Success(result) => {
///         // ... use result.code_boc64
///     }
///     tolkc::CompilerResult::Error(error) => {
///         eprintln!("Cannot compile test file {}", error.message); // :(
///     }
/// }
/// ```
pub fn compile(path: &Path) -> CompilerResult {
    Compiler::new(
        2,
        "/Users/petrmakhnev/emulator-rs/crates/tolkc/assets/fift/".to_string(),
    )
    .compile(path)
}

/// Simple wrapper over C++ implemented Tolk compiler.
pub struct Compiler {
    /// Level of optimizations, 0 – no optimizations, 2 – all optimizations.
    pub opt_level: i64,
    /// Path to folder with Fift implementation files.
    pub fift_path: String,
    /// Show comments with stack for instructions in Fift code.
    pub with_stack_comments: bool,
    /// Show comments with Tolk source file references in Fift code.
    pub with_src_line_comments: bool,
    /// Other experimental options.
    pub experimental_options: String,
}

impl Compiler {
    pub fn new(opt_level: i64, fift_path: String) -> Self {
        Self {
            opt_level,
            fift_path,
            with_stack_comments: false,
            with_src_line_comments: false,
            experimental_options: "".to_string(),
        }
    }

    /// Compiles passed file with Tolk compiler.
    ///
    /// Returns successful result with `code_boc64` or error with `message`.
    pub fn compile(&self, path: &Path) -> CompilerResult {
        let config = serde_json::to_string(&CompilerConfig {
            entrypoint_file_name: path.to_string_lossy().to_string(),
            optimization_level: self.opt_level,
            with_stack_comments: self.with_stack_comments,
            with_src_line_comments: self.with_src_line_comments,
            experimental_options: self.experimental_options,
            fift_path: self.fift_path,
        })
        .expect("Critical error, cannot serializer path to JSON, should not happen");

        let compilation_result = unsafe {
            unsafe extern "C" fn read_callback(
                kind: std::os::raw::c_int,
                data_ptr: *const c_char,
                dest_contents: *mut *mut c_char,
                dest_error: *mut *mut c_char,
            ) {
                fn realpath(path: PathBuf) -> Result<String, std::io::Error> {
                    if path.is_absolute() {
                        return Ok(path.into_os_string().into_string().unwrap());
                    }

                    if path.starts_with("@stdlib/") {
                        return Ok(path.to_string_lossy().to_string());
                    }

                    let abs_path = canonicalize(path)?;
                    Ok(abs_path.to_string_lossy().into_owned())
                }

                match kind {
                    0 => {
                        let mut relative_path = "".to_string();
                        let relative_path_raw =
                            unsafe { CStr::from_ptr(data_ptr).to_str().unwrap() };
                        if !relative_path_raw.ends_with(".tolk") {
                            relative_path.push_str(relative_path_raw);
                            relative_path += ".tolk";
                        } else {
                            relative_path.push_str(relative_path_raw);
                        }

                        let Ok(abs_path) = realpath(relative_path.parse().unwrap()) else {
                            let raw_str =
                                CString::new("cannot realpath a path".to_string()).unwrap();
                            unsafe {
                                *dest_error = raw_str.into_raw();
                            }
                            return;
                        };

                        let raw_str = CString::new(abs_path).unwrap();
                        unsafe { *dest_contents = raw_str.into_raw() }
                    }
                    1 => {
                        let file_path = unsafe { CStr::from_ptr(data_ptr).to_str().unwrap() };

                        let content = if file_path.contains("@stdlib/") {
                            let filename = file_path
                                .strip_prefix("@stdlib/")
                                .unwrap_or_else(|| file_path);
                            match read_stdlib_file(filename).map(|s| s.to_string()) {
                                Some(content) => content,
                                None => {
                                    let raw_str = CString::new(
                                        "Cannot read standard library file, file not found",
                                    )
                                    .unwrap();
                                    unsafe {
                                        *dest_error = raw_str.into_raw();
                                    }
                                    return;
                                }
                            }
                        } else {
                            match read_to_string(file_path) {
                                Ok(content) => content,
                                Err(error) => {
                                    let raw_str = CString::new(error.to_string()).unwrap();
                                    unsafe {
                                        *dest_error = raw_str.into_raw();
                                    }
                                    return;
                                }
                            }
                        };

                        let raw_str = CString::new(content).unwrap();
                        unsafe { *dest_contents = raw_str.into_raw() }
                    }
                    _ => {}
                }
            }

            let config_cstr =
                CString::new(config).expect("Cannot convert JSON to CString, should not happen");
            tolk_compile(config_cstr.as_ptr(), Some(read_callback))
        };

        let compilation_result_str = unsafe {
            CString::from_raw(compilation_result.cast_mut())
                .to_string_lossy()
                .to_string()
        };

        let result = serde_json::from_str::<CompilerResult>(&compilation_result_str);
        result.unwrap_or_else(|err| {
            CompilerResult::Error(CompilerResultError {
                message: err.to_string(),
            })
        })
    }
}

#[derive(Serialize)]
pub struct CompilerConfig {
    #[serde(rename = "entrypointFileName")]
    pub entrypoint_file_name: String,
    #[serde(rename = "optimizationLevel")]
    pub optimization_level: i64,
    #[serde(rename = "withStackComments")]
    pub with_stack_comments: bool,
    #[serde(rename = "withSrcLineComments")]
    pub with_src_line_comments: bool,
    #[serde(rename = "experimentalOptions")]
    pub experimental_options: String,
    #[serde(rename = "fiftPath")]
    pub fift_path: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum CompilerResult {
    Success(CompilerResultSuccess),
    Error(CompilerResultError),
}

#[derive(Deserialize)]
pub struct CompilerResultSuccess {
    #[serde(rename = "fiftCode")]
    pub fift_code: String,
    #[serde(rename = "codeBoc64")]
    pub code_boc64: String,
    #[serde(rename = "codeHashHex")]
    pub code_hash_hex: String,
}

#[derive(Deserialize)]
pub struct CompilerResultError {
    pub message: String,
}

/// We embed the whole standard library in binary for easier distribution.
static TOLK_STDLIB_DIR: Dir = include_dir!("./crates/tolkc/assets/tolk-stdlib");

fn read_stdlib_file(path: &str) -> Option<&'static str> {
    TOLK_STDLIB_DIR.get_file(path)?.contents_utf8()
}

// C FFI declarations

unsafe extern "C" {
    pub fn tolk_compile(
        config_json: *const ::std::os::raw::c_char,
        callback: WasmFsReadCallback,
    ) -> *const ::std::os::raw::c_char;
}

type WasmFsReadCallback = Option<
    unsafe extern "C" fn(
        kind: ::std::os::raw::c_int,
        data: *const ::std::os::raw::c_char,
        dest_contents: *mut *mut ::std::os::raw::c_char,
        dest_error: *mut *mut ::std::os::raw::c_char,
    ),
>;
