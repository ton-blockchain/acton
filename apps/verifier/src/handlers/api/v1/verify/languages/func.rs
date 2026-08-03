use serde_json::Value;

use super::{SourceMetadata, string_param};
use crate::error::ApiError;

pub(super) const LANGUAGE: &str = "func";

pub(super) fn compiler_version(compile_params: &Value) -> Result<String, ApiError> {
    string_param(compile_params, &["compiler_version"]).ok_or_else(|| {
        ApiError::bad_request(
            "missing compiler version for func: provide compile_params.compiler_version".to_owned(),
        )
    })
}

pub(super) fn entrypoint(sources: &[SourceMetadata]) -> Result<String, ApiError> {
    sources
        .iter()
        .find(|source| source.is_entrypoint)
        .or_else(|| {
            sources
                .iter()
                .find(|source| source.include_in_command.unwrap_or(false))
        })
        .or_else(|| sources.first())
        .map(|source| source.path.clone())
        .ok_or_else(|| ApiError::bad_request("missing FunC source".to_owned()))
}
