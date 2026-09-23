use std::fmt;

/// Category of failure, suitable for application decisions without parsing text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// Invalid endpoint, header, parameters, or client configuration.
    Configuration,
    /// The HTTP connection or response-body transfer failed.
    Transport,
    /// The operation deadline or an individual attempt timeout expired.
    Timeout,
    /// The server returned a structured API error, including errors inside HTTP 200.
    Api,
    /// An unsuccessful HTTP response did not contain a supported API error.
    Http,
    /// JSON syntax or the response type did not match the expected contract.
    Decode,
    /// The response body exceeded the configured byte limit.
    ResponseTooLarge,
}

/// Structured server diagnostic. The code is independent of the HTTP status.
#[derive(Clone)]
pub struct ApiError {
    /// Protocol-specific error code, when supplied by the server.
    pub code: Option<i32>,
    /// Server diagnostic text. Treat its contents as untrusted before displaying or logging it.
    pub message: String,
}

impl fmt::Debug for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApiError")
            .field("code", &self.code)
            .finish_non_exhaustive()
    }
}

/// Failure of one logical call, retaining its operation, status, and API diagnostic.
/// Display and debug output omit server response bodies and request credentials.
pub struct Error {
    pub(crate) kind: ErrorKind,
    pub(crate) operation: String,
    pub(crate) status: Option<u16>,
    pub(crate) attempts: u32,
    pub(crate) api: Option<ApiError>,
    pub(crate) detail: Option<String>,
    pub(crate) source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl Error {
    pub(crate) fn new(kind: ErrorKind, operation: impl Into<String>) -> Self {
        Self {
            kind,
            operation: operation.into(),
            status: None,
            attempts: 0,
            api: None,
            detail: None,
            source: None,
        }
    }

    pub(crate) fn configuration(detail: impl Into<String>) -> Self {
        let mut error = Self::new(ErrorKind::Configuration, "configure");
        error.detail = Some(detail.into());
        error
    }

    pub(crate) fn with_source(
        mut self,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        self.source = Some(Box::new(source));
        self
    }

    /// Failure category; retries have already been applied according to client policy.
    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Operation name without query parameters or request payloads.
    #[must_use]
    pub fn operation(&self) -> &str {
        &self.operation
    }

    /// Last received HTTP status; absent when no response headers were received.
    #[must_use]
    pub const fn status(&self) -> Option<u16> {
        self.status
    }

    /// Number of attempts started before this error was returned.
    #[must_use]
    pub const fn attempts(&self) -> u32 {
        self.attempts
    }

    /// Server error code and diagnostic, when a structured error was received.
    #[must_use]
    pub const fn api_error(&self) -> Option<&ApiError> {
        self.api.as_ref()
    }

    /// Configuration explanation or JSON field path, without the offending value.
    #[must_use]
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TON Center {}: {:?}", self.operation, self.kind)?;
        if let Some(status) = self.status {
            write!(f, " (HTTP {status})")?;
        }
        if let Some(code) = self.api.as_ref().and_then(|api| api.code) {
            write!(f, " (API {code})")?;
        }
        if let Some(detail) = &self.detail {
            write!(f, ": {detail}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Error")
            .field("kind", &self.kind)
            .field("operation", &self.operation)
            .field("status", &self.status)
            .field("attempts", &self.attempts)
            .field("api", &self.api)
            .field("detail", &self.detail)
            .finish_non_exhaustive()
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_deref().map(|source| source as _)
    }
}
