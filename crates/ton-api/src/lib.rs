use anyhow::{Context, anyhow};
use num_bigint::{BigInt, ToBigInt};
use reqwest::blocking::Response;
use serde::Deserialize;
use std::collections::HashMap;
pub use ton_networks::{CustomNetworkUrls, Network};
use tycho_types::boc::Boc;
use tycho_types::cell::Cell;

pub struct TonApiClient {
    client: reqwest::blocking::Client,
    network: Network,
    api_key: Option<String>,
    custom_networks: HashMap<String, CustomNetworkUrls>,
}

impl TonApiClient {
    pub fn new(
        network: Network,
        custom_networks: HashMap<String, CustomNetworkUrls>,
        api_key: Option<String>,
    ) -> anyhow::Result<TonApiClient> {
        Ok(TonApiClient {
            client: reqwest::blocking::ClientBuilder::new()
                .build()
                .context("Cannot create HTTP client, please check if network is available")?,
            network,
            api_key,
            custom_networks,
        })
    }

    #[must_use]
    pub fn with_network(mut self, network: Network) -> Self {
        self.network = network;
        self
    }

    #[must_use]
    pub fn with_api_key(mut self, api_key: String) -> Self {
        self.api_key = Some(api_key);
        self
    }

    fn build_request(&self, url: &str) -> reqwest::blocking::RequestBuilder {
        let mut request = self.client.get(url).header("User-Agent", "acton-cli");

        if let Some(ref key) = self.api_key {
            request = request.header("X-API-Key", key);
        }

        request
    }

    fn build_post_request(&self, url: &str) -> reqwest::blocking::RequestBuilder {
        let mut request = self.client.post(url).header("User-Agent", "acton-cli");

        if let Some(ref key) = self.api_key {
            request = request.header("X-API-Key", key);
        }

        request
    }

    #[must_use]
    pub fn network(&self) -> Network {
        self.network.clone()
    }

    /// Get account state from `TonCenter`
    pub fn get_account_state(&self, address: &str) -> anyhow::Result<AccountState> {
        let accounts = self.get_account_states(&[address])?;
        accounts
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("Account not found"))
    }

    /// Get multiple account states from `TonCenter`
    pub fn get_account_states(&self, addresses: &[&str]) -> anyhow::Result<Vec<AccountState>> {
        if addresses.is_empty() {
            return Ok(vec![]);
        }

        let mut url = format!(
            "{}/accountStates?",
            self.network.toncenter_v3_url(&self.custom_networks)?
        );
        for (i, address) in addresses.iter().enumerate() {
            if i > 0 {
                url.push('&');
            }
            url.push_str("address=");
            url.push_str(&urlencoding::encode(address));
        }

        let response = self
            .build_request(&url)
            .send()
            .context("Failed to send request to TonCenter")?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "TonCenter API returned status: {}",
                response.status()
            ));
        }

        #[derive(Deserialize)]
        struct TonCenterResponse {
            accounts: Vec<AccountState>,
        }

        let data: TonCenterResponse = response
            .json()
            .context("Failed to parse TonCenter response")?;

        Ok(data.accounts)
    }

    /// Get contract BOC from `TonCenter` (tries mainnet first, then testnet)
    pub fn get_contract_boc(&self, address: &str) -> anyhow::Result<String> {
        let state = self.get_account_state(address)?;

        if state.status != "active" {
            return Err(anyhow!("Contract is not active (status: {})", state.status));
        }

        state
            .code_boc
            .ok_or_else(|| anyhow!("Contract has no code"))
    }

    /// Run get method on contract
    pub fn run_get_method(
        &self,
        address: &str,
        method: &str,
        stack: &[serde_json::Value],
    ) -> anyhow::Result<GetMethodResult> {
        let url = format!(
            "{}/jsonRPC",
            self.network.toncenter_v2_url(&self.custom_networks)?
        );

        let json = serde_json::json!({
            "id": "1",
            "jsonrpc": "2.0",
            "method": "runGetMethod",
            "params": {
                "address": address,
                "method": method,
                "stack": stack
            }
        });

        let response = self
            .build_post_request(&url)
            .json(&json)
            .send()
            .context("Failed to send runGetMethod request")?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Run get method failed: {error_text}");
        }

        #[derive(Deserialize)]
        struct JsonRpcResponse {
            result: GetMethodResult,
        }

        let result: JsonRpcResponse = response
            .json()
            .context("Failed to parse runGetMethod response")?;

        Ok(result.result)
    }

    /// Get wallet seqno
    pub fn get_wallet_seqno(&self, address: &str) -> anyhow::Result<(u32, bool)> {
        let result = self.run_get_method(address, "seqno", &[])?;

        if result.exit_code == -13 {
            // likely uninit wallet
            return Ok((0, true));
        }

        if let Some(first) = result.stack.first()
            && first.len() == 2
            && let (StackItem::Str(type_str), StackItem::Str(value_str)) = (&first[0], &first[1])
            && type_str == "num"
        {
            let seqno = u32::from_str_radix(value_str.trim_start_matches("0x"), 16)?;
            return Ok((seqno, false));
        }

        Ok((0, false))
    }

    /// Send BOC to network
    pub fn send_boc(&self, boc: &str) -> anyhow::Result<()> {
        let url = format!(
            "{}/sendBoc",
            self.network.toncenter_v2_url(&self.custom_networks)?
        );

        let json = serde_json::json!({ "boc": boc });

        let response = self
            .build_post_request(&url)
            .json(&json)
            .send()
            .context("Failed to send BOC")?;

        if !response.status().is_success() {
            return Err(Self::handle_fail(response));
        }

        Ok(())
    }

    pub fn get_last_block_seqno(&self) -> anyhow::Result<u64> {
        let url = format!(
            "{}/getMasterchainInfo",
            self.network.toncenter_v2_url(&self.custom_networks)?
        );

        let response = self
            .build_request(&url)
            .send()
            .context("Failed to send request to TonCenter")?;

        if !response.status().is_success() {
            return Err(Self::handle_fail(response));
        }

        #[derive(Deserialize)]
        struct TonCenterMasterchainInfoResponse {
            pub result: TonCenterMasterchainInfoResult,
        }

        #[derive(Deserialize)]
        struct TonCenterMasterchainInfoResult {
            pub last: TonCenterMasterchainInfoLastBlock,
        }

        #[derive(Deserialize)]
        struct TonCenterMasterchainInfoLastBlock {
            pub seqno: u64,
        }

        let data: TonCenterMasterchainInfoResponse = response
            .json()
            .context("Failed to parse TonCenter response")?;

        Ok(data.result.last.seqno)
    }

    pub fn get_account_info(
        &self,
        seqno: Option<u64>,
        address: &str,
    ) -> anyhow::Result<TonCenterAccountInfoResult> {
        let url = format!(
            "{}/getAddressInformation?address={}{}",
            self.network.toncenter_v2_url(&self.custom_networks)?,
            urlencoding::encode(address),
            seqno
                .map(|seqno| format!("&seqno={seqno}"))
                .unwrap_or_default(),
        );

        let response = self
            .build_request(&url)
            .send()
            .context("Failed to send request to TonCenter")?;

        if !response.status().is_success() {
            return Err(Self::handle_fail(response));
        }

        #[derive(Deserialize, Debug)]
        struct TonCenterAccountInfoResponse {
            pub result: TonCenterAccountInfoResult,
        }

        let data: TonCenterAccountInfoResponse = response
            .json()
            .context("Failed to parse TonCenter response")?;

        Ok(data.result)
    }

    pub fn get_library_by_hash(&self, hash: &str) -> anyhow::Result<Cell> {
        let url = format!(
            "{}/getLibraries",
            self.network.toncenter_v2_url(&self.custom_networks)?,
        );

        let response = self
            .build_request(&url)
            .query(&[("libraries", hash)])
            .send()
            .context("Failed to send request to TonCenter for library")?;

        if !response.status().is_success() {
            return Err(Self::handle_fail(response));
        }

        #[derive(Deserialize)]
        struct TonCenterLibrariesResponse {
            ok: bool,
            result: TonCenterLibrariesResult,
        }

        #[derive(Deserialize)]
        struct TonCenterLibrariesResult {
            result: Vec<TonCenterLibraryData>,
        }

        #[derive(Deserialize)]
        struct TonCenterLibraryData {
            data: String,
        }

        let data: TonCenterLibrariesResponse = response
            .json()
            .context("Failed to parse TonCenter libraries response")?;

        if !data.ok || data.result.result.is_empty() {
            anyhow::bail!("Library with hash {hash} not found");
        }

        Boc::decode_base64(&data.result.result[0].data).context("Failed to decode library BOC data")
    }

    pub fn decode_optional_cell(cell_data: &String) -> anyhow::Result<Option<Cell>> {
        if cell_data.is_empty() {
            return Ok(None);
        }
        Ok(Some(Boc::decode_base64(cell_data)?))
    }

    pub fn get_transactions(
        &self,
        address: &str,
        limit: Option<u32>,
        lt: Option<String>,
        hash: Option<String>,
    ) -> anyhow::Result<Vec<TonCenterTransaction>> {
        let url = format!(
            "{}/getTransactions",
            self.network.toncenter_v2_url(&self.custom_networks)?
        );

        let mut params = vec![("address", address.to_string())];
        if let Some(limit) = limit {
            params.push(("limit", limit.to_string()));
        }
        if let Some(lt) = lt {
            params.push(("lt", lt));
        }
        if let Some(hash) = hash {
            params.push(("hash", hash));
        }

        let response = self
            .build_request(&url)
            .query(&params)
            .send()
            .context("Failed to send getTransactions request")?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "TonCenter API returned status: {}",
                response.status()
            ));
        }

        #[derive(Deserialize)]
        struct TonCenterTransactionsResponse {
            result: Vec<TonCenterTransaction>,
        }

        let data: TonCenterTransactionsResponse = response
            .json()
            .context("Failed to parse getTransactions response")?;

        Ok(data.result)
    }

    pub fn get_address_balance(&self, address: &str) -> anyhow::Result<BigInt> {
        let url = format!(
            "{}/getAddressBalance?address={}",
            self.network.toncenter_v2_url(&self.custom_networks)?,
            urlencoding::encode(address)
        );

        let response = self
            .build_request(&url)
            .send()
            .context("Failed to send getAddressBalance request")?;

        if !response.status().is_success() {
            return Err(Self::handle_fail(response));
        }

        #[derive(Deserialize)]
        struct TonCenterBalanceResponse {
            ok: bool,
            result: String,
        }

        let data: TonCenterBalanceResponse = response
            .json()
            .context("Failed to parse getAddressBalance response")?;

        if !data.ok {
            anyhow::bail!("TonCenter returned ok=false for getAddressBalance");
        }

        data.result.parse::<BigInt>().map_err(Into::into)
    }

    fn handle_fail(response: Response) -> anyhow::Error {
        let status = response.status();
        let Ok(data) = response.json::<TonCenterErrorResponse>() else {
            return anyhow!("TonCenter API returned status: {status}");
        };

        anyhow!(
            data.error
                .trim_start_matches("LITE_SERVER_UNKNOWN: ")
                .to_owned()
        )
    }
}

#[derive(Deserialize, Clone)]
pub struct AccountState {
    pub address: String,
    pub balance: Option<String>,
    pub code_boc: Option<String>,
    pub status: String,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum StackItem {
    Str(String),
    Obj(serde_json::Value),
}

#[derive(Deserialize, Debug)]
pub struct GetMethodResult {
    pub stack: Vec<Vec<StackItem>>,
    pub exit_code: i32,
}

#[derive(Deserialize, Debug)]
pub struct TonCenterAccountInfoResult {
    pub balance: StringOrNumber,
    pub code: String,
    pub data: String,
    pub state: String,
    pub frozen_hash: String,
    pub last_transaction_id: TonCenterAccountInfoLastTransactionId,
}

#[derive(Deserialize, Debug)]
pub struct TonCenterAccountInfoLastTransactionId {
    pub lt: String,
    pub hash: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum StringOrNumber {
    Str(String),
    Num(i64),
}

impl StringOrNumber {
    pub fn to_bigint(&self) -> anyhow::Result<BigInt> {
        match self {
            StringOrNumber::Str(str) => str.parse::<BigInt>().map_err(Into::into),
            StringOrNumber::Num(num) => num
                .to_bigint()
                .ok_or_else(|| anyhow!("cannot convert {num} to bigint")),
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct TonCenterTransaction {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub utime: u64,
    pub data: String,
    pub transaction_id: TonCenterTransactionId,
    pub fee: String,
    pub storage_fee: String,
    pub other_fee: String,
    pub in_msg: Option<TonCenterMessage>,
    pub out_msgs: Vec<TonCenterMessage>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct TonCenterTransactionId {
    pub lt: String,
    pub hash: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct TonCenterMessage {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub source: Option<String>,
    pub destination: Option<String>,
    pub value: String,
    pub fwd_fee: Option<String>,
    pub ihr_fee: Option<String>,
    pub created_lt: Option<String>,
    pub body_hash: Option<String>,
    pub message: Option<String>,
}

#[derive(Deserialize)]
struct TonCenterErrorResponse {
    #[allow(dead_code)]
    ok: bool,
    error: String,
}
