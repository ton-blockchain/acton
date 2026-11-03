//! Stateless validation shared by the creation form and file import.

use acton_localnet::{GenesisConfig, GenesisIssue};
use axum::{Json, http::StatusCode};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{EnvironmentRuntimeError, StudioApiError};

/// A genesis document. An empty document selects local development defaults.
#[derive(Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct GenesisDocument {
    toml: String,
}

/// Lossless scalar values for the editor; large prices never pass through JS numbers.
#[derive(Serialize, ToSchema)]
pub(crate) struct GenesisForm {
    toml: String,
    sections: Vec<GenesisSection>,
    issues: Vec<GenesisIssue>,
}

/// Syntax or type failures cannot produce a resolved form. When available, the
/// field path lets an existing editor keep and highlight the user's input.
#[derive(Serialize, ToSchema)]
pub(crate) struct GenesisParseError {
    error: crate::StudioApiErrorDetails,
    fields: Vec<String>,
}

#[derive(Serialize, ToSchema)]
struct GenesisSection {
    path: String,
    fields: Vec<GenesisField>,
}

#[derive(Serialize, ToSchema)]
struct GenesisField {
    key: String,
    value: String,
    boolean: bool,
}

fn sections(value: &serde_json::Value, path: &str, result: &mut Vec<GenesisSection>) {
    let Some(object) = value.as_object() else {
        return;
    };
    let fields = object
        .iter()
        .filter(|(_, value)| !value.is_object())
        .map(|(key, value)| GenesisField {
            key: key.clone(),
            value: value.to_string(),
            boolean: value.is_boolean(),
        })
        .collect::<Vec<_>>();
    if !fields.is_empty() {
        result.push(GenesisSection {
            path: path.to_owned(),
            fields,
        });
    }
    for (key, value) in object.iter().filter(|(_, value)| value.is_object()) {
        let path = if path.is_empty() {
            key.clone()
        } else {
            format!("{path}.{key}")
        };
        sections(value, &path, result);
    }
}

pub(crate) fn validate(
    source: &str,
    block_time_ms: Option<u32>,
    election_time_seconds: Option<u32>,
) -> Result<GenesisConfig, StudioApiError> {
    GenesisConfig::parse_toml(source)
        .and_then(|config| config.with_timing(block_time_ms, election_time_seconds))
        .map_err(|error| {
            StudioApiError(EnvironmentRuntimeError::InvalidRequest {
                code: "invalid_genesis",
                message: format!("{error:#}"),
            })
        })
}

#[utoipa::path(
    post,
    path = "/api/v1/genesis",
    request_body = GenesisDocument,
    responses(
        (status = 200, description = "Genesis draft with resolved defaults and field validation issues", body = GenesisForm),
        (status = 400, description = "Invalid genesis TOML syntax or field type", body = GenesisParseError)
    ),
    tag = "environments"
)]
pub(crate) async fn parse(
    Json(document): Json<GenesisDocument>,
) -> Result<Json<GenesisForm>, (StatusCode, Json<GenesisParseError>)> {
    let config = GenesisConfig::parse_toml(&document.toml).map_err(|error| {
        let fields = error
            .downcast_ref::<GenesisIssue>()
            .map(|issue| issue.fields.clone())
            .unwrap_or_default();
        (
            StatusCode::BAD_REQUEST,
            Json(GenesisParseError {
                error: crate::StudioApiErrorDetails {
                    code: "invalid_genesis",
                    message: format!("{error:#}"),
                },
                fields,
            }),
        )
    })?;
    let mut fields = Vec::new();
    sections(
        &serde_json::to_value(&config).expect("genesis is serializable"),
        "",
        &mut fields,
    );
    Ok(Json(GenesisForm {
        issues: config.validation_issues(),
        toml: config.to_toml().expect("parsed genesis is TOML-compatible"),
        sections: fields,
    }))
}
