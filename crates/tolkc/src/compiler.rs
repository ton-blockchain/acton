#![allow(unsafe_code)]
use dunce::canonicalize;
use include_dir::{Dir, include_dir};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::ffi::{CStr, CString, c_char, c_int};
use std::fs::read_to_string;
use std::path::{Path, PathBuf};
use ton_source_map::{HighLevelSourceMap, SourceMap, parse_marks_dict};

thread_local! {
    static CURRENT_MAPPINGS: RefCell<FxHashMap<String, String>> = RefCell::new(FxHashMap::default());
}

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
#[must_use]
pub fn compile(path: &Path, debug: bool) -> CompilerResult {
    Compiler::new(2).compile(path, debug)
}

#[must_use]
pub fn compile_fast(path: &Path, debug: bool) -> CompilerResult {
    Compiler::new(0).compile(path, debug)
}

#[repr(u32)]
enum FsReadCallbackKind {
    Realpath = 0,
    ReadFile = 1,
}

impl From<c_int> for FsReadCallbackKind {
    fn from(value: c_int) -> Self {
        if value == 0 {
            return FsReadCallbackKind::Realpath;
        }

        FsReadCallbackKind::ReadFile
    }
}

/// Simple wrapper over C++ implemented Tolk compiler.
pub struct Compiler {
    /// Level of optimizations, 0 – no optimizations, 2 – all optimizations.
    pub opt_level: i64,
    /// Show comments with stack for instructions in Fift code.
    pub with_stack_comments: bool,
    /// Show comments with Tolk source file references in Fift code.
    pub with_src_line_comments: bool,
    /// Other experimental options.
    pub experimental_options: String,
    /// Mappings for paths (e.g. "@core" -> "/path/to/core")
    pub mappings: FxHashMap<String, String>,
}

impl Compiler {
    #[must_use]
    pub fn new(opt_level: i64) -> Self {
        Self {
            opt_level,
            with_stack_comments: false,
            with_src_line_comments: false,
            experimental_options: String::new(),
            mappings: FxHashMap::default(),
        }
    }

    /// Sets mapping that will be used to resolve imports paths.
    ///
    /// For example:
    ///
    /// - `@root`: `foo/bar/`
    ///
    /// `import "@root/baz"` will be resolved to `foo/bar/baz`
    pub fn with_mappings(mut self, mappings: &Option<BTreeMap<String, String>>) -> Self {
        if let Some(mappings) = mappings {
            self.mappings = mappings
                .iter()
                .map(|(key, value)| {
                    if key.starts_with('@') {
                        (key.clone(), value.clone())
                    } else {
                        (format!("@{key}"), value.clone())
                    }
                })
                .collect();
        }
        self
    }

    /// Compiles passed file with Tolk compiler.
    ///
    /// Returns successful result with `code_boc64` or error with `message`.
    pub fn compile(&self, path: &Path, with_debug_info: bool) -> CompilerResult {
        CURRENT_MAPPINGS.with(|m| {
            *m.borrow_mut() = self.mappings.clone();
        });

        let config = serde_json::to_string(&CompilerConfig {
            entrypoint_file_name: path.to_string_lossy().to_string(),
            optimization_level: self.opt_level,
            with_stack_comments: self.with_stack_comments,
            with_src_line_comments: self.with_src_line_comments,
            experimental_options: self.experimental_options.clone(),
            collect_source_map: with_debug_info,
        })
        .expect("Critical error, cannot serializer path to JSON, should not happen");

        // SAFETY: we're calling safe C function
        let compilation_result = unsafe {
            unsafe extern "C" fn read_callback(
                kind: std::os::raw::c_int,
                data_ptr: *const c_char,
                dest_contents: *mut *mut c_char,
                dest_error: *mut *mut c_char,
            ) {
                fn realpath(path_str: &str) -> Result<PathBuf, String> {
                    if Path::new(path_str).is_absolute() {
                        return canonicalize(path_str).map_err(|e| e.to_string());
                    }

                    if path_str.starts_with("@stdlib/") || path_str.starts_with("@fiftlib/") {
                        return Ok(PathBuf::from(path_str));
                    }

                    if path_str.starts_with('@') {
                        let (prefix, suffix) = match path_str.find('/') {
                            Some(pos) => (&path_str[..pos], &path_str[pos + 1..]),
                            None => (path_str, ""),
                        };

                        let mut resolved = None;
                        CURRENT_MAPPINGS.with(|mappings| {
                            let mappings = mappings.borrow();
                            if let Some(target) = mappings.get(prefix) {
                                let cur_mapped_path = Path::new(target).join(suffix);

                                resolved =
                                    Some(canonicalize(cur_mapped_path).map_err(|e| e.to_string()));
                            }
                        });

                        if let Some(res) = resolved {
                            return res;
                        }

                        return Err(format!("Unknown path mapping '{prefix}'"));
                    }

                    canonicalize(path_str).map_err(|e| e.to_string())
                }

                match FsReadCallbackKind::from(kind) {
                    FsReadCallbackKind::Realpath => {
                        // SAFETY: `data_ptr` is valid not-null pointer
                        let relative_path_raw = unsafe {
                            CStr::from_ptr(data_ptr)
                                .to_str()
                                .expect("Invalid UTF-8 in relative path")
                        };

                        let result = if relative_path_raw.ends_with(".tolk") {
                            realpath(relative_path_raw)
                        } else {
                            let mut path = String::with_capacity(relative_path_raw.len() + 5);
                            path.push_str(relative_path_raw);
                            path.push_str(".tolk");
                            realpath(&path)
                        };

                        let abs_path = match result {
                            Ok(abs_path) => abs_path,
                            Err(err) => {
                                let raw_str = CString::new(err).expect("Failed to create C string");
                                // SAFETY: `dest_error` is valid not-null pointer
                                unsafe {
                                    *dest_error = raw_str.into_raw();
                                }
                                return;
                            }
                        };

                        let raw_str = CString::new(
                            abs_path.to_str().expect("Invalid UTF-8 in absolute path"),
                        )
                        .expect("Failed to create C string from absolute path");
                        // SAFETY: `dest_contents` is valid not-null pointer
                        unsafe { *dest_contents = raw_str.into_raw() }
                    }
                    FsReadCallbackKind::ReadFile => {
                        // SAFETY: `data_ptr` is valid not-null pointer
                        let file_path = unsafe {
                            CStr::from_ptr(data_ptr)
                                .to_str()
                                .expect("Invalid UTF-8 in file path")
                        };

                        let content = if let Some(filename) = file_path.strip_prefix("@stdlib/") {
                            if let Some(content) =
                                read_stdlib_file(filename).map(ToString::to_string)
                            {
                                content
                            } else {
                                let raw_str = CString::new(
                                    "Cannot read standard library file, file not found",
                                )
                                .expect("Failed to create C string");
                                // SAFETY: `dest_error` is valid not-null pointer
                                unsafe { *dest_error = raw_str.into_raw() };
                                return;
                            }
                        } else if let Some(filename) = file_path.strip_prefix("@fiftlib/") {
                            if let Some(content) =
                                read_fift_stdlib_file(filename).map(ToString::to_string)
                            {
                                content
                            } else {
                                let raw_str = CString::new(
                                    "Cannot read Fift standard library file, file not found",
                                )
                                .expect("Failed to create C string");
                                // SAFETY: `dest_error` is valid not-null pointer
                                unsafe { *dest_error = raw_str.into_raw() };
                                return;
                            }
                        } else {
                            match read_to_string(file_path) {
                                Ok(content) => content,
                                Err(error) => {
                                    let raw_str = CString::new(error.to_string())
                                        .expect("Failed to create C string from error");
                                    // SAFETY: `dest_error` is valid not-null pointer
                                    unsafe { *dest_error = raw_str.into_raw() };
                                    return;
                                }
                            }
                        };

                        let raw_str =
                            CString::new(content).expect("Failed to create C string from content");
                        // SAFETY: `dest_contents` is valid not-null pointer
                        unsafe { *dest_contents = raw_str.into_raw() }
                    }
                }
            }

            let config_cstr =
                CString::new(config).expect("Cannot convert JSON to CString, should not happen");
            tolk_compile(config_cstr.as_ptr(), Some(read_callback))
        };

        // SAFETY: we assume that `compilation_result` is valid C string
        let compilation_result_str = unsafe {
            CString::from_raw(compilation_result.cast_mut())
                .to_string_lossy()
                .to_string()
        };

        let result = serde_json::from_str::<CompilerInternalResult>(&compilation_result_str);

        match result {
            Ok(CompilerInternalResult::Success(result)) => {
                let debug_marks = if with_debug_info {
                    parse_marks_dict(&result.debug_mark_base64, &result.code_boc64)
                        .unwrap_or_default()
                } else {
                    HashMap::new()
                };
                CompilerResult::Success(CompilerResultSuccess {
                    fift_code: result.fift_code,
                    code_boc64: result.code_boc64,
                    code_hash_hex: result.code_hash_hex,
                    source_map: result.source_map.map(|source_map| SourceMap {
                        high_level: source_map,
                        debug_marks,
                    }),
                })
            }
            Ok(CompilerInternalResult::Error(result)) => CompilerResult::Error(result),
            Err(err) => CompilerResult::Error(CompilerResultError {
                message: err.to_string(),
            }),
        }
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
    #[serde(rename = "collectSourceMap")]
    pub collect_source_map: bool,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum CompilerResult {
    Success(CompilerResultSuccess),
    Error(CompilerResultError),
}

#[derive(Debug)]
pub struct CompilerResultSuccess {
    pub fift_code: String,
    pub code_boc64: String,
    pub code_hash_hex: String,
    pub source_map: Option<SourceMap>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum CompilerInternalResult {
    Success(CompilerInternalResultSuccess),
    Error(CompilerResultError),
}

#[derive(Debug, Deserialize)]
pub struct CompilerInternalResultSuccess {
    #[serde(rename = "fiftCode")]
    pub fift_code: String,
    #[serde(rename = "codeBoc64")]
    pub code_boc64: String,
    #[serde(rename = "codeHashHex")]
    pub code_hash_hex: String,
    #[serde(rename = "debugMarkBase64")]
    pub debug_mark_base64: String,
    #[serde(rename = "sourceMap")]
    pub source_map: Option<HighLevelSourceMap>,
}

#[derive(Debug, Deserialize)]
pub struct CompilerResultError {
    pub message: String,
}

/// We embed the whole standard library of Tolk and Fift in binary for easier distribution.
pub static TOLK_STDLIB_DIR: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/assets/tolk-stdlib");
static FIFT_STDLIB_DIR: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/assets/fift");

fn read_stdlib_file(path: &str) -> Option<&'static str> {
    TOLK_STDLIB_DIR.get_file(path)?.contents_utf8()
}

fn read_fift_stdlib_file(path: &str) -> Option<&'static str> {
    FIFT_STDLIB_DIR.get_file(path)?.contents_utf8()
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
