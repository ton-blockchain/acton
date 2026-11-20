use axum::{
    Json,
    extract::Path,
    http::{HeaderMap, StatusCode, header::USER_AGENT},
    response::{IntoResponse, Redirect},
};

use crate::blockchain::is_valid_code_hash;

const UNSUPPORTED_BLUEPRINT_ERROR: &str =
    "This verifier is no longer supported. Update @ton/blueprint to version 0.47.1 or newer";
const UNSUPPORTED_ACTON_ERROR: &str =
    "This version of acton verify is no longer supported. Update Acton to version 1.2.0 or newer";

pub async fn source_file_redirect(
    Path((prefix, hash_suffix, file)): Path<(String, String, String)>,
) -> Result<Redirect, StatusCode> {
    if prefix.len() != 2 {
        return Err(StatusCode::NOT_FOUND);
    }

    let code_hash = format!("{prefix}{hash_suffix}").to_ascii_lowercase();
    if !is_valid_code_hash(&code_hash) {
        return Err(StatusCode::NOT_FOUND);
    }

    let query = form_urlencoded::Serializer::new(String::new())
        .append_pair("file", &file)
        .finish();

    Ok(Redirect::permanent(&format!("/{code_hash}?{query}")))
}

pub async fn source_handler(headers: HeaderMap) -> impl IntoResponse {
    let is_acton = headers
        .get(USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("acton/"));

    let message = if is_acton {
        UNSUPPORTED_ACTON_ERROR
    } else {
        UNSUPPORTED_BLUEPRINT_ERROR
    };

    (StatusCode::GONE, Json(message))
}
