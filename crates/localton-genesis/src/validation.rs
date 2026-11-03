use serde::Serialize;
use utoipa::ToSchema;

/// A failed constraint and the fields that can be edited to satisfy it.
///
/// Paths use TOML dotted keys, including table names. A relationship can name
/// several fields; this does not imply that all of them must change.
#[derive(Debug, Clone, Serialize, PartialEq, Eq, ToSchema)]
pub struct GenesisIssue {
    /// Fields involved in the failed constraint, relative to the document root
    pub fields: Vec<String>,
    /// Human-readable explanation shared by CLI and Studio
    pub message: String,
}

impl std::fmt::Display for GenesisIssue {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for GenesisIssue {}

pub(crate) fn check(
    issues: &mut Vec<GenesisIssue>,
    valid: bool,
    prefix: &str,
    fields: &[&str],
    message: impl Into<String>,
) {
    if !valid {
        issues.push(GenesisIssue {
            fields: fields
                .iter()
                .map(|field| {
                    if prefix.is_empty() {
                        (*field).to_owned()
                    } else {
                        format!("{prefix}.{field}")
                    }
                })
                .collect(),
            message: message.into(),
        });
    }
}
