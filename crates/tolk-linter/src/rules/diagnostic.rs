use tolk_resolver::FileId;
use tolk_resolver::file_index::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub enum Applicability {
    /// The fix can be applied automatically without user intervention
    Auto,
    /// The fix requires manual review and confirmation before application
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub enum Severity {
    Info,
    Warning,
    Error,
    Fatal,
    Help,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Diagnostic {
    pub file_id: FileId,
    pub severity: Severity,
    pub code: Option<String>,
    pub message: String,
    pub annotations: Vec<Annotation>,
    pub fixes: Vec<Fix>,
    pub help: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Fix {
    pub message: String,
    pub edits: Vec<Edit>,
    pub applicability: Applicability,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Edit {
    pub span: Span,
    pub replacement: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum DiagnosticTag {
    /// Unused or unnecessary code. Used for unused parameters, unreachable code, etc.
    Unnecessary,
    /// Deprecated or obsolete code.
    Deprecated,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Annotation {
    /// The span of this annotation, corresponding to some subsequence of the
    /// user's input that we want to highlight.
    pub span: Span,
    /// An optional message associated with this annotation's span.
    ///
    /// When present, rendering will include this message in the output and
    /// draw a line between the highlighted span and the message.
    pub message: Option<String>,
    /// Whether this annotation is "primary" or not. When it isn't primary, an
    /// annotation is said to be "secondary."
    pub is_primary: bool,
    /// The diagnostic tags associated with this annotation.
    pub tags: Vec<DiagnosticTag>,
}
