use async_trait::async_trait;
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD},
};
use reqwest::{Client, RequestBuilder, StatusCode, header::USER_AGENT};
use serde::Deserialize;
use thiserror::Error;

use crate::config::Config;

const TONCENTER_API_KEY_HEADER: &str = "X-API-Key";
const CODE_HASH_BYTES: usize = 32;

fn user_agent() -> String {
    let git_hash = option_env!("GIT_HASH").unwrap_or("unknown");
    format!("ton-verifier/{} ({git_hash})", env!("CARGO_PKG_VERSION"))
}

#[async_trait]
pub trait BlockchainClient: Send + Sync + 'static {
    async fn get_code_hash(&self, address: &str) -> Result<Option<String>, BlockchainError>;
}

#[derive(Clone)]
pub struct ToncenterClient {
    http: Client,
    base_url: String,
    api_key: Option<String>,
}

impl ToncenterClient {
    #[must_use]
    pub fn from_config(config: &Config) -> Self {
        Self::new(
            config.toncenter_base_url().to_owned(),
            config.toncenter_api_key().map(ToOwned::to_owned),
        )
    }

    #[must_use]
    pub fn new(base_url: String, api_key: Option<String>) -> Self {
        Self {
            http: Client::new(),
            base_url,
            api_key,
        }
    }

    fn account_states_url(&self) -> String {
        format!(
            "{}/api/v3/accountStates",
            self.base_url.trim_end_matches('/')
        )
    }

    fn account_states_request(&self, address: &str) -> RequestBuilder {
        let mut request = self
            .http
            .get(self.account_states_url())
            .query(&[("address", address), ("include_boc", "false")])
            .header(USER_AGENT, user_agent());

        if let Some(api_key) = &self.api_key {
            request = request.header(TONCENTER_API_KEY_HEADER, api_key);
        }

        request
    }
}

#[async_trait]
impl BlockchainClient for ToncenterClient {
    async fn get_code_hash(&self, address: &str) -> Result<Option<String>, BlockchainError> {
        let response = self
            .account_states_request(address)
            .send()
            .await
            .map_err(BlockchainError::Transport)?;
        let status = response.status();
        let body = response.text().await.map_err(BlockchainError::Transport)?;

        if !status.is_success() {
            return Err(BlockchainError::api(status, body));
        }

        let account_states =
            serde_json::from_str::<AccountStatesResponse>(&body).map_err(BlockchainError::Json)?;

        Ok(account_states
            .accounts
            .into_iter()
            .find_map(|account| non_empty_text(account.code_hash)))
    }
}

fn non_empty_text(value: Option<String>) -> Option<String> {
    let value = value?;
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    Some(normalize_code_hash(value))
}

pub(crate) fn normalize_code_hash(value: &str) -> String {
    if is_hex_code_hash(value) {
        return value.to_ascii_lowercase();
    }

    decode_base64_code_hash(value)
        .map_or_else(|| value.to_owned(), |bytes| bytes_to_lower_hex(&bytes))
}

fn is_hex_code_hash(value: &str) -> bool {
    value.len() == CODE_HASH_BYTES * 2 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn decode_base64_code_hash(value: &str) -> Option<Vec<u8>> {
    [STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD]
        .into_iter()
        .find_map(|engine| {
            engine
                .decode(value)
                .ok()
                .filter(|bytes| bytes.len() == CODE_HASH_BYTES)
        })
}

fn bytes_to_lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

#[derive(Debug, Error)]
pub enum BlockchainError {
    #[error("toncenter transport error: {0}")]
    Transport(reqwest::Error),
    #[error("toncenter API error: status={status}, body={body}")]
    Api { status: StatusCode, body: String },
    #[error("toncenter malformed response: {0}")]
    Json(serde_json::Error),
}

impl BlockchainError {
    const fn api(status: StatusCode, body: String) -> Self {
        Self::Api { status, body }
    }
}

#[derive(Debug, Deserialize)]
struct AccountStatesResponse {
    accounts: Vec<AccountState>,
}

#[derive(Debug, Deserialize)]
struct AccountState {
    code_hash: Option<String>,
}

#[cfg(test)]
mod tests {
    use reqwest::header::USER_AGENT;

    use super::{ToncenterClient, normalize_code_hash, user_agent};

    #[test]
    fn toncenter_request_has_user_agent() {
        let client = ToncenterClient::new("https://toncenter.com".to_owned(), None);
        let request = client.account_states_request("EQ123").build();
        let Ok(request) = request else {
            panic!("Toncenter request should be valid");
        };
        let expected_user_agent = user_agent();

        assert_eq!(
            request
                .headers()
                .get(USER_AGENT)
                .and_then(|value| value.to_str().ok()),
            Some(expected_user_agent.as_str())
        );
    }

    #[test]
    fn normalize_code_hash_keeps_hex_as_lowercase() {
        assert_eq!(
            normalize_code_hash("AF8F72E22D3DD6EEC1F312693C026E4D1751E2DFEC9B3F6577E8C8B3A668947C"),
            "af8f72e22d3dd6eec1f312693c026e4d1751e2dfec9b3f6577e8c8b3a668947c"
        );
    }

    #[test]
    fn normalize_code_hash_decodes_base64_to_hex() {
        assert_eq!(
            normalize_code_hash("r49y4i091u7B8xJpPAJuTRdR4t/smz9ld+jIs6ZolHw="),
            "af8f72e22d3dd6eec1f312693c026e4d1751e2dfec9b3f6577e8c8b3a668947c"
        );
    }
}
