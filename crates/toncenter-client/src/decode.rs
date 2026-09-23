use reqwest::StatusCode;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::value::RawValue;

use crate::{ApiError, Error, ErrorKind};

pub(crate) fn response<T: DeserializeOwned>(
    body: &[u8],
    status: StatusCode,
    operation: &str,
) -> Result<T, Error> {
    let fail = |kind| {
        let mut error = Error::new(kind, operation);
        error.status = Some(status.as_u16());
        error
    };
    let mut deserializer = serde_json::Deserializer::from_slice(body);
    deserializer.disable_recursion_limit();
    let envelope = Envelope::deserialize(serde_stacker::Deserializer::new(&mut deserializer)).ok();
    if let Some(envelope) = envelope
        && (envelope.ok.is_some_and(|value| value.get() == "false") || envelope.error.is_some())
    {
        let mut error = fail(ErrorKind::Api);
        error.api = Some(ApiError {
            code: envelope
                .code
                .and_then(|value| serde_json::from_str(value.get()).ok()),
            message: envelope
                .error
                .or(envelope.result)
                .and_then(|value| serde_json::from_str(value.get()).ok())
                .unwrap_or_else(|| "API request failed".to_owned()),
        });
        return Err(error);
    }
    if !status.is_success() {
        return Err(fail(ErrorKind::Http));
    }
    let mut deserializer = serde_json::Deserializer::from_slice(body);
    deserializer.disable_recursion_limit();
    let result =
        serde_path_to_error::deserialize(serde_stacker::Deserializer::new(&mut deserializer))
            .map_err(|source| {
                let mut error = fail(ErrorKind::Decode);
                error.detail = Some(format!("response field {}", source.path()));
                error
            })?;
    deserializer.end().map_err(|_| fail(ErrorKind::Decode))?;
    Ok(result)
}

#[derive(Deserialize)]
struct Envelope<'a> {
    #[serde(borrow)]
    ok: Option<&'a RawValue>,
    #[serde(borrow)]
    error: Option<&'a RawValue>,
    #[serde(borrow)]
    code: Option<&'a RawValue>,
    #[serde(borrow)]
    result: Option<&'a RawValue>,
}
