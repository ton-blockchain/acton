use anyhow::anyhow;
use dap::prelude::Request;
use serde_json::Value;
use std::io::BufRead;

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub(crate) enum IncomingRequest {
    Known(Request),
    Unsupported { seq: i64, command: String },
}

pub(crate) fn poll_request<R: BufRead>(
    input_buffer: &mut R,
) -> anyhow::Result<Option<IncomingRequest>> {
    let mut content_length = None;

    loop {
        let mut line = String::new();
        let read_size = input_buffer.read_line(&mut line)?;
        if read_size == 0 {
            return Ok(None);
        }

        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            if content_length.is_some() {
                break;
            }
            continue;
        }

        let Some((header, value)) = trimmed.split_once(':') else {
            return Err(anyhow!("Invalid DAP header: {trimmed}"));
        };

        if header == "Content-Length" {
            content_length = Some(value.trim().parse()?);
        } else {
            return Err(anyhow!("Invalid DAP header: {trimmed}"));
        }
    }

    let Some(content_length) = content_length else {
        return Err(anyhow!("Missing Content-Length header"));
    };

    let mut content = vec![0; content_length];
    input_buffer.read_exact(&mut content)?;

    let mut value: Value = serde_json::from_slice(&content)?;
    normalize_request_value(&mut value);
    match serde_json::from_value::<Request>(value.clone()) {
        Ok(request) => Ok(Some(IncomingRequest::Known(request))),
        Err(err) => {
            let is_request = value.get("type").and_then(Value::as_str) == Some("request");
            let seq = value.get("seq").and_then(Value::as_i64);
            let command = value
                .get("command")
                .and_then(Value::as_str)
                .map(str::to_owned);

            if is_request && let (Some(seq), Some(command)) = (seq, command) {
                return Ok(Some(IncomingRequest::Unsupported { seq, command }));
            }

            Err(anyhow!("Error while deserializing DAP request: {err}"))
        }
    }
}

/// Normalize requests.
///
/// JetBrains sends custom empty `configurationDone`.
fn normalize_request_value(value: &mut Value) {
    if value.get("type").and_then(Value::as_str) != Some("request") {
        return;
    }
    if value.get("command").and_then(Value::as_str) != Some("configurationDone") {
        return;
    }

    let has_empty_arguments = value
        .get("arguments")
        .and_then(Value::as_object)
        .is_some_and(|arguments| arguments.is_empty());
    if !has_empty_arguments {
        return;
    }

    if let Some(object) = value.as_object_mut() {
        object.remove("arguments");
    }
}
