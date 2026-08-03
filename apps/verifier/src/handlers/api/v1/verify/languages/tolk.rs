use std::collections::BTreeMap;

use serde_json::Value;

use super::{SourceMetadata, string_param};
use crate::error::ApiError;

pub(super) const LANGUAGE: &str = "tolk";

pub(super) fn compiler_version(compile_params: &Value) -> Result<String, ApiError> {
    string_param(compile_params, &["compiler_version"]).ok_or_else(|| {
        ApiError::bad_request(
            "missing compiler version for tolk: provide compile_params.compiler_version".to_owned(),
        )
    })
}

pub(super) fn entrypoint(sources: &[SourceMetadata]) -> Result<String, ApiError> {
    sources
        .iter()
        .find(|source| source.is_entrypoint)
        .map(|source| source.path.clone())
        .ok_or_else(|| ApiError::bad_request("missing entrypoint source".to_owned()))
}

pub(super) fn import_mappings(
    compile_params: &Value,
) -> Result<BTreeMap<String, String>, ApiError> {
    compile_params.get("import_mappings").map_or_else(
        || Ok(BTreeMap::new()),
        |value| {
            serde_json::from_value::<BTreeMap<String, String>>(value.clone())
                .map_err(|err| ApiError::bad_request(format!("invalid import_mappings: {err}")))
        },
    )
}
