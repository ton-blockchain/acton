use crate::Rule;
use crate::rules::violation::Violation;
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

#[derive(Debug, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct Diagnostic {
    pub file_id: FileId,
    pub rule: Rule,
    pub name: &'static str,
    pub severity: Severity,
    pub code: Option<String>,
    pub message: String,
    pub annotations: Vec<Annotation>,
    pub fixes: Vec<Fix>,
    pub help: Option<String>,
}

impl Diagnostic {
    pub fn for_violation<V: Violation>(file_id: FileId, severity: Severity, violation: V) -> Self {
        Self {
            file_id,
            rule: V::rule(),
            name: V::rule().name(),
            severity,
            code: V::code().map(str::to_string),
            message: violation.message(),
            annotations: vec![],
            fixes: vec![],
            help: None,
        }
    }

    pub fn warning_for<V: Violation>(file_id: FileId, violation: V) -> Self {
        Self::for_violation(file_id, Severity::Warning, violation)
    }

    pub fn error_for<V: Violation>(file_id: FileId, violation: V) -> Self {
        Self::for_violation(file_id, Severity::Error, violation)
    }

    pub fn help_for<V: Violation>(file_id: FileId, violation: V, message: &str) -> Self {
        let mut d = Self::for_violation(file_id, Severity::Help, violation);
        message.clone_into(&mut d.message);
        d
    }

    #[must_use]
    pub fn with_annotations(mut self, annotations: Vec<Annotation>) -> Self {
        self.annotations = annotations;
        self
    }

    #[must_use]
    pub fn with_fixes(mut self, fixes: Vec<Fix>) -> Self {
        self.fixes = fixes;
        self
    }

    #[must_use]
    pub fn with_help<T: AsRef<str>>(mut self, help: T) -> Self {
        self.help = Some(help.as_ref().to_owned());
        self
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct Fix {
    pub message: String,
    pub edits: Vec<Edit>,
    pub applicability: Applicability,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct Edit {
    pub span: Span,
    pub replacement: String,
    pub file_id: FileId,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum DiagnosticTag {
    /// Unused or unnecessary code. Used for unused parameters, unreachable code, etc.
    Unnecessary,
    /// Deprecated or obsolete code.
    Deprecated,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
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
