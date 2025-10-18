use include_dir::{Dir, include_dir};
use serde::{Deserialize, Serialize};
use std::ffi::{CStr, CString, c_char};
use std::fs::{canonicalize, read_to_string};
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn compile(path: &Path) -> serde_json::Result<CompilerResult> {
    Compiler::new().compile(path)
}

pub struct Compiler {
    opt_level: i64,
    fift_path: Option<String>,
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            opt_level: 2,
            fift_path: None,
        }
    }

    pub fn compile(&self, path: &Path) -> serde_json::Result<CompilerResult> {
        let config = serde_json::to_string(&CompilerConfig {
            entrypoint_file_name: path.to_string_lossy().to_string(),
            optimization_level: self.opt_level,
            with_stack_comments: false,
            with_src_line_comments: false,
            experimental_options: "".to_string(),
            fift_path: self
                .fift_path
                .clone()
                .unwrap_or("/Users/petrmakhnev/emulator-rs/crates/tolkc/assets/fift/".to_string()),
        })?;

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

            let config_str = CString::new(config).unwrap();
            tolk_compile(config_str.as_ptr(), Some(read_callback))
        };

        let compilation_result_str = unsafe {
            CString::from_raw(compilation_result.cast_mut())
                .to_string_lossy()
                .to_string()
        };

        serde_json::from_str::<CompilerResult>(&compilation_result_str)
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
    Error(ResultError),
}

#[derive(Deserialize)]
pub struct CompilerResultSuccess {
    #[serde(rename = "fiftCode")]
    pub _fift_code: String,
    #[serde(rename = "codeBoc64")]
    pub code_boc64: String,
    #[serde(rename = "codeHashHex")]
    pub _code_hash_hex: String,
}

#[derive(Deserialize)]
pub struct ResultError {
    pub message: String,
}

static TOLK_STDLIB_DIR: Dir = include_dir!("./crates/tolkc/assets/tolk-stdlib");

fn read_stdlib_file(path: &str) -> Option<&'static str> {
    TOLK_STDLIB_DIR.get_file(path)?.contents_utf8()
}

unsafe extern "C" {
    pub fn tolk_compile(
        config_json: *const ::std::os::raw::c_char,
        callback: WasmFsReadCallback,
    ) -> *const ::std::os::raw::c_char;
}

pub type WasmFsReadCallback = Option<
    unsafe extern "C" fn(
        kind: ::std::os::raw::c_int,
        data: *const ::std::os::raw::c_char,
        dest_contents: *mut *mut ::std::os::raw::c_char,
        dest_error: *mut *mut ::std::os::raw::c_char,
    ),
>;
