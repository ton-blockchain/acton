use anyhow::{Context, anyhow};
use num_bigint::{BigInt, ToBigInt};
use reqwest::blocking::Response;
use serde::Deserialize;
use tycho_types::boc::Boc;
use tycho_types::cell::Cell;

#[derive(Debug, Clone)]
pub enum Network {
    Mainnet,
    Testnet,
}

impl Network {
    pub fn as_str(&self) -> &'static str {
        match self {
            Network::Mainnet => "mainnet",
            Network::Testnet => "testnet",
        }
    }

    pub fn from_str(s: &str) -> anyhow::Result<Self> {
        match s.to_lowercase().as_str() {
            "mainnet" => Ok(Network::Mainnet),
            "testnet" => Ok(Network::Testnet),
            _ => anyhow::bail!("Unsupported network: {}. Supported: mainnet, testnet", s),
        }
    }

    pub fn toncenter_url(&self) -> &'static str {
        match self {
            Network::Mainnet => "https://toncenter.com",
            Network::Testnet => "https://testnet.toncenter.com",
        }
    }
}

pub struct TonApiClient {
    client: reqwest::blocking::Client,
    network: Network,
    api_key: Option<String>,
}

impl TonApiClient {
    pub fn new(network: Network, api_key: Option<String>) -> Self {
        Self {
            client: reqwest::blocking::Client::new(),
            network,
            api_key,
        }
    }

    pub fn with_network(mut self, network: Network) -> Self {
        self.network = network;
        self
    }

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

    pub fn network(&self) -> Network {
        self.network.clone()
    }

    /// Get account state from TonCenter
    pub fn get_account_state(&self, address: &str) -> anyhow::Result<AccountState> {
        let url = format!(
            "{}/api/v3/accountStates?address={}",
            self.network.toncenter_url(),
            urlencoding::encode(address)
        );

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

        if data.accounts.is_empty() {
            return Err(anyhow!("Account not found"));
        }

        Ok(data.accounts[0].clone())
    }

    /// Get contract BOC from TonCenter (tries mainnet first, then testnet)
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
        stack: Vec<serde_json::Value>,
    ) -> anyhow::Result<GetMethodResult> {
        let url = format!("{}/api/v2/jsonRPC", self.network.toncenter_url());

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
            return Err(anyhow!(
                "TonCenter API returned status: {}",
                response.status()
            ));
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
        let result = self.run_get_method(address, "seqno", vec![])?;

        if result.exit_code == -13 {
            // likely uninit wallet
            return Ok((0, true));
        }

        if let Some(first) = result.stack.first() {
            if first.len() == 2 {
                if let (StackItem::Str(type_str), StackItem::Str(value_str)) =
                    (&first[0], &first[1])
                {
                    if type_str == "num" {
                        let seqno = u32::from_str_radix(value_str.trim_start_matches("0x"), 16)?;
                        return Ok((seqno, false));
                    }
                }
            }
        }

        Ok((0, false))
    }

    /// Send BOC to network
    pub fn send_boc(&self, boc: &str) -> anyhow::Result<()> {
        let url = format!("{}/api/v2/sendBoc", self.network.toncenter_url());

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
        let url = format!("{}/api/v2/getMasterchainInfo", self.network.toncenter_url());

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
        address: &String,
    ) -> anyhow::Result<TonCenterAccountInfoResult> {
        let url = format!(
            "{}/api/v2/getAddressInformation?address={}{}",
            self.network.toncenter_url(),
            urlencoding::encode(address),
            seqno
                .map(|seqno| format!("&seqno={seqno}"))
                .unwrap_or("".to_owned()),
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
        let url = format!("{}/api/v2/getLibraries", self.network.toncenter_url(),);

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
            anyhow::bail!("Library with hash {} not found", hash);
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
        let url = format!("{}/api/v2/getTransactions", self.network.toncenter_url());

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
            "{}/api/v2/getAddressBalance?address={}",
            self.network.toncenter_url(),
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
        let data = match response.json::<TonCenterErrorResponse>() {
            Ok(res) => res,
            Err(_) => {
                return anyhow!("TonCenter API returned status: {status}");
            }
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
