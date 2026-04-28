use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// JSON report emitted by `acton check --output-format json`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(title = "Acton Lint JSON Report Schema")]
pub struct LintJsonReport {
    /// Whether the lint run completed without errors or warning-limit failures.
    pub success: bool,
    /// Diagnostics emitted by the lint run.
    pub diagnostics: Vec<LintJsonDiagnostic>,
}

/// A single lint diagnostic entry in the JSON report.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LintJsonDiagnostic {
    /// Absolute path to the file where the diagnostic was reported.
    pub file: String,
    /// Normalized severity used in JSON output.
    pub severity: LintJsonSeverity,
    /// Stable lint rule name.
    pub name: String,
    /// Stable lint rule code when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// Primary human-readable message.
    pub message: String,
    /// Optional help text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    /// Related source annotations for the diagnostic.
    pub annotations: Vec<LintJsonAnnotation>,
    /// Available fixes for the diagnostic.
    pub fixes: Vec<LintJsonFix>,
    /// Diagnostic source identifier.
    pub source: LintJsonSource,
}

/// A source annotation attached to a lint diagnostic.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LintJsonAnnotation {
    /// 0-based source range for the annotation.
    pub range: LintJsonRange,
    /// Human-readable annotation text when provided by the diagnostic.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Whether this is the primary annotation.
    pub is_primary: bool,
    /// Optional annotation tags such as `deprecated`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<LintJsonAnnotationTag>>,
}

/// Additional metadata attached to an annotation.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum LintJsonAnnotationTag {
    /// Marks code that can be removed.
    Unnecessary,
    /// Marks deprecated code usage.
    Deprecated,
}

/// A suggested fix for a lint diagnostic.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LintJsonFix {
    /// Human-readable fix summary.
    pub message: String,
    /// Text edits that implement the fix.
    pub edits: Vec<LintJsonFixEdit>,
    /// Whether the fix can be applied automatically.
    pub applicability: LintJsonFixApplicability,
}

/// A single edit inside a suggested fix.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LintJsonFixEdit {
    /// 0-based source range to replace.
    pub range: LintJsonRange,
    /// Replacement text for the source range.
    #[serde(rename = "newText")]
    pub new_text: String,
    /// Absolute path to the file that should be edited.
    pub file: String,
}

/// A 0-based source range.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LintJsonRange {
    /// Inclusive start position.
    pub start: LintJsonPosition,
    /// Exclusive end position.
    pub end: LintJsonPosition,
}

/// A 0-based line/character position.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LintJsonPosition {
    /// 0-based line number.
    pub line: u32,
    /// 0-based UTF-8 column offset.
    pub character: u32,
}

/// Normalized diagnostic severity used in JSON output.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum LintJsonSeverity {
    /// Warning-level diagnostic.
    Warning,
    /// Error-level diagnostic.
    Error,
    /// Informational diagnostic.
    Info,
}

/// Whether a fix can be applied automatically.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum LintJsonFixApplicability {
    /// Safe to apply automatically.
    Auto,
    /// Requires manual review or application.
    Manual,
}

/// Diagnostic source identifier.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum LintJsonSource {
    /// The Tolk linter emitted this diagnostic.
    Tolk,
}
