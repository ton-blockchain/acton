use crate::error::LocalnetError;
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::future::Future;
use std::time::{SystemTime, UNIX_EPOCH};
use ton_api::toncenter::v2::StringOrNumber;

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct ToncenterHttpError {
    status: StatusCode,
    message: String,
}

impl ToncenterHttpError {
    pub fn conflict(message: impl Into<String>) -> anyhow::Error {
        Self {
            status: StatusCode::CONFLICT,
            message: message.into(),
        }
        .into()
    }

    pub fn unprocessable_entity(message: impl Into<String>) -> anyhow::Error {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            message: message.into(),
        }
        .into()
    }
}

#[must_use]
pub fn error_status(error: &anyhow::Error) -> StatusCode {
    if error
        .downcast_ref::<crate::api::toncenter_v2::RunGetMethodStackDepthError>()
        .is_some()
    {
        return StatusCode::from_u16(533).expect("533 is a valid extension status");
    }
    if let Some(error) = error.downcast_ref::<ToncenterHttpError>() {
        return error.status;
    }
    if let Some(error) = error.downcast_ref::<LocalnetError>() {
        return match error {
            LocalnetError::ProtocolViolation { .. } | LocalnetError::InvalidRequest { .. } => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            LocalnetError::MasterchainWaitTimeout { .. } => StatusCode::GATEWAY_TIMEOUT,
            LocalnetError::BlockNotFound { .. }
            | LocalnetError::BlockLookupNotFound { .. }
            | LocalnetError::BlockDataNotFound { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        };
    }
    StatusCode::INTERNAL_SERVER_ERROR
}

pub fn parse_params<T: DeserializeOwned>(params: Value, method: &str) -> anyhow::Result<T> {
    serde_json::from_value(params).map_err(|_| {
        ToncenterHttpError::unprocessable_entity(format!("Invalid params for {method}"))
    })
}

pub fn parse_method_name(method: &StringOrNumber) -> anyhow::Result<String> {
    match method {
        StringOrNumber::String(value) => Ok(value.clone()),
        StringOrNumber::Number(_) | StringOrNumber::Unsigned(_) => {
            method.to_i32().map(|value| value.to_string()).map_err(|_| {
                ToncenterHttpError::unprocessable_entity(
                    "numeric `method` must be a signed 32-bit integer",
                )
            })
        }
    }
}

pub async fn handle_result<T, F, M>(
    result: impl Future<Output = anyhow::Result<T>>,
    mapper: F,
) -> Response
where
    F: FnOnce(&T) -> M,
    M: Serialize,
{
    match result.await {
        Ok(res) => Json(ton_api::toncenter::v2::TonlibResponse {
            ok: true,
            result: mapper(&res),
            extra: get_extra(),
        })
        .into_response(),
        Err(e) => {
            let status = error_status(&e);
            (
                status,
                Json(ton_api::toncenter::v2::TonlibErrorResponse {
                    ok: false,
                    error: e.to_string(),
                    code: i32::from(status.as_u16()),
                    extra: get_extra(),
                    jsonrpc: None,
                    id: None,
                }),
            )
                .into_response()
        }
    }
}

#[must_use]
pub fn get_extra() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or_else(|_| "0".to_string(), |d| d.as_millis().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::toncenter_v2::RunGetMethodStackDepthError;

    #[test]
    fn stack_depth_error_uses_upstream_status() {
        let error = anyhow::Error::new(RunGetMethodStackDepthError);
        assert_eq!(error_status(&error).as_u16(), 533);
    }

    #[test]
    fn numeric_method_name_must_fit_openapi_int32() {
        assert_eq!(
            parse_method_name(&StringOrNumber::Number(i64::from(i32::MIN))).unwrap(),
            i32::MIN.to_string()
        );
        assert_eq!(
            parse_method_name(&StringOrNumber::Unsigned(i32::MAX as u64)).unwrap(),
            i32::MAX.to_string()
        );
        assert!(parse_method_name(&StringOrNumber::Number(i64::from(i32::MAX) + 1)).is_err());
        assert!(parse_method_name(&StringOrNumber::Unsigned(i32::MAX as u64 + 1)).is_err());
        assert_eq!(
            parse_method_name(&StringOrNumber::String("2147483648".to_owned())).unwrap(),
            "2147483648"
        );
    }
}
