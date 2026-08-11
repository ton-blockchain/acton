use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use utoipa::ToSchema;

use crate::{
    compilers::CompilerError, payment::PaymentError, registry::RegistryError,
    registry_index::VerificationIndexError, source_bundle::SourceBundleError,
    source_storage::SourceStorageError, verification::VerificationError,
};

const INTERNAL_ERROR_MESSAGE: &str = "internal verifier error";
const RETRYABLE_SOURCE_STORAGE_ERROR: &str =
    "verification_retryable: source storage is temporarily unavailable";

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
    expose_message: bool,
    public_fallback: &'static str,
    payment_retryable: bool,
}

impl ApiError {
    pub const fn bad_request(message: String) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message,
            expose_message: true,
            public_fallback: INTERNAL_ERROR_MESSAGE,
            payment_retryable: false,
        }
    }

    const fn hidden_bad_gateway(message: String) -> Self {
        Self {
            status: StatusCode::BAD_GATEWAY,
            message,
            expose_message: false,
            public_fallback: INTERNAL_ERROR_MESSAGE,
            payment_retryable: false,
        }
    }

    const fn retryable_source_storage(message: String) -> Self {
        Self {
            status: StatusCode::BAD_GATEWAY,
            message,
            expose_message: false,
            public_fallback: RETRYABLE_SOURCE_STORAGE_ERROR,
            payment_retryable: true,
        }
    }

    pub const fn unauthorized(message: String) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message,
            expose_message: true,
            public_fallback: INTERNAL_ERROR_MESSAGE,
            payment_retryable: false,
        }
    }

    pub const fn payment_required(message: String) -> Self {
        Self {
            status: StatusCode::PAYMENT_REQUIRED,
            message,
            expose_message: true,
            public_fallback: INTERNAL_ERROR_MESSAGE,
            payment_retryable: false,
        }
    }

    pub const fn conflict(message: String) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message,
            expose_message: true,
            public_fallback: INTERNAL_ERROR_MESSAGE,
            payment_retryable: false,
        }
    }

    pub const fn service_unavailable(message: String) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message,
            expose_message: true,
            public_fallback: INTERNAL_ERROR_MESSAGE,
            payment_retryable: false,
        }
    }

    #[must_use]
    pub const fn is_payment_retryable(&self) -> bool {
        self.payment_retryable
    }

    pub const fn not_found(message: String) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message,
            expose_message: true,
            public_fallback: INTERNAL_ERROR_MESSAGE,
            payment_retryable: false,
        }
    }
}

impl From<VerificationError> for ApiError {
    fn from(err: VerificationError) -> Self {
        match err {
            VerificationError::CodeHashNotFound { .. } => Self::not_found(err.to_string()),
            VerificationError::Blockchain(blockchain_err) => {
                Self::hidden_bad_gateway(blockchain_err.to_string())
            }
            err => Self::bad_request(err.to_string()),
        }
    }
}

impl From<CompilerError> for ApiError {
    fn from(err: CompilerError) -> Self {
        match err {
            CompilerError::CompileFailed(message) => Self::bad_request(message),
            err => Self::hidden_bad_gateway(err.to_string()),
        }
    }
}

impl From<RegistryError> for ApiError {
    fn from(err: RegistryError) -> Self {
        match err {
            RegistryError::SourceStorage(err)
            | RegistryError::VerificationIndex(VerificationIndexError::SourceStorage(err)) => {
                Self::from(err)
            }
            err => Self::hidden_bad_gateway(err.to_string()),
        }
    }
}

impl From<SourceBundleError> for ApiError {
    fn from(err: SourceBundleError) -> Self {
        Self::bad_request(err.to_string())
    }
}

impl From<SourceStorageError> for ApiError {
    fn from(err: SourceStorageError) -> Self {
        let retryable = source_storage_error_is_payment_retryable(&err);
        let message = err.to_string();
        if retryable {
            Self::retryable_source_storage(message)
        } else {
            Self::hidden_bad_gateway(message)
        }
    }
}

fn source_storage_error_is_payment_retryable(err: &SourceStorageError) -> bool {
    match err {
        SourceStorageError::CreateDir { .. }
        | SourceStorageError::WriteFile { .. }
        | SourceStorageError::RemoveDir { .. }
        | SourceStorageError::ReadDir { .. }
        | SourceStorageError::ReadFile { .. } => true,
        SourceStorageError::Git { command, .. } | SourceStorageError::GitSpawn { command, .. } => {
            command.starts_with("git push ")
        }
        SourceStorageError::MissingConfig(_)
        | SourceStorageError::InvalidPath { .. }
        | SourceStorageError::ReadFileUtf8 { .. }
        | SourceStorageError::SerializeManifest(_)
        | SourceStorageError::DeserializeManifest { .. }
        | SourceStorageError::FileHashMismatch { .. }
        | SourceStorageError::UnpreparedSourceRepository { .. }
        | SourceStorageError::DirtySourceRepository { .. }
        | SourceStorageError::GitOutputUtf8 { .. }
        | SourceStorageError::DetachedHead
        | SourceStorageError::CleanupFailed { .. }
        | SourceStorageError::Operation(_) => false,
    }
}

impl From<PaymentError> for ApiError {
    fn from(err: PaymentError) -> Self {
        match err {
            PaymentError::RecoveryInProgress => Self::service_unavailable(err.to_string()),
            PaymentError::AlreadyUsed | PaymentError::InProgress => Self::conflict(err.to_string()),
            PaymentError::TransactionNotFound
            | PaymentError::InvalidTransaction
            | PaymentError::MissingAmount
            | PaymentError::InsufficientAmount { .. }
            | PaymentError::CodeHashMismatch => Self::payment_required(err.to_string()),
            _ => Self::hidden_bad_gateway(err.to_string()),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let message = if self.expose_message {
            self.message
        } else {
            tracing::error!(
                status = %self.status,
                error = %self.message,
                "verifier operation failed"
            );
            self.public_fallback.to_owned()
        };

        (self.status, Json(ErrorResponse { error: message })).into_response()
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub error: String,
}

#[cfg(test)]
mod tests {
    use std::{
        io::{self, Write},
        path::PathBuf,
        sync::{Arc, Mutex},
    };

    use axum::body::{Body, to_bytes};
    use tracing_subscriber::fmt::MakeWriter;

    use super::*;

    #[derive(Clone, Default)]
    struct LogBuffer(Arc<Mutex<Vec<u8>>>);

    struct LogWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for LogWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0
                .lock()
                .expect("log buffer mutex should not be poisoned")
                .write(bytes)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl<'a> MakeWriter<'a> for LogBuffer {
        type Writer = LogWriter;

        fn make_writer(&'a self) -> Self::Writer {
            LogWriter(Arc::clone(&self.0))
        }
    }

    impl LogBuffer {
        fn content(&self) -> String {
            String::from_utf8(
                self.0
                    .lock()
                    .expect("log buffer mutex should not be poisoned")
                    .clone(),
            )
            .expect("log output should be UTF-8")
        }
    }

    async fn response_body(response: Response<Body>) -> String {
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body should be readable");
        String::from_utf8(bytes.to_vec()).expect("response body should be UTF-8")
    }

    fn git_errors(secret: &str) -> [SourceStorageError; 3] {
        let invalid_utf8 =
            String::from_utf8(vec![0xff]).expect_err("invalid UTF-8 fixture should fail to decode");
        [
            SourceStorageError::Git {
                command: format!("git push https://user:{secret}@example.com/repository.git main"),
                status: "exit status: 1".to_owned(),
                stderr: "authentication failed".to_owned(),
            },
            SourceStorageError::GitSpawn {
                command: format!("git push https://user:{secret}@example.com/repository.git main"),
                source: io::Error::other(secret.to_owned()),
            },
            SourceStorageError::GitOutputUtf8 {
                command: format!("git show https://user:{secret}@example.com/repository.git"),
                source: invalid_utf8,
            },
        ]
    }

    #[tokio::test]
    async fn transient_source_storage_errors_are_retryable_without_exposing_details() {
        let secret = "secret-token";

        for error in git_errors(secret).into_iter().take(2) {
            let error = ApiError::from(error);
            assert!(error.is_payment_retryable());
            let response = error.into_response();
            assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
            let body = response_body(response).await;
            assert_eq!(
                body,
                r#"{"error":"verification_retryable: source storage is temporarily unavailable"}"#
            );
            assert!(!body.contains(secret));
        }
    }

    #[tokio::test]
    async fn stringified_git_error_details_are_not_returned_to_client() {
        let secret = "secret-token";

        for error in git_errors(secret) {
            let error = SourceStorageError::UnpreparedSourceRepository {
                path: PathBuf::from("source-repository"),
                message: error.to_string(),
            };
            let error = ApiError::from(error);
            assert!(!error.is_payment_retryable());
            let response = error.into_response();
            assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
            let body = response_body(response).await;
            assert_eq!(body, r#"{"error":"internal verifier error"}"#);
            assert!(!body.contains(secret));
        }
    }

    #[tokio::test]
    async fn ordinary_unprepared_repository_message_is_not_returned_to_client() {
        let message = "root commit must contain only `.gitattributes`";
        let error = SourceStorageError::UnpreparedSourceRepository {
            path: PathBuf::from("source-repository"),
            message: message.to_owned(),
        };
        let error = ApiError::from(error);
        assert!(!error.is_payment_retryable());
        let response = error.into_response();

        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        let body = response_body(response).await;
        assert_eq!(body, r#"{"error":"internal verifier error"}"#);
        assert!(!body.contains(message));
    }

    #[tokio::test]
    async fn deterministic_source_storage_error_is_not_payment_retryable() {
        let secret = "invalid-storage-path-detail";
        let error = ApiError::from(SourceStorageError::InvalidPath {
            path: PathBuf::from("source-repository"),
            message: secret.to_owned(),
        });
        assert!(!error.is_payment_retryable());

        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        let body = response_body(response).await;
        assert_eq!(body, r#"{"error":"internal verifier error"}"#);
        assert!(!body.contains(secret));
    }

    #[tokio::test]
    async fn wrapped_git_error_details_are_not_returned_to_client() {
        let secret = "secret-token";
        let git_error = || SourceStorageError::Git {
            command: format!("git push https://user:{secret}@example.com/repository.git main"),
            status: "exit status: 1".to_owned(),
            stderr: "authentication failed".to_owned(),
        };
        let errors = [
            RegistryError::SourceStorage(git_error()),
            RegistryError::VerificationIndex(VerificationIndexError::SourceStorage(git_error())),
        ];

        for error in errors {
            let error = ApiError::from(error);
            assert!(error.is_payment_retryable());
            let response = error.into_response();
            assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
            let body = response_body(response).await;
            assert_eq!(
                body,
                r#"{"error":"verification_retryable: source storage is temporarily unavailable"}"#
            );
            assert!(!body.contains(secret));
        }
    }

    #[tokio::test]
    async fn compiler_server_error_is_hidden_and_not_payment_retryable() {
        let error = ApiError::from(CompilerError::Timeout { timeout_ms: 5_000 });
        assert!(!error.is_payment_retryable());

        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(
            response_body(response).await,
            r#"{"error":"internal verifier error"}"#
        );
    }

    #[tokio::test]
    async fn payment_provider_and_sqlite_details_are_hidden() {
        let secret = "internal-payment-detail";
        let errors = [
            PaymentError::Provider {
                status: 500,
                body: secret.to_owned(),
            },
            PaymentError::Sqlite(rusqlite::Error::InvalidQuery),
        ];

        for error in errors {
            let error = ApiError::from(error);
            assert!(!error.is_payment_retryable());
            let response = error.into_response();
            assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
            let body = response_body(response).await;
            assert_eq!(body, r#"{"error":"internal verifier error"}"#);
            assert!(!body.contains(secret));
        }
    }

    #[tokio::test]
    async fn hidden_git_error_details_are_written_to_application_log() {
        let logs = LogBuffer::default();
        let subscriber = tracing_subscriber::fmt()
            .with_ansi(false)
            .without_time()
            .with_writer(logs.clone())
            .finish();
        let secret = "secret-token";
        let git_error = SourceStorageError::Git {
            command: format!("git push https://user:{secret}@example.com/repository.git main"),
            status: "exit status: 1".to_owned(),
            stderr: "authentication failed".to_owned(),
        };
        let error = RegistryError::VerificationIndex(VerificationIndexError::SourceStorage(
            SourceStorageError::UnpreparedSourceRepository {
                path: PathBuf::from("source-repository"),
                message: git_error.to_string(),
            },
        ));

        let response =
            tracing::subscriber::with_default(subscriber, || ApiError::from(error).into_response());

        let body = response_body(response).await;
        assert!(!body.contains(secret));
        assert!(logs.content().contains(secret));
    }
}
