use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use super::{ReceivedFile, SourceMetadata, validate_source_path};
use crate::error::ApiError;

mod func;
mod tact;
mod tolk;

pub(super) struct LanguageCompileInput {
    pub language: String,
    pub compiler_version: String,
    pub entrypoint: String,
    pub import_mappings: BTreeMap<String, String>,
}

pub(super) fn prepare(
    language: &str,
    compile_params: &Value,
    sources: &[SourceMetadata],
    files: &BTreeMap<String, ReceivedFile>,
) -> Result<LanguageCompileInput, ApiError> {
    let language = Language::parse(language)?;
    validate_sources(sources)?;

    let entrypoint = language.entrypoint(sources, files)?;
    let compiler_version = language.compiler_version(compile_params, sources, files)?;
    let import_mappings = language.import_mappings(compile_params)?;

    Ok(LanguageCompileInput {
        language: language.as_str().to_owned(),
        compiler_version,
        entrypoint,
        import_mappings,
    })
}

#[derive(Clone, Copy, Debug)]
enum Language {
    Func,
    Tolk,
    Tact,
}

impl Language {
    fn parse(language: &str) -> Result<Self, ApiError> {
        match language.trim().to_ascii_lowercase().as_str() {
            func::LANGUAGE => Ok(Self::Func),
            tolk::LANGUAGE => Ok(Self::Tolk),
            tact::LANGUAGE => Ok(Self::Tact),
            _ => Err(ApiError::bad_request(format!(
                "unsupported language: {language}"
            ))),
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Func => func::LANGUAGE,
            Self::Tolk => tolk::LANGUAGE,
            Self::Tact => tact::LANGUAGE,
        }
    }

    fn entrypoint(
        self,
        sources: &[SourceMetadata],
        files: &BTreeMap<String, ReceivedFile>,
    ) -> Result<String, ApiError> {
        match self {
            Self::Func => func::entrypoint(sources),
            Self::Tolk => tolk::entrypoint(sources),
            Self::Tact => tact::entrypoint(sources, files),
        }
    }

    fn compiler_version(
        self,
        compile_params: &Value,
        sources: &[SourceMetadata],
        files: &BTreeMap<String, ReceivedFile>,
    ) -> Result<String, ApiError> {
        match self {
            Self::Func => func::compiler_version(compile_params),
            Self::Tolk => tolk::compiler_version(compile_params),
            Self::Tact => tact::compiler_version(compile_params, sources, files),
        }
    }

    fn import_mappings(self, compile_params: &Value) -> Result<BTreeMap<String, String>, ApiError> {
        match self {
            Self::Tolk => tolk::import_mappings(compile_params),
            Self::Func | Self::Tact => Ok(BTreeMap::new()),
        }
    }
}

fn validate_sources(sources: &[SourceMetadata]) -> Result<(), ApiError> {
    if sources.is_empty() {
        return Err(ApiError::bad_request(
            "sources must contain at least one source".to_owned(),
        ));
    }

    let mut seen_paths = BTreeSet::new();
    let mut has_entrypoint = false;

    for source in sources {
        validate_source_path(&source.path)?;
        if !seen_paths.insert(source.path.clone()) {
            return Err(ApiError::bad_request(format!(
                "duplicate source path: {}",
                source.path
            )));
        }
        if source.is_entrypoint {
            if has_entrypoint {
                return Err(ApiError::bad_request(
                    "multiple entrypoint sources were provided".to_owned(),
                ));
            }
            has_entrypoint = true;
        }
    }

    Ok(())
}

fn string_param(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(key).and_then(Value::as_str))
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
}
