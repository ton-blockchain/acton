use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use utoipa::ToSchema;

use crate::{
    compilers::CompilerError, registry::RegistryError, registry_index::VerificationIndexError,
    source_bundle::SourceBundleError, source_storage::SourceStorageError,
    verification::VerificationError,
};

const INTERNAL_ERROR_MESSAGE: &str = "internal verifier error";

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
    expose_message: bool,
}

impl ApiError {
    pub const fn bad_request(message: String) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message,
            expose_message: true,
        }
    }

    pub const fn bad_gateway(message: String) -> Self {
        Self {
            status: StatusCode::BAD_GATEWAY,
            message,
            expose_message: true,
        }
    }

    const fn hidden_bad_gateway(message: String) -> Self {
        Self {
            status: StatusCode::BAD_GATEWAY,
            message,
            expose_message: false,
        }
    }

    pub const fn unauthorized(message: String) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message,
            expose_message: true,
        }
    }

    pub const fn not_found(message: String) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message,
            expose_message: true,
        }
    }
}

impl From<VerificationError> for ApiError {
    fn from(err: VerificationError) -> Self {
        match err {
            VerificationError::CodeHashNotFound { .. } => Self::not_found(err.to_string()),
            VerificationError::Blockchain(blockchain_err) => {
                Self::bad_gateway(blockchain_err.to_string())
            }
            err => Self::bad_request(err.to_string()),
        }
    }
}

impl From<CompilerError> for ApiError {
    fn from(err: CompilerError) -> Self {
        match err {
            CompilerError::CompileFailed(message) => Self::bad_request(message),
            err => Self::bad_gateway(err.to_string()),
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
            err => Self::bad_gateway(err.to_string()),
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
        let should_hide_message = should_hide_source_storage_message(&err);
        let message = err.to_string();

        if should_hide_message {
            Self::hidden_bad_gateway(message)
        } else {
            Self::bad_gateway(message)
        }
    }
}

fn should_hide_source_storage_message(err: &SourceStorageError) -> bool {
    match err {
        SourceStorageError::Git { .. }
        | SourceStorageError::GitSpawn { .. }
        | SourceStorageError::GitOutputUtf8 { .. } => true,
        SourceStorageError::UnpreparedSourceRepository { message, .. } => {
            message.starts_with("git command failed:")
                || message.starts_with("failed to spawn git command ")
                || message.starts_with("git output was not valid UTF-8 for ")
        }
        _ => false,
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
            INTERNAL_ERROR_MESSAGE.to_owned()
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
    async fn git_error_details_are_not_returned_to_client() {
        let secret = "secret-token";

        for error in git_errors(secret) {
            let response = ApiError::from(error).into_response();
            assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
            let body = response_body(response).await;
            assert_eq!(body, r#"{"error":"internal verifier error"}"#);
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
            let response = ApiError::from(error).into_response();
            assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
            let body = response_body(response).await;
            assert_eq!(body, r#"{"error":"internal verifier error"}"#);
            assert!(!body.contains(secret));
        }
    }

    #[tokio::test]
    async fn ordinary_unprepared_repository_message_is_returned_to_client() {
        let message = "root commit must contain only `.gitattributes`";
        let error = SourceStorageError::UnpreparedSourceRepository {
            path: PathBuf::from("source-repository"),
            message: message.to_owned(),
        };
        let response = ApiError::from(error).into_response();

        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert!(response_body(response).await.contains(message));
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
            let response = ApiError::from(error).into_response();
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
