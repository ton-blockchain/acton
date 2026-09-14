use tycho_types::models::{StdAddr, StdAddrFormat};

use crate::{
    blockchain::{is_valid_code_hash, normalize_code_hash},
    error::ApiError,
};

pub(super) fn optional_address(value: Option<String>) -> Result<Option<String>, ApiError> {
    let Some(address) = non_empty_trimmed(value) else {
        return Ok(None);
    };

    StdAddr::from_str_ext(&address, StdAddrFormat::any())
        .map_err(|_| ApiError::bad_request("invalid TON address".to_owned()))?;
    Ok(Some(address))
}

pub(super) fn code_hash(value: &str) -> Result<String, ApiError> {
    let code_hash = normalize_code_hash(value.trim());
    if !is_valid_code_hash(&code_hash) {
        return Err(ApiError::bad_request(
            "code_hash must contain exactly 64 hexadecimal characters".to_owned(),
        ));
    }
    Ok(code_hash)
}

pub(super) fn optional_code_hash(value: Option<String>) -> Result<Option<String>, ApiError> {
    non_empty_trimmed(value)
        .map(|value| code_hash(&value))
        .transpose()
}

fn non_empty_trimmed(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}
