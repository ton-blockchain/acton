use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

const SOURCE_BUNDLE_SCHEMA_VERSION: u8 = 1;

pub struct SourceBundleInput<'a> {
    pub compiler: SourceBundleCompiler<'a>,
    pub sources: Vec<SourceBundleSource<'a>>,
    pub files: Vec<SourceBundleFile<'a>>,
}

pub struct SourceBundleCompiler<'a> {
    pub language: &'a str,
    pub version: &'a str,
    pub entrypoint: &'a str,
    pub params: &'a Value,
}

pub struct SourceBundleSource<'a> {
    pub path: &'a str,
    pub include_in_command: Option<bool>,
    pub is_stdlib: Option<bool>,
    pub has_include_directives: Option<bool>,
}

pub struct SourceBundleFile<'a> {
    pub path: &'a str,
    pub bytes: &'a [u8],
}

pub fn compute_source_bundle_hash(
    input: SourceBundleInput<'_>,
) -> Result<String, SourceBundleError> {
    let canonical = CanonicalBundle::from_input(input);
    let bytes = serde_json::to_vec(&canonical).map_err(SourceBundleError::Serialize)?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

#[derive(Debug, Error)]
pub enum SourceBundleError {
    #[error("failed to serialize source bundle hash input: {0}")]
    Serialize(serde_json::Error),
}

#[derive(Serialize)]
struct CanonicalBundle {
    schema_version: u8,
    compiler: CanonicalCompiler,
    sources: Vec<CanonicalSource>,
    files: Vec<CanonicalFile>,
}

impl CanonicalBundle {
    fn from_input(input: SourceBundleInput<'_>) -> Self {
        let mut sources = input
            .sources
            .into_iter()
            .map(|source| CanonicalSource {
                path: source.path.to_owned(),
                include_in_command: source.include_in_command,
                is_stdlib: source.is_stdlib,
                has_include_directives: source.has_include_directives,
            })
            .collect::<Vec<_>>();
        sources.sort_by(|left, right| left.path.cmp(&right.path));

        let mut files = input
            .files
            .into_iter()
            .map(|file| CanonicalFile {
                path: file.path.to_owned(),
                content_hash: hex::encode(Sha256::digest(file.bytes)),
            })
            .collect::<Vec<_>>();
        files.sort_by(|left, right| left.path.cmp(&right.path));

        Self {
            schema_version: SOURCE_BUNDLE_SCHEMA_VERSION,
            compiler: CanonicalCompiler {
                language: input.compiler.language.to_owned(),
                version: input.compiler.version.to_owned(),
                entrypoint: input.compiler.entrypoint.to_owned(),
                params: CanonicalJson::from(input.compiler.params),
            },
            sources,
            files,
        }
    }
}

#[derive(Serialize)]
struct CanonicalCompiler {
    language: String,
    version: String,
    entrypoint: String,
    params: CanonicalJson,
}

#[derive(Serialize)]
struct CanonicalSource {
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    include_in_command: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_stdlib: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    has_include_directives: Option<bool>,
}

#[derive(Serialize)]
struct CanonicalFile {
    path: String,
    content_hash: String,
}

#[derive(Serialize)]
#[serde(untagged)]
enum CanonicalJson {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    Array(Vec<Self>),
    Object(BTreeMap<String, Self>),
}

impl From<&Value> for CanonicalJson {
    fn from(value: &Value) -> Self {
        match value {
            Value::Null => Self::Null,
            Value::Bool(value) => Self::Bool(*value),
            Value::Number(value) => Self::Number(value.clone()),
            Value::String(value) => Self::String(value.clone()),
            Value::Array(values) => Self::Array(values.iter().map(Self::from).collect()),
            Value::Object(values) => Self::Object(
                values
                    .iter()
                    .map(|(key, value)| (key.clone(), Self::from(value)))
                    .collect(),
            ),
        }
    }
}
