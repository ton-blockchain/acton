use anyhow::{Context, anyhow};
use num_bigint::{BigInt, ToBigInt};
use serde::Deserialize;
use tycho_types::boc::Boc;
use tycho_types::cell::Cell;

pub fn get_last_block_seqno(network: &str, api_key: Option<String>) -> anyhow::Result<u64> {
    let base_url = toncenter_url(network)?;
    let url = format!("{}/api/v2/getMasterchainInfo", base_url);
    let client = reqwest::blocking::Client::new();
    let mut request = client.get(url).header("User-Agent", "acton-cli");

    if let Some(key) = api_key {
        request = request.header("X-API-Key", key);
    }

    let response = request
        .send()
        .context("Failed to send request to TonCenter")?;

    if !response.status().is_success() {
        anyhow::bail!("TonCenter API returned status: {}", response.status());
    }

    let data: TonCenterMasterchainInfoResponse = response
        .json()
        .context("Failed to parse TonCenter response")?;

    Ok(data.result.last.seqno)
}

pub fn get_account_info(
    seqno: u64,
    address: &String,
    network: &str,
    api_key: Option<String>,
) -> anyhow::Result<TonCenterAccountInfoResult> {
    let base_url = toncenter_url(network)?;
    let url = format!(
        "{}/api/v2/getAddressInformation?address={}&seqno={seqno}",
        base_url,
        urlencoding::encode(address)
    );
    let client = reqwest::blocking::Client::new();
    let mut request = client.get(url).header("User-Agent", "acton-cli");

    if let Some(key) = api_key {
        request = request.header("X-API-Key", key);
    }

    let response = request
        .send()
        .context("Failed to send request to TonCenter")?;

    if !response.status().is_success() {
        anyhow::bail!("TonCenter API returned status: {}", response.status());
    }

    let data: TonCenterAccountInfoResponse = response
        .json()
        .context("Failed to parse TonCenter response")?;

    Ok(data.result)
}

fn toncenter_url(network: &str) -> anyhow::Result<&str> {
    let base_url = match network {
        "mainnet" => "https://toncenter.com",
        "testnet" => "https://testnet.toncenter.com",
        _ => anyhow::bail!(
            "Unsupported network: {}. Supported networks: mainnet, testnet",
            network
        ),
    };
    Ok(base_url)
}

pub fn decode_optional_cell(cell_data: &String) -> anyhow::Result<Option<Cell>> {
    if cell_data.is_empty() {
        return Ok(None);
    }
    Ok(Some(Boc::decode_base64(cell_data)?))
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

#[derive(Deserialize, Debug)]
struct TonCenterAccountInfoResponse {
    pub result: TonCenterAccountInfoResult,
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
