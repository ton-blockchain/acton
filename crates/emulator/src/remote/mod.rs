use anyhow::{Context, anyhow};
use num_bigint::{BigInt, ToBigInt};
use reqwest::blocking::Response;
use serde::Deserialize;
use tycho_types::boc::Boc;
use tycho_types::cell::Cell;

pub fn get_account_info(
    seqno: Option<u64>,
    address: &str,
    network: &str,
    api_key: Option<String>,
) -> anyhow::Result<TonCenterAccountInfoResult> {
    let base_url = toncenter_url(network)?;
    let url = format!(
        "{}/api/v2/getAddressInformation?address={}{}",
        base_url,
        urlencoding::encode(address),
        seqno
            .map(|seqno| format!("&seqno={seqno}"))
            .unwrap_or("".to_owned()),
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
        return Err(handle_fail(response));
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

pub fn get_library_by_hash(
    network: &str,
    hash: &str,
    api_key: Option<String>,
) -> anyhow::Result<Cell> {
    let base_url = toncenter_url(network)?;
    let url = format!("{}/api/v2/getLibraries", base_url);

    let client = reqwest::blocking::Client::new();
    let mut request = client.get(&url).header("User-Agent", "acton-cli");

    if let Some(key) = api_key {
        request = request.header("X-API-Key", key);
    }

    let response = request
        .query(&[("libraries", hash)])
        .send()
        .context("Failed to send request to TonCenter for library")?;

    if !response.status().is_success() {
        return Err(handle_fail(response));
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

#[derive(Deserialize)]
struct TonCenterErrorResponse {
    #[allow(dead_code)]
    ok: bool,
    error: String,
}
