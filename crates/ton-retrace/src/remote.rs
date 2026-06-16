//! Internal HTTP clients for interacting with external TON APIs (`TonCenter`, `TonHub`).

use crate::Network;
use crate::types::{BlockInfo, BlocksResponse, TransactionData, TransactionTransactionsResponse};
use anyhow::Context;
use reqwest::Client;
use reqwest::header::USER_AGENT;
use serde::Deserialize;
use std::env;
use std::ffi::OsStr;
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use toncenter_keys::api_key as toncenter_api_key;
use tycho_types::boc::Boc;
use tycho_types::prelude::Cell;

const USE_PROXY_ENV: &str = "ACTON_USE_PROXY";
const TONCENTER_MIN_REQUEST_INTERVAL: Duration = Duration::from_millis(1200);
static TONCENTER_REQUEST_GATE: LazyLock<Mutex<Option<Instant>>> =
    LazyLock::new(|| Mutex::new(None));

const fn user_agent() -> &'static str {
    concat!("acton/", env!("CARGO_PKG_VERSION"))
}

fn http_client_builder() -> reqwest::ClientBuilder {
    let builder = Client::builder();
    if proxy_enabled() {
        builder
    } else {
        builder.no_proxy()
    }
}

fn proxy_enabled() -> bool {
    proxy_enabled_from_value(env::var_os(USE_PROXY_ENV).as_deref())
}

fn proxy_enabled_from_value(value: Option<&OsStr>) -> bool {
    value.is_some_and(|value| {
        let value = value.to_string_lossy();
        let value = value.trim();
        value == "1" || value == "true"
    })
}

/// Client for `TonCenter` V2/V3 API.
///
/// Used for fetching transaction metadata, block information, and library cells.
pub(crate) struct TonCenterClient {
    client: Client,
    api_key: Option<String>,
    base_url: String,
}

impl TonCenterClient {
    /// Creates a new `TonCenter` client for the specified network.
    pub(crate) fn new(network: Network) -> anyhow::Result<Self> {
        let base_url = match network {
            Network::Mainnet => "https://toncenter.com/api/v3".to_string(),
            Network::Testnet => "https://testnet.toncenter.com/api/v3".to_string(),
            Network::Localnet | Network::Custom(_) => {
                anyhow::bail!("Network {network} is not yet supported in retrace")
            }
        };
        Ok(Self {
            client: http_client_builder().build()?,
            api_key: toncenter_api_key(&network),
            base_url,
        })
    }

    /// Applies a simple global rate limit for unauthenticated `TonCenter` requests.
    ///
    /// `TonCenter` has stricter limits without an API key, so we serialize
    /// requests and keep at least 1 second between request starts.
    async fn maybe_wait_for_rate_limit(&self) {
        if self.api_key.is_some() {
            return;
        }

        let mut last_request = TONCENTER_REQUEST_GATE.lock().await;
        if let Some(last) = *last_request {
            let elapsed = last.elapsed();
            if elapsed < TONCENTER_MIN_REQUEST_INTERVAL {
                let wait_for = TONCENTER_MIN_REQUEST_INTERVAL - elapsed;
                log::debug!("throttle for {wait_for:?}");
                tokio::time::sleep(wait_for).await;
            }
        }
        *last_request = Some(Instant::now());
    }

    /// Fetches transaction metadata by its hash using V3 API.
    pub(crate) async fn get_transactions(
        &self,
        hash: &str,
        limit: u32,
    ) -> anyhow::Result<TransactionData> {
        let mut request = self
            .client
            .get(format!("{}/transactions", self.base_url))
            .header(USER_AGENT, user_agent())
            .query(&[("hash", hash), ("limit", &limit.to_string())]);

        if let Some(key) = &self.api_key {
            request = request.header("X-API-Key", key);
        }

        self.maybe_wait_for_rate_limit().await;
        let response = request.send().await?;
        if !response.status().is_success() {
            anyhow::bail!("TonCenter V3 returned status: {}", response.status());
        }

        let result: serde_json::Value = response.json().await?;

        if let Some(error) = result.get("error") {
            anyhow::bail!("TonCenter V3 error: {error}");
        }

        let response_data: TransactionData = serde_json::from_value(result)
            .map_err(|e| anyhow::anyhow!("Failed to decode TonCenter V3 response: {e}"))?;
        Ok(response_data)
    }

    /// Fetches block information by workchain, shard, and seqno using V3 API.
    pub(crate) async fn get_blocks(
        &self,
        workchain: i32,
        shard: &str,
        seqno: u32,
    ) -> anyhow::Result<BlocksResponse> {
        let mut request = self
            .client
            .get(format!("{}/blocks", self.base_url))
            .header(USER_AGENT, user_agent())
            .query(&[
                ("workchain", workchain.to_string()),
                ("shard", shard.to_string()),
                ("seqno", seqno.to_string()),
            ]);

        if let Some(key) = &self.api_key {
            request = request.header("X-API-Key", key);
        }

        self.maybe_wait_for_rate_limit().await;
        let response = request.send().await?;
        if !response.status().is_success() {
            anyhow::bail!("TonCenter V3 returned status: {}", response.status());
        }

        let result: serde_json::Value = response.json().await?;

        if let Some(error) = result.get("error") {
            anyhow::bail!("TonCenter V3 error: {error}");
        }

        let response_data: BlocksResponse = serde_json::from_value(result)
            .map_err(|e| anyhow::anyhow!("Failed to decode TonCenter V3 response: {e}"))?;
        Ok(response_data)
    }

    /// Fetches transactions for an account using `TonCenter` V2 JSON-RPC.
    ///
    /// Used as a fallback or for specific V2-only functionality.
    pub(crate) async fn get_transactions_toncenter(
        &self,
        address: &str,
        lt: u64,
        hash: &str,
        to_lt: u64,
        limit: u32,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        let url = format!("{}/jsonRPC", self.base_url.replace("/api/v3", "/api/v2"));

        let body = serde_json::json!({
            "id": "1",
            "jsonrpc": "2.0",
            "method": "getTransactions",
            "params": {
                "address": address,
                "lt": lt.to_string(),
                "hash": hash,
                "to_lt": to_lt.to_string(),
                "limit": limit,
                "archival": true
            }
        });

        let mut request = self
            .client
            .post(url)
            .header(USER_AGENT, user_agent())
            .json(&body);

        if let Some(key) = &self.api_key {
            request = request.header("X-API-Key", key);
        }

        self.maybe_wait_for_rate_limit().await;
        let response = request.send().await?;
        if !response.status().is_success() {
            anyhow::bail!("TonCenter V2 returned status: {}", response.status());
        }

        let result: serde_json::Value = response.json().await?;

        if let Some(error) = result.get("error") {
            anyhow::bail!("TonCenter V2 error: {error}");
        }

        let result = result.get("result").and_then(|v| v.as_array()).cloned();
        Ok(result.unwrap_or_default())
    }

    /// Fetches library cells (T-libs) by their hash using V2 API.
    pub(crate) async fn get_libraries(&self, hash: &str) -> anyhow::Result<String> {
        let url = format!(
            "{}/getLibraries",
            self.base_url.replace("/api/v3", "/api/v2")
        );

        let mut request = self
            .client
            .get(url)
            .header(USER_AGENT, user_agent())
            .query(&[("libraries", hash)]);

        if let Some(key) = &self.api_key {
            request = request.header("X-API-Key", key);
        }

        self.maybe_wait_for_rate_limit().await;
        let response = request.send().await?;
        if !response.status().is_success() {
            anyhow::bail!("TonCenter V2 returned status: {}", response.status());
        }

        let result: serde_json::Value = response.json().await?;

        if let Some(error) = result.get("error") {
            anyhow::bail!("TonCenter V2 error: {error}");
        }

        let result = result
            .get("result")
            .and_then(|v| v.get("result"))
            .and_then(|v| v.as_array());
        let lib_data = result
            .and_then(|arr| arr.first())
            .and_then(|v| v.get("data"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Library not found"))?;

        Ok(lib_data.to_string())
    }

    /// Fetches all blockchain config parameters for a masterchain block using V2 API.
    pub(crate) async fn get_config_all(&self, seqno: u32) -> anyhow::Result<Cell> {
        let url = format!(
            "{}/getConfigAll",
            self.base_url.replace("/api/v3", "/api/v2")
        );

        let mut request = self
            .client
            .get(url)
            .header(USER_AGENT, user_agent())
            .query(&[("seqno", seqno.to_string())]);

        if let Some(key) = &self.api_key {
            request = request.header("X-API-Key", key);
        }

        self.maybe_wait_for_rate_limit().await;
        let response = request.send().await?;
        if !response.status().is_success() {
            anyhow::bail!("TonCenter V2 returned status: {}", response.status());
        }

        #[derive(Deserialize)]
        struct TonCenterConfigAllResponse {
            ok: bool,
            result: Option<TonCenterConfigInfo>,
            error: Option<String>,
        }

        #[derive(Deserialize)]
        struct TonCenterConfigInfo {
            config: TonCenterConfigCell,
        }

        #[derive(Deserialize)]
        struct TonCenterConfigCell {
            bytes: String,
        }

        let data: TonCenterConfigAllResponse = response
            .json()
            .await
            .context("Failed to parse TonCenter getConfigAll response")?;

        if !data.ok {
            anyhow::bail!(
                "{}",
                data.error
                    .unwrap_or_else(|| "TonCenter returned ok=false for getConfigAll".into())
            );
        }

        let config_boc = data
            .result
            .ok_or_else(|| anyhow::anyhow!("TonCenter getConfigAll response has no result"))?
            .config
            .bytes;

        Boc::decode_base64(&config_boc).context("Failed to decode blockchain config BOC data")
    }

    /// Fetches a serialized `ShardAccount` cell using V2 API.
    pub(crate) async fn get_shard_account_cell(
        &self,
        seqno: u32,
        address: &str,
    ) -> anyhow::Result<Cell> {
        let url = format!(
            "{}/getShardAccountCell",
            self.base_url.replace("/api/v3", "/api/v2")
        );
        let query = [
            ("address", address.to_owned()),
            ("seqno", seqno.to_string()),
        ];

        let mut request = self
            .client
            .get(url)
            .header(USER_AGENT, user_agent())
            .query(&query);

        if let Some(key) = &self.api_key {
            request = request.header("X-API-Key", key);
        }

        self.maybe_wait_for_rate_limit().await;
        let response = request.send().await?;
        if !response.status().is_success() {
            anyhow::bail!("TonCenter V2 returned status: {}", response.status());
        }

        #[derive(Deserialize)]
        struct TonCenterShardAccountCellResponse {
            ok: bool,
            result: Option<TonCenterTvmCell>,
            error: Option<String>,
        }

        #[derive(Deserialize)]
        struct TonCenterTvmCell {
            bytes: String,
        }

        let data: TonCenterShardAccountCellResponse = response
            .json()
            .await
            .context("Failed to parse getShardAccountCell response")?;

        if !data.ok {
            anyhow::bail!(
                "{}",
                data.error
                    .unwrap_or_else(|| "TonCenter returned ok=false for getShardAccountCell".into())
            );
        }

        let cell_boc = data
            .result
            .ok_or_else(|| anyhow::anyhow!("TonCenter getShardAccountCell response has no result"))?
            .bytes;

        Boc::decode_base64(&cell_boc).context("Failed to decode shard account cell BOC data")
    }
}

/// Client for `TonHub` (TON API v4).
///
/// Used for fetching account transaction `BoCs` and master-block metadata.
pub(crate) struct TonHubClient {
    client: Client,
    base_url: String,
}

impl TonHubClient {
    /// Creates a new `TonHub` client for the specified network.
    pub(crate) fn new(network: Network) -> anyhow::Result<Self> {
        let base_url = match network {
            Network::Mainnet => "https://mainnet-v4.tonhubapi.com".to_string(),
            Network::Testnet => "https://testnet-v4.tonhubapi.com".to_string(),
            Network::Localnet | Network::Custom(_) => {
                anyhow::bail!("Network {network} is not yet supported in retrace")
            }
        };
        Ok(Self {
            client: http_client_builder().build()?,
            base_url,
        })
    }

    /// Fetches full transaction details including `BoC` and blocks for a specific account/lt/hash.
    pub(crate) async fn get_account_transactions(
        &self,
        address: &str,
        lt: u64,
        hash: &str,
    ) -> anyhow::Result<TransactionTransactionsResponse> {
        let url = format!("{}/account/{}/tx/{}/{}", self.base_url, address, lt, hash);
        let response = self
            .client
            .get(url)
            .header(USER_AGENT, user_agent())
            .send()
            .await?;
        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            anyhow::bail!("TonHub API error {status}: {text}");
        }
        let response_data: TransactionTransactionsResponse = serde_json::from_str(&text)
            .map_err(|e| anyhow::anyhow!("Failed to decode TonHub response: {e}. Body: {text}"))?;
        Ok(response_data)
    }

    /// Fetches master-block information by sequence number.
    pub(crate) async fn get_block(&self, seqno: u32) -> anyhow::Result<BlockInfo> {
        let url = format!("{}/block/{}", self.base_url, seqno);

        #[derive(Deserialize)]
        struct BlockResponse {
            exist: bool,
            block: Option<BlockInfo>,
        }

        let response = self
            .client
            .get(url)
            .header(USER_AGENT, user_agent())
            .send()
            .await?;
        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            anyhow::bail!("TonHub API error {status}: {text}");
        }
        let response_data: BlockResponse = serde_json::from_str(&text)
            .map_err(|e| anyhow::anyhow!("Failed to decode TonHub response: {e}. Body: {text}"))?;

        if !response_data.exist {
            anyhow::bail!("Block {seqno} is out of scope");
        }
        response_data
            .block
            .ok_or_else(|| anyhow::anyhow!("Block info is missing in response"))
    }
}
