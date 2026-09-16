use async_trait::async_trait;
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD},
};
use reqwest::{Client, RequestBuilder, StatusCode, header::USER_AGENT};
use serde::Deserialize;
use std::time::Duration;
use thiserror::Error;

use crate::config::{Config, TonNetwork};

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
    pub fn for_network(config: &Config, network: TonNetwork) -> Self {
        match network {
            TonNetwork::Mainnet => Self::new(
                config.toncenter_mainnet_base_url().to_owned(),
                config.toncenter_mainnet_api_key().map(ToOwned::to_owned),
            ),
            TonNetwork::Testnet => Self::new(
                config.toncenter_testnet_base_url().to_owned(),
                config.toncenter_testnet_api_key().map(ToOwned::to_owned),
            ),
        }
    }

    #[must_use]
    pub fn new(base_url: String, api_key: Option<String>) -> Self {
        Self {
            http: Client::new(),
            base_url,
            api_key,
        }
    }

    fn account_states_request(&self, address: &str) -> RequestBuilder {
        self.toncenter_request("/api/v3/accountStates")
            .timeout(Duration::from_secs(30))
            .query(&[("address", address), ("include_boc", "false")])
    }

    pub(crate) fn toncenter_request(&self, path: &str) -> RequestBuilder {
        let mut request = self
            .http
            .get(format!("{}{}", self.base_url.trim_end_matches('/'), path))
            .header(USER_AGENT, user_agent());

        if let Some(api_key) = &self.api_key {
            request = request.header(TONCENTER_API_KEY_HEADER, api_key);
        }

        request
    }
}

#[derive(Clone)]
pub struct MultiNetworkToncenterClient {
    mainnet: ToncenterClient,
    testnet: ToncenterClient,
}

impl MultiNetworkToncenterClient {
    #[must_use]
    pub fn from_config(config: &Config) -> Self {
        Self {
            mainnet: ToncenterClient::for_network(config, TonNetwork::Mainnet),
            testnet: ToncenterClient::for_network(config, TonNetwork::Testnet),
        }
    }

    #[must_use]
    pub const fn new(mainnet: ToncenterClient, testnet: ToncenterClient) -> Self {
        Self { mainnet, testnet }
    }
}

#[async_trait]
impl BlockchainClient for MultiNetworkToncenterClient {
    async fn get_code_hash(&self, address: &str) -> Result<Option<String>, BlockchainError> {
        let (mainnet, testnet) = tokio::join!(
            self.mainnet.get_code_hash(address),
            self.testnet.get_code_hash(address),
        );
        let mainnet = mainnet?;
        let testnet = testnet?;

        match (mainnet, testnet) {
            (Some(mainnet_code_hash), Some(testnet_code_hash)) => {
                Err(BlockchainError::AddressFoundOnBothNetworks {
                    address: address.to_owned(),
                    mainnet_code_hash,
                    testnet_code_hash,
                })
            }
            (Some(code_hash), None) | (None, Some(code_hash)) => Ok(Some(code_hash)),
            (None, None) => Ok(None),
        }
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

        let code_hash = account_states
            .accounts
            .into_iter()
            .find_map(|account| non_empty_text(account.code_hash));
        if code_hash
            .as_deref()
            .is_some_and(|hash| !is_valid_code_hash(hash))
        {
            return Err(BlockchainError::InvalidCodeHash);
        }
        Ok(code_hash)
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
    normalize_hash(value)
}

pub(crate) fn normalize_hash(value: &str) -> String {
    if is_valid_hash(value) {
        return value.to_ascii_lowercase();
    }

    decode_base64_code_hash(value)
        .map_or_else(|| value.to_owned(), |bytes| bytes_to_lower_hex(&bytes))
}

pub(crate) fn is_valid_code_hash(value: &str) -> bool {
    is_valid_hash(value)
}

pub(crate) fn is_valid_hash(value: &str) -> bool {
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
    #[error("address {address} has code_hash on both TON mainnet and testnet")]
    AddressFoundOnBothNetworks {
        address: String,
        mainnet_code_hash: String,
        testnet_code_hash: String,
    },
    #[error("toncenter returned an invalid code hash")]
    InvalidCodeHash,
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
    use std::io::Write;

    use reqwest::header::USER_AGENT;

    use crate::config::{Config, TonNetwork};

    use super::{
        BlockchainClient, BlockchainError, MultiNetworkToncenterClient, TONCENTER_API_KEY_HEADER,
        ToncenterClient, normalize_code_hash, user_agent,
    };

    #[test]
    fn toncenter_request_has_user_agent() {
        let client = ToncenterClient::new("https://toncenter.com".to_owned(), None);
        let request = client.account_states_request("EQ123").build();
        let Ok(request) = request else {
            panic!("Toncenter request should be valid");
        };
        let expected_user_agent = user_agent();
        assert_eq!(request.timeout(), Some(&std::time::Duration::from_secs(30)));

        assert_eq!(
            request
                .headers()
                .get(USER_AGENT)
                .and_then(|value| value.to_str().ok()),
            Some(expected_user_agent.as_str())
        );
    }

    #[test]
    fn toncenter_client_uses_the_selected_network_settings() {
        let mut config_file = tempfile::NamedTempFile::new().expect("config file");
        writeln!(
            config_file,
            r#"
[toncenter]
mainnet_base_url = "https://mainnet.example.com"
mainnet_api_key = "mainnet-key"
testnet_base_url = "https://testnet.example.com"
testnet_api_key = "testnet-key"
"#
        )
        .expect("write config");
        let config = Config::load_from_path(config_file.path()).expect("load config");

        for (network, expected_host, expected_api_key) in [
            (TonNetwork::Mainnet, "mainnet.example.com", "mainnet-key"),
            (TonNetwork::Testnet, "testnet.example.com", "testnet-key"),
        ] {
            let request = ToncenterClient::for_network(&config, network)
                .account_states_request("0:account")
                .build()
                .expect("request");
            assert_eq!(request.url().host_str(), Some(expected_host));
            assert_eq!(request.url().path(), "/api/v3/accountStates");
            assert_eq!(
                request
                    .headers()
                    .get(TONCENTER_API_KEY_HEADER)
                    .and_then(|value| value.to_str().ok()),
                Some(expected_api_key)
            );
        }
    }

    #[tokio::test]
    async fn account_lookup_rejects_malformed_hashes_and_normalizes_valid_ones() {
        use axum::{Json, Router, routing::get};
        for (hash, expected) in [
            ("../unexpected", None),
            (
                "qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqo=",
                Some("a".repeat(64)),
            ),
        ] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("listener");
            let address = listener.local_addr().expect("address");
            let router =
                Router::new().route(
                    "/api/v3/accountStates",
                    get(move || async move {
                        Json(serde_json::json!({"accounts": [{"code_hash": hash}]}))
                    }),
                );
            let server = tokio::spawn(async move { axum::serve(listener, router).await });
            let result = ToncenterClient::new(format!("http://{address}"), None)
                .get_code_hash("0:account")
                .await;
            server.abort();
            if let Some(expected) = expected {
                assert_eq!(result.expect("valid hash"), Some(expected));
            } else {
                assert!(matches!(result, Err(BlockchainError::InvalidCodeHash)));
            }
        }
    }

    #[tokio::test]
    async fn multi_network_lookup_finds_one_network_and_rejects_duplicates() {
        use axum::{Json, Router, extract::Query, routing::get};
        use std::collections::HashMap;

        const MAINNET_HASH: &str =
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        const TESTNET_HASH: &str =
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let server_address = listener.local_addr().expect("address");
        let router = Router::new()
            .route(
                "/mainnet/api/v3/accountStates",
                get(|Query(query): Query<HashMap<String, String>>| async move {
                    let code_hash = match query.get("address").map(String::as_str) {
                        Some("mainnet-only" | "duplicate") => Some(MAINNET_HASH),
                        _ => None,
                    };
                    Json(serde_json::json!({
                        "accounts": code_hash.map(|code_hash| vec![serde_json::json!({
                            "code_hash": code_hash
                        })]).unwrap_or_default()
                    }))
                }),
            )
            .route(
                "/testnet/api/v3/accountStates",
                get(|Query(query): Query<HashMap<String, String>>| async move {
                    let code_hash = match query.get("address").map(String::as_str) {
                        Some("testnet-only" | "duplicate") => Some(TESTNET_HASH),
                        _ => None,
                    };
                    Json(serde_json::json!({
                        "accounts": code_hash.map(|code_hash| vec![serde_json::json!({
                            "code_hash": code_hash
                        })]).unwrap_or_default()
                    }))
                }),
            );
        let server = tokio::spawn(async move { axum::serve(listener, router).await });
        let client = MultiNetworkToncenterClient::new(
            ToncenterClient::new(format!("http://{server_address}/mainnet"), None),
            ToncenterClient::new(format!("http://{server_address}/testnet"), None),
        );

        assert_eq!(
            client.get_code_hash("mainnet-only").await.expect("lookup"),
            Some(MAINNET_HASH.to_owned())
        );
        assert_eq!(
            client.get_code_hash("testnet-only").await.expect("lookup"),
            Some(TESTNET_HASH.to_owned())
        );
        assert_eq!(client.get_code_hash("missing").await.expect("lookup"), None);
        assert!(matches!(
            client.get_code_hash("duplicate").await,
            Err(BlockchainError::AddressFoundOnBothNetworks {
                address,
                mainnet_code_hash,
                testnet_code_hash,
            }) if address == "duplicate"
                && mainnet_code_hash == MAINNET_HASH
                && testnet_code_hash == TESTNET_HASH
        ));

        server.abort();
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
