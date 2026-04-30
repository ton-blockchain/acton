#![allow(unsafe_code)]
use crate::abi::ContractABI;
use dunce;
use include_dir::{Dir, include_dir};
use rustc_hash::FxHashMap;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::fs;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};

/// Compiles passed file with Tolk compiler.
///
/// Returns successful result with `code_boc64` or error with `message`.
///
/// ## Example
///
/// ```no_run
/// use std::path::Path;
///
/// let tmp_test_filename = "file.tolk";
/// let compilation_result = tolk_compiler::compile(Path::new(&tmp_test_filename), false);
/// match compilation_result {
///     tolk_compiler::CompilerResult::Success(result) => {
///         // ... use result.code_boc64
///     }
///     tolk_compiler::CompilerResult::Error(error) => {
///         eprintln!("Cannot compile test file {}", error.message); // :(
///     }
/// }
/// ```
#[must_use]
pub fn compile(path: &Path, debug: bool) -> CompilerResult {
    Compiler::new(2).compile(path, debug)
}

pub fn prime_debug_cp0() -> anyhow::Result<()> {
    // SAFETY: `tolk_prime_debug_cp0` is a pure native initializer that returns
    // either null on success or a malloc-allocated error string on failure.
    let raw = unsafe { tolk_prime_debug_cp0() };
    if raw.is_null() {
        return Ok(());
    }

    // SAFETY: `raw` was checked for null above and points to a valid
    // null-terminated string allocated by `strdup`, so freeing it with libc
    // `free` is correct after copying it into owned Rust memory.
    let message = unsafe {
        let message = CStr::from_ptr(raw).to_string_lossy().into_owned();
        free(raw.cast_mut().cast::<c_void>());
        message
    };
    anyhow::bail!(message)
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
    /// Allow compilation without a contract entrypoint.
    pub allow_no_entrypoint: bool,
    /// Mappings for paths (e.g. "@core" -> "/path/to/core")
    pub mappings: FxHashMap<String, String>,
}

impl Compiler {
    #[must_use]
    pub fn new(opt_level: i64) -> Self {
        Self {
            opt_level,
            with_stack_comments: true,
            with_src_line_comments: true,
            allow_no_entrypoint: false,
            mappings: FxHashMap::default(),
        }
    }

    #[must_use]
    pub const fn with_allow_no_entrypoint(mut self, allow_no_entrypoint: bool) -> Self {
        self.allow_no_entrypoint = allow_no_entrypoint;
        self
    }

    /// Sets mapping that will be used to resolve imports paths.
    ///
    /// For example:
    ///
    /// - `@root`: `foo/bar/`
    ///
    /// `import "@root/baz"` will be resolved to `foo/bar/baz`
    #[must_use]
    pub fn with_mappings(mut self, mappings: &Option<BTreeMap<String, String>>) -> Self {
        if let Some(mappings) = mappings {
            self.mappings = mappings.clone().into_iter().collect();
        }
        self
    }

    /// Run compiler in check mode and return all found errors.
    pub fn check(&self, path: &Path) -> anyhow::Result<Vec<CompilerError>> {
        let result = self.run_internal::<CompilerCheckResult>(path, false, true)?;

        match result {
            CompilerCheckResult::Success(_) => Ok(vec![]),
            CompilerCheckResult::Error(errors) => Ok(errors.errors),
        }
    }

    /// Compiles passed file with Tolk compiler.
    ///
    /// Returns successful result with `code_boc64` or error with `message`.
    #[must_use]
    pub fn compile(&self, path: &Path, with_debug_marks: bool) -> CompilerResult {
        let result = self.run_internal::<CompilerInternalResult>(path, with_debug_marks, false);

        match result {
            Ok(CompilerInternalResult::Success(result)) => {
                let CompilerInternalResultSuccess {
                    fift_code,
                    code_boc64,
                    code_hash_hex,
                    debug_mark_base64,
                    symbol_types_json,
                    debug_marks_json,
                    abi,
                    ..
                } = result;
                let source_map = if let Some(symbol_types) = symbol_types_json {
                    let marks_dict = match crate::debug_marks_dict::parse_debug_marks(
                        debug_mark_base64.as_deref(),
                        &code_boc64,
                    ) {
                        Ok(marks_dict) => marks_dict,
                        Err(err) => {
                            return CompilerResult::Error(CompilerResultError {
                                message: err.to_string(),
                            });
                        }
                    };
                    Some(crate::source_map::SourceMap::from_parts(
                        symbol_types,
                        debug_marks_json.unwrap_or_default(),
                        marks_dict,
                    ))
                } else {
                    None
                };

                CompilerResult::Success(CompilerResultSuccess {
                    fift_code,
                    code_boc64,
                    code_hash_hex,
                    source_map,
                    abi,
                })
            }
            Ok(CompilerInternalResult::Error(result)) => CompilerResult::Error(result),
            Err(err) => CompilerResult::Error(CompilerResultError {
                message: err.to_string(),
            }),
        }
    }

    fn run_internal<TBody: DeserializeOwned>(
        &self,
        path: &Path,
        with_debug_marks: bool,
        check_only: bool,
    ) -> anyhow::Result<TBody> {
        let mut callback_context = FsCallbackContext {
            mappings: self.mappings.clone(),
        };

        let config = serde_json::to_string(&CompilerConfig {
            entrypoint_file_name: path.to_string_lossy().to_string(),
            optimization_level: self.opt_level,
            with_stack_comments: self.with_stack_comments,
            with_src_line_comments: self.with_src_line_comments,
            with_symbol_types: true,
            with_debug_marks,
            json_errors: check_only,
            check_only,
            allow_no_entrypoint: self.allow_no_entrypoint,
        })
        .expect("Critical error, cannot serializer path to JSON, should not happen");

        // SAFETY: we're calling safe C function
        let compilation_result = unsafe {
            unsafe extern "C" fn read_callback(
                kind: std::os::raw::c_int,
                data_ptr: *const c_char,
                dest_contents: *mut *mut c_char,
                dest_error: *mut *mut c_char,
                callback_payload: *mut c_void,
            ) {
                // SAFETY: callback_payload always safe FsCallbackContext object
                let callback_context = unsafe {
                    callback_payload
                        .cast::<FsCallbackContext>()
                        .as_ref()
                        .expect("Missing compiler callback context")
                };

                match FsReadCallbackKind::from(kind) {
                    FsReadCallbackKind::Realpath => {
                        // SAFETY: `data_ptr` is valid not-null pointer
                        let relative_path_raw = unsafe {
                            CStr::from_ptr(data_ptr)
                                .to_str()
                                .expect("Invalid UTF-8 in relative path")
                        };

                        let result = if relative_path_raw.ends_with(".tolk") {
                            callback_context.realpath(relative_path_raw)
                        } else {
                            let mut path = String::with_capacity(relative_path_raw.len() + 5);
                            path.push_str(relative_path_raw);
                            path.push_str(".tolk");
                            callback_context.realpath(&path)
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
            let callback_payload = (&raw mut callback_context).cast::<c_void>();
            tolk_compile(config_cstr.as_ptr(), Some(read_callback), callback_payload)
        };

        // SAFETY: we assume that `compilation_result` is valid C string
        let compilation_result_str = unsafe {
            CString::from_raw(compilation_result.cast_mut())
                .to_string_lossy()
                .to_string()
        };

        let result = serde_json::from_str::<TBody>(&compilation_result_str)
            .map_err(|error| anyhow::anyhow!("cannot parse JSON result from compiler: {error}"))?;
        Ok(result)
    }
}

struct FsCallbackContext {
    mappings: FxHashMap<String, String>,
}

impl FsCallbackContext {
    fn fail_if_symlink(path: &Path) -> Result<(), String> {
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                Err("Cannot import symlink file".to_string())
            }
            _ => Ok(()),
        }
    }

    fn realpath(&self, path_str: &str) -> Result<PathBuf, String> {
        if Path::new(path_str).is_absolute() {
            Self::fail_if_symlink(Path::new(path_str))?;
            return dunce::canonicalize(path_str).map_err(|e| e.to_string());
        }

        if path_str.starts_with("@stdlib/") || path_str.starts_with("@fiftlib/") {
            return Ok(PathBuf::from(path_str));
        }

        if path_str.starts_with('@') {
            let (prefix, suffix) = match path_str.find('/') {
                Some(pos) => (&path_str[..pos], &path_str[pos + 1..]),
                None => (path_str, ""),
            };

            let target = self
                .mappings
                .get(prefix)
                .ok_or_else(|| format!("Unknown path mapping '{prefix}'"))?;
            let cur_mapped_path = Path::new(target).join(suffix);

            Self::fail_if_symlink(&cur_mapped_path)?;
            return dunce::canonicalize(cur_mapped_path).map_err(|e| e.to_string());
        }

        Self::fail_if_symlink(Path::new(path_str))?;
        dunce::canonicalize(path_str).map_err(|e| e.to_string())
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
    #[serde(rename = "withSymbolTypes")]
    pub with_symbol_types: bool,
    #[serde(rename = "withDebugMarks")]
    pub with_debug_marks: bool,
    #[serde(rename = "checkOnly")]
    pub check_only: bool,
    #[serde(rename = "jsonErrors")]
    pub json_errors: bool,
    #[serde(rename = "allowNoEntrypoint")]
    pub allow_no_entrypoint: bool,
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
    pub source_map: Option<crate::source_map::SourceMap>,
    pub abi: Option<ContractABI>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
#[allow(clippy::large_enum_variant)]
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
    #[serde(rename = "debugMarksBase64", default)]
    pub debug_mark_base64: Option<String>,
    #[serde(rename = "symbolTypesJson", default)]
    pub symbol_types_json: Option<crate::source_map::SymbolTypesJson>,
    #[serde(rename = "debugMarksJson", default)]
    pub debug_marks_json: Option<Vec<crate::source_map::DebugMark>>,
    #[serde(rename = "abiJson")]
    pub abi: Option<ContractABI>,
    #[serde(rename = "tolkVersion")]
    pub tolk_version: Option<String>,
    #[serde(rename = "stderr")]
    pub stderr: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CompilerResultError {
    pub message: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
#[allow(clippy::large_enum_variant)]
pub enum CompilerCheckResult {
    Success(CompilerCheckResultSuccess),
    Error(CompilerCheckError),
}

#[derive(Debug, Deserialize)]
pub struct CompilerCheckResultSuccess {
    pub stderr: String,
}

#[derive(Debug, Deserialize)]
pub struct CompilerCheckError {
    pub errors: Vec<CompilerError>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CompilerError {
    pub message: String,
    #[serde(default, alias = "isWarning")]
    pub is_warning: bool,
    pub range: CompilerErrorRange,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CompilerErrorRange {
    pub file_name: String,
    pub start_line_no: usize,
    pub start_char_no: usize,
    pub end_line_no: usize,
    pub end_char_no: usize,
    pub text_inside: String,
}

/// We embed the whole standard library of Tolk and Fift in binary for easier distribution.
pub static TOLK_STDLIB_DIR: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/assets/tolk-stdlib");
static FIFT_STDLIB_DIR: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/assets/fift-stdlib");

fn read_stdlib_file(path: &str) -> Option<&'static str> {
    TOLK_STDLIB_DIR.get_file(path)?.contents_utf8()
}

fn read_fift_stdlib_file(path: &str) -> Option<&'static str> {
    FIFT_STDLIB_DIR.get_file(path)?.contents_utf8()
}

// C FFI declarations

unsafe extern "C" {
    pub fn tolk_prime_debug_cp0() -> *const ::std::os::raw::c_char;
    pub fn tolk_compile(
        config_json: *const ::std::os::raw::c_char,
        callback: WasmFsReadCallback,
        callback_payload: *mut c_void,
    ) -> *const ::std::os::raw::c_char;
    pub fn free(ptr: *mut c_void);
}

type WasmFsReadCallback = Option<
    unsafe extern "C" fn(
        kind: ::std::os::raw::c_int,
        data: *const ::std::os::raw::c_char,
        dest_contents: *mut *mut ::std::os::raw::c_char,
        dest_error: *mut *mut ::std::os::raw::c_char,
        callback_payload: *mut c_void,
    ),
>;

#[cfg(test)]
mod tests {
    use super::CompilerError;

    #[test]
    fn compiler_error_deserializes_warning_flag() {
        let error: CompilerError = serde_json::from_str(
            r#"{
                "message":"warning message",
                "is_warning":true,
                "range":{
                    "file_name":"main.tolk",
                    "start_line_no":1,
                    "start_char_no":1,
                    "end_line_no":1,
                    "end_char_no":5,
                    "text_inside":"main"
                }
            }"#,
        )
        .expect("failed to deserialize compiler warning");

        assert!(error.is_warning);
    }

    #[test]
    fn compiler_error_defaults_warning_flag_to_false() {
        let error: CompilerError = serde_json::from_str(
            r#"{
                "message":"error message",
                "range":{
                    "file_name":"main.tolk",
                    "start_line_no":1,
                    "start_char_no":1,
                    "end_line_no":1,
                    "end_char_no":5,
                    "text_inside":"main"
                }
            }"#,
        )
        .expect("failed to deserialize compiler error");

        assert!(!error.is_warning);
    }
}
