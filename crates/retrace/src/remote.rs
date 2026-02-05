//! Internal HTTP clients for interacting with external TON APIs (`TonCenter`, `TonHub`, DTON).

use crate::Network;
use crate::types::{
    AccountFromAPI, BlockInfo, BlocksResponse, TransactionData, TransactionTransactionsResponse,
};
use reqwest::Client;
use serde::Deserialize;

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
    pub(crate) fn new(network: Network, api_key: Option<String>) -> Self {
        let base_url = match network {
            Network::Mainnet => "https://toncenter.com/api/v3".to_string(),
            Network::Testnet => "https://testnet.toncenter.com/api/v3".to_string(),
            Network::Custom(_) => todo!("Custom networks are not yet supported in retrace"),
        };
        Self {
            client: Client::new(),
            api_key,
            base_url,
        }
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
            .query(&[("hash", hash), ("limit", &limit.to_string())]);

        if let Some(key) = &self.api_key {
            request = request.header("X-API-Key", key);
        }

        let response = request.send().await?;
        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            anyhow::bail!("TonCenter V3 error {status}: {text}");
        }
        let response_data: TransactionData = serde_json::from_str(&text).map_err(|e| {
            anyhow::anyhow!("Failed to decode TonCenter V3 response: {e}. Body: {text}")
        })?;
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
            .query(&[
                ("workchain", workchain.to_string()),
                ("shard", shard.to_string()),
                ("seqno", seqno.to_string()),
            ]);

        if let Some(key) = &self.api_key {
            request = request.header("X-API-Key", key);
        }

        let response = request.send().await?;
        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            anyhow::bail!("TonCenter V3 error {status}: {text}");
        }
        let response_data: BlocksResponse = serde_json::from_str(&text).map_err(|e| {
            anyhow::anyhow!("Failed to decode TonCenter V3 response: {e}. Body: {text}")
        })?;
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

        let mut request = self.client.post(url).json(&body);

        if let Some(key) = &self.api_key {
            request = request.header("X-API-Key", key);
        }

        let response: serde_json::Value = request.send().await?.json().await?;

        if let Some(error) = response.get("error") {
            anyhow::bail!("TonCenter V2 error: {error}");
        }

        let result = response.get("result").and_then(|v| v.as_array()).cloned();
        Ok(result.unwrap_or_default())
    }

    /// Fetches library cells (T-libs) by their hash using V2 API.
    pub(crate) async fn get_libraries(&self, hash: &str) -> anyhow::Result<String> {
        let url = format!(
            "{}/getLibraries",
            self.base_url.replace("/api/v3", "/api/v2")
        );

        let mut request = self.client.get(url).query(&[("libraries", hash)]);

        if let Some(key) = &self.api_key {
            request = request.header("X-API-Key", key);
        }

        let response: serde_json::Value = request.send().await?.json().await?;

        if let Some(error) = response.get("error") {
            anyhow::bail!("TonCenter V2 error: {error}");
        }

        let result = response
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
}

/// Client for `TonHub` (TON API v4).
///
/// Used for fetching account snapshots and master-block configurations.
pub(crate) struct TonHubClient {
    client: Client,
    base_url: String,
}

impl TonHubClient {
    /// Creates a new `TonHub` client for the specified network.
    pub(crate) fn new(network: Network) -> Self {
        let base_url = match network {
            Network::Mainnet => "https://mainnet-v4.tonhubapi.com".to_string(),
            Network::Testnet => "https://testnet-v4.tonhubapi.com".to_string(),
            Network::Custom(_) => todo!("Custom networks are not yet supported in retrace"),
        };
        Self {
            client: Client::new(),
            base_url,
        }
    }

    /// Fetches full transaction details including `BoC` and blocks for a specific account/lt/hash.
    pub(crate) async fn get_account_transactions(
        &self,
        address: &str,
        lt: u64,
        hash: &str,
    ) -> anyhow::Result<TransactionTransactionsResponse> {
        let url = format!("{}/account/{}/tx/{}/{}", self.base_url, address, lt, hash);
        let response = self.client.get(url).send().await?;
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

        let response = self.client.get(url).send().await?;
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

    /// Fetches an account snapshot at a specific master-block sequence number.
    pub(crate) async fn get_account(
        &self,
        seqno: u32,
        address: &str,
    ) -> anyhow::Result<AccountFromAPI> {
        let url = format!("{}/block/{}/{}", self.base_url, seqno, address);

        #[derive(Deserialize)]
        struct AccountResponse {
            account: AccountFromAPI,
        }

        let response = self.client.get(url).send().await?;
        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            anyhow::bail!("TonHub API error {status}: {text}");
        }
        let response_data: AccountResponse = serde_json::from_str(&text)
            .map_err(|e| anyhow::anyhow!("Failed to decode TonHub response: {e}. Body: {text}"))?;
        Ok(response_data.account)
    }

    /// Fetches the global configuration for a specific master-block sequence number.
    pub(crate) async fn get_config(&self, seqno: u32) -> anyhow::Result<String> {
        let url = format!("{}/block/{}/config", self.base_url, seqno);

        #[derive(Deserialize)]
        struct ConfigResponse {
            config: ConfigData,
        }

        #[derive(Deserialize)]
        struct ConfigData {
            cell: String,
        }

        let response = self.client.get(url).send().await?;
        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            anyhow::bail!("TonHub API error {status}: {text}");
        }
        let response_data: ConfigResponse = serde_json::from_str(&text)
            .map_err(|e| anyhow::anyhow!("Failed to decode TonHub response: {e}. Body: {text}"))?;
        Ok(response_data.config.cell)
    }
}

/// Client for dton.io GraphQL API.
///
/// Primarily used as a fallback for fetching library cells.
pub(crate) struct DtonClient {
    client: Client,
    api_key: String,
}

impl DtonClient {
    /// Creates a new Dton client with an optional API key.
    pub(crate) fn new(api_key: Option<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.unwrap_or_else(|| "fpYxhGTWfIe3ZEf2s6vvgAGmps_qnNmD".to_string()),
        }
    }

    /// Fetches a library cell by its hash via GraphQL.
    pub(crate) async fn get_lib(&self, network: Network, hash: &str) -> anyhow::Result<String> {
        let endpoint = match network {
            Network::Mainnet => format!("https://dton.io/{}/graphql", self.api_key),
            Network::Testnet => format!("https://testnet.dton.io/{}/graphql", self.api_key),
            Network::Custom(_) => anyhow::bail!("Custom networks are not yet supported in retrace"),
        };

        let query = serde_json::json!({
            "query": format!("query fetchAuthor {{ get_lib(lib_hash: \"{}\") }}", hash),
            "variables": {}
        });

        let response: serde_json::Value = self
            .client
            .post(endpoint)
            .json(&query)
            .send()
            .await?
            .json()
            .await?;

        let lib_data = response
            .get("data")
            .and_then(|v| v.get("get_lib"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Library not found on DTON"))?;

        Ok(lib_data.to_string())
    }
}
