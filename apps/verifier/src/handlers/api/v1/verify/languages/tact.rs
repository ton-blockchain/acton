use std::{collections::BTreeMap, path::Path};

use serde_json::Value;

use super::{ReceivedFile, SourceMetadata, string_param, validate_source_path};
use crate::error::ApiError;

pub(super) const LANGUAGE: &str = "tact";

pub(super) fn compiler_version(
    compile_params: &Value,
    sources: &[SourceMetadata],
    files: &BTreeMap<String, ReceivedFile>,
) -> Result<String, ApiError> {
    string_param(compile_params, &["compiler_version"])
        .map_or_else(|| compiler_version_from_pkg(sources, files), Ok)
}

pub(super) fn entrypoint(
    sources: &[SourceMetadata],
    files: &BTreeMap<String, ReceivedFile>,
) -> Result<String, ApiError> {
    let pkg = pkg_json(sources, files)?;
    let parameters = pkg
        .pointer("/compiler/parameters")
        .ok_or_else(|| ApiError::bad_request("missing Tact compiler parameters".to_owned()))?;
    let parameters = match parameters {
        Value::String(raw) => serde_json::from_str::<Value>(raw).map_err(|err| {
            ApiError::bad_request(format!("invalid Tact compiler parameters JSON: {err}"))
        })?,
        Value::Object(_) => parameters.clone(),
        _ => {
            return Err(ApiError::bad_request(
                "invalid Tact compiler parameters: expected JSON string or object".to_owned(),
            ));
        }
    };
    let entrypoint = parameters
        .get("entrypoint")
        .and_then(Value::as_str)
        .filter(|entrypoint| !entrypoint.trim().is_empty())
        .ok_or_else(|| ApiError::bad_request("missing Tact compiler entrypoint".to_owned()))?;
    let entrypoint = entrypoint.trim().trim_start_matches("./").to_owned();
    validate_source_path(&entrypoint)?;

    Ok(entrypoint)
}

fn compiler_version_from_pkg(
    sources: &[SourceMetadata],
    files: &BTreeMap<String, ReceivedFile>,
) -> Result<String, ApiError> {
    let pkg = pkg_json(sources, files)?;

    pkg.pointer("/compiler/version")
        .and_then(Value::as_str)
        .filter(|version| !version.trim().is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            ApiError::bad_request(
                "missing Tact compiler version: provide compile_params.compiler_version or pkg.compiler.version"
                    .to_owned(),
            )
        })
}

fn pkg_json(
    sources: &[SourceMetadata],
    files: &BTreeMap<String, ReceivedFile>,
) -> Result<Value, ApiError> {
    let pkg_path = pkg_entrypoint(sources)?;
    let pkg = files.get(&pkg_path).ok_or_else(|| {
        ApiError::bad_request(format!("source metadata has no uploaded file: {pkg_path}"))
    })?;

    serde_json::from_slice::<Value>(&pkg.content)
        .map_err(|err| ApiError::bad_request(format!("invalid Tact pkg JSON: {err}")))
}

fn pkg_entrypoint(sources: &[SourceMetadata]) -> Result<String, ApiError> {
    sources
        .iter()
        .filter(|source| {
            Path::new(&source.path)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("pkg"))
        })
        .min_by_key(|source| source.path.split('/').count())
        .map(|source| source.path.clone())
        .ok_or_else(|| ApiError::bad_request("missing Tact .pkg source".to_owned()))
}
