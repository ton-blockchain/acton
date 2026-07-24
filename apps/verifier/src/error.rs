use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use utoipa::ToSchema;

use crate::{
    compilers::CompilerError, registry::RegistryError, source_bundle::SourceBundleError,
    source_storage::SourceStorageError, verification::VerificationError,
};

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    pub const fn bad_request(message: String) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message,
        }
    }

    pub const fn bad_gateway(message: String) -> Self {
        Self {
            status: StatusCode::BAD_GATEWAY,
            message,
        }
    }

    pub const fn not_found(message: String) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message,
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
        Self::bad_gateway(err.to_string())
    }
}

impl From<SourceBundleError> for ApiError {
    fn from(err: SourceBundleError) -> Self {
        Self::bad_request(err.to_string())
    }
}

impl From<SourceStorageError> for ApiError {
    fn from(err: SourceStorageError) -> Self {
        Self::bad_gateway(err.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorResponse {
                error: self.message,
            }),
        )
            .into_response()
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub error: String,
}
