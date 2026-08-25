//! Reusable Rust source generation from Tolk contract ABI JSON.
//!
//! Add this crate to the project's build dependencies, then generate bindings
//! into Cargo's output directory:
//!
//! ```ignore
//! // build.rs
//! use std::{env, fs, path::PathBuf};
//!
//! fn main() {
//!     let abi = PathBuf::from("abi/Counter.abi.json");
//!     println!("cargo:rerun-if-changed={}", abi.display());
//!
//!     let bindings = acton_client_codegen::generate_from_file(&abi)
//!         .expect("Counter ABI must generate Rust bindings");
//!     let output = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR must be set"));
//!     fs::write(output.join("counter.rs"), bindings).expect("bindings must be written");
//! }
//! ```

//!
//! Include the result from the project source:
//!
//! ```ignore
//! pub mod counter {
//!     include!(concat!(env!("OUT_DIR"), "/counter.rs"));
//! }
//! ```

mod backend;
mod cell_codec;
mod const_values;
mod declarations;
mod names;
mod rust_types;
mod stack_codec;
mod symbols;
mod wrappers;

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use std::error::Error as StdError;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use syn::Error;
use tolk_source_map::abi::ContractABI;

/// Tolk ABI schema version understood by this generator.
pub const SUPPORTED_ABI_SCHEMA_VERSION: &str = "1.0";

/// Generation settings shared by file generation and proc-macro expansion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenerateOptions {
    /// Embed the original ABI as the generated `ABI_JSON` constant.
    pub embed_abi_json: bool,
}

impl Default for GenerateOptions {
    fn default() -> Self {
        Self {
            embed_abi_json: true,
        }
    }
}

/// An error produced while reading, parsing, or generating Rust bindings.
#[derive(Debug)]
pub enum CodegenError {
    /// The ABI file could not be read.
    ReadAbi { path: PathBuf, source: io::Error },
    /// The ABI JSON does not match the Tolk ABI schema.
    ParseAbi(serde_json::Error),
    /// The ABI uses a schema version unsupported by this generator.
    UnsupportedSchemaVersion { actual: String },
    /// The ABI contains a construct unsupported by the Rust backend.
    Generate(Error),
}

fn generate_error(message: impl Into<String>) -> CodegenError {
    CodegenError::Generate(Error::new(Span::call_site(), message.into()))
}

impl fmt::Display for CodegenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadAbi { path, source } => {
                write!(
                    formatter,
                    "failed to read ABI `{}`: {source}",
                    path.display()
                )
            }
            Self::ParseAbi(source) => write!(formatter, "failed to parse ABI JSON: {source}"),
            Self::UnsupportedSchemaVersion { actual } => write!(
                formatter,
                "unsupported ABI schema version `{actual}`; expected `{SUPPORTED_ABI_SCHEMA_VERSION}`"
            ),
            Self::Generate(source) => source.fmt(formatter),
        }
    }
}

impl StdError for CodegenError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::ReadAbi { source, .. } => Some(source),
            Self::ParseAbi(source) => Some(source),
            Self::Generate(source) => Some(source),
            Self::UnsupportedSchemaVersion { .. } => None,
        }
    }
}

/// Generates formatted Rust source from Tolk ABI JSON.
pub fn generate(abi_json: &str) -> Result<String, CodegenError> {
    generate_with_options(abi_json, GenerateOptions::default())
}

/// Reads a Tolk ABI JSON file and generates formatted Rust source.
pub fn generate_from_file(path: impl AsRef<Path>) -> Result<String, CodegenError> {
    let path = path.as_ref();
    let abi_json = fs::read_to_string(path).map_err(|source| CodegenError::ReadAbi {
        path: path.to_owned(),
        source,
    })?;
    generate(&abi_json)
}

/// Generates formatted Rust source with explicit generation settings.
pub fn generate_with_options(
    abi_json: &str,
    options: GenerateOptions,
) -> Result<String, CodegenError> {
    let tokens = generate_tokens_with_options(abi_json, options)?;
    let file = syn::parse2(tokens).map_err(CodegenError::Generate)?;
    Ok(prettyplease::unparse(&file))
}

/// Generates Rust tokens for adapters such as `acton-client-macros`.
pub fn generate_tokens_with_options(
    abi_json: &str,
    options: GenerateOptions,
) -> Result<TokenStream2, CodegenError> {
    let abi = serde_json::from_str::<ContractABI>(abi_json).map_err(CodegenError::ParseAbi)?;
    if abi.abi_schema_version != SUPPORTED_ABI_SCHEMA_VERSION {
        return Err(CodegenError::UnsupportedSchemaVersion {
            actual: abi.abi_schema_version,
        });
    }

    let generated = backend::RustBackend::new(&abi).emit()?;
    let abi_json = options
        .embed_abi_json
        .then(|| quote! { pub const ABI_JSON: &str = #abi_json; });

    Ok(quote! {
        #abi_json
        #generated
    })
}
