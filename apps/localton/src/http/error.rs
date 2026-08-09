//! Converts handler failures into JSON HTTP responses.
//!
//! Any error convertible to [`anyhow::Error`] can become [`HttpError`]. Axum
//! renders it as HTTP 400 with the body `{ "error": "..." }`. Config and admin
//! handlers use this type as their common error return value.

use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::Serialize;
use utoipa::ToSchema;

/// JSON error response from a Localton HTTP API
#[derive(Debug, Serialize, ToSchema)]
pub(super) struct ErrorResponse {
    /// Error message for the request
    error: String,
}

pub(super) struct HttpError(anyhow::Error);

impl<E> From<E> for HttpError
where
    E: Into<anyhow::Error>,
{
    fn from(error: E) -> Self {
        Self(error.into())
    }
}

impl IntoResponse for HttpError {
    fn into_response(self) -> axum::response::Response {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: self.0.to_string(),
            }),
        )
            .into_response()
    }
}
