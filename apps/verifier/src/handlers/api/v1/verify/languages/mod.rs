use std::{collections::BTreeMap, path::Path};

use serde_json::Value;

use super::{ReceivedFile, SourceMetadata, validate_source_path};
use crate::error::ApiError;

mod func;
mod tact;
mod tolk;

const SOURCE_EXTENSIONS_BY_LANGUAGE: [(&str, &[&str]); 3] = [
    (func::LANGUAGE, &["fc", "func"]),
    (tolk::LANGUAGE, &["tolk"]),
    (tact::LANGUAGE, &["pkg", "tact"]),
];

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
    validate_source_extensions(language, sources)?;

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

fn validate_source_extensions(
    language: Language,
    sources: &[SourceMetadata],
) -> Result<(), ApiError> {
    let language_name = language.as_str();
    let allowed_extensions = SOURCE_EXTENSIONS_BY_LANGUAGE
        .iter()
        .find_map(|(known_language, extensions)| {
            (*known_language == language_name).then_some(*extensions)
        })
        .ok_or_else(|| ApiError::bad_request(format!("unsupported language: {language_name}")))?;

    for source in sources {
        let extension = Path::new(&source.path)
            .extension()
            .and_then(|extension| extension.to_str());
        if extension.is_some_and(|extension| {
            allowed_extensions
                .iter()
                .any(|allowed| extension.eq_ignore_ascii_case(allowed))
        }) {
            continue;
        }

        return Err(ApiError::bad_request(format!(
            "source extension does not match language {}: {}; expected .{}",
            language.as_str(),
            source.path,
            allowed_extensions.join(", ."),
        )));
    }

    Ok(())
}

pub(super) fn is_known_source_extension(extension: &str) -> bool {
    SOURCE_EXTENSIONS_BY_LANGUAGE
        .iter()
        .flat_map(|(_, extensions)| *extensions)
        .any(|known| extension.eq_ignore_ascii_case(known))
}

fn validate_sources(sources: &[SourceMetadata]) -> Result<(), ApiError> {
    if sources.is_empty() {
        return Err(ApiError::bad_request(
            "sources must contain at least one source".to_owned(),
        ));
    }

    let mut seen_paths = BTreeMap::new();
    let mut has_entrypoint = false;

    for source in sources {
        validate_source_path(&source.path)?;
        if Path::new(&source.path).starts_with("output") {
            return Err(ApiError::bad_request(format!(
                "source path uses reserved output directory: {}",
                source.path
            )));
        }
        let normalized_path = source.path.to_ascii_lowercase();
        if let Some(existing_path) = seen_paths.insert(normalized_path, source.path.clone()) {
            return Err(ApiError::bad_request(format!(
                "duplicate source paths: {existing_path}, {}",
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
