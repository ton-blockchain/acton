#[cfg(test)]
mod tests;

use async_trait::async_trait;
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD},
};
use serde::Deserialize;
use thiserror::Error;
use toncenter_client::Client;

use crate::config::{Config, TonNetwork};

const CODE_HASH_BYTES: usize = 32;

fn user_agent() -> String {
    let git_hash = option_env!("GIT_HASH").unwrap_or("unknown");
    format!("ton-verifier/{} ({git_hash})", env!("CARGO_PKG_VERSION"))
}

#[async_trait]
pub trait BlockchainClient: Send + Sync + 'static {
    async fn get_code_hash(&self, address: &str) -> Result<Option<String>, BlockchainError>;
}

/// Creates a v3 client with the configured endpoint and API key for one network.
///
/// # Errors
/// Returns an error if the endpoint URL or HTTP headers are invalid.
pub fn client_for_network(
    config: &Config,
    network: TonNetwork,
) -> Result<Client, toncenter_client::Error> {
    match network {
        TonNetwork::Mainnet => client(
            config.toncenter_mainnet_base_url(),
            config.toncenter_mainnet_api_key().map(ToOwned::to_owned),
        ),
        TonNetwork::Testnet => client(
            config.toncenter_testnet_base_url(),
            config.toncenter_testnet_api_key().map(ToOwned::to_owned),
        ),
    }
}

/// Creates a v3 client for a TON Center server, including the verifier's User-Agent.
///
/// # Errors
/// Returns an error if the base URL or API key cannot be used in an HTTP request.
pub fn client(base_url: &str, api_key: Option<String>) -> Result<Client, toncenter_client::Error> {
    Client::builder()
        .v3_url(format!("{}/api/v3", base_url.trim_end_matches('/')))
        .user_agent(user_agent())
        .api_key(api_key)
        .build()
}

#[derive(Clone)]
pub struct MultiNetworkToncenterClient {
    mainnet: Client,
    testnet: Client,
}

impl MultiNetworkToncenterClient {
    /// Creates clients for both networks to detect addresses deployed on either one.
    ///
    /// # Errors
    /// Returns an error if either network's endpoint URL or HTTP headers are invalid.
    pub fn from_config(config: &Config) -> Result<Self, toncenter_client::Error> {
        Ok(Self {
            mainnet: client_for_network(config, TonNetwork::Mainnet)?,
            testnet: client_for_network(config, TonNetwork::Testnet)?,
        })
    }

    #[must_use]
    pub const fn new(mainnet: Client, testnet: Client) -> Self {
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
impl BlockchainClient for Client {
    async fn get_code_hash(&self, address: &str) -> Result<Option<String>, BlockchainError> {
        let account_states: AccountStatesResponse = self
            .v3_get(
                "accountStates",
                &[("address", address), ("include_boc", "false")],
            )
            .await?;

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
    #[error(transparent)]
    Provider(#[from] toncenter_client::Error),
}

#[derive(Debug, Deserialize)]
struct AccountStatesResponse {
    accounts: Vec<AccountState>,
}

#[derive(Debug, Deserialize)]
struct AccountState {
    code_hash: Option<String>,
}
