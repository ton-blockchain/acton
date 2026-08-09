use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::Args;
use reqwest::blocking::Client;
use serde::Deserialize;
use tycho_types::boc::Boc;

const DEFAULT_CONFIG_PATH: &str = "crates/ton-executor/src/default_config.boc64";
const TONCENTER_GET_CONFIG_ALL_URL: &str = "https://toncenter.com/api/v2/getConfigAll";
const HTTP_CONNECT_TIMEOUT_SECS: u64 = 5;
const HTTP_REQUEST_TIMEOUT_SECS: u64 = 20;

#[derive(Args)]
pub(crate) struct UpdateDefaultConfigArgs {
    #[arg(long, default_value = TONCENTER_GET_CONFIG_ALL_URL)]
    pub(crate) url: String,
    #[arg(long, value_name = "PATH", default_value = DEFAULT_CONFIG_PATH)]
    pub(crate) output: PathBuf,
}

#[derive(Deserialize)]
struct TonCenterConfigAllResponse {
    ok: bool,
    result: TonCenterConfigInfo,
}

#[derive(Deserialize)]
struct TonCenterConfigInfo {
    config: TonCenterConfigCell,
}

#[derive(Deserialize)]
struct TonCenterConfigCell {
    bytes: String,
}

pub(crate) fn run(args: UpdateDefaultConfigArgs) -> Result<()> {
    let config_boc64 = fetch_default_config_boc64(&args.url)?;

    if let Ok(existing) = fs::read_to_string(&args.output)
        && existing == config_boc64
    {
        println!(
            "Default config is already up to date: {}",
            args.output.display()
        );
        return Ok(());
    }

    fs::write(&args.output, config_boc64)
        .with_context(|| format!("failed to write `{}`", args.output.display()))?;

    println!(
        "Updated default config from `{}` into `{}`",
        args.url,
        args.output.display()
    );
    Ok(())
}

fn fetch_default_config_boc64(url: &str) -> Result<String> {
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(HTTP_CONNECT_TIMEOUT_SECS))
        .timeout(Duration::from_secs(HTTP_REQUEST_TIMEOUT_SECS))
        .build()
        .context("failed to create TonCenter HTTP client")?;

    let response = client
        .get(url)
        .send()
        .with_context(|| format!("failed to send TonCenter getConfigAll request: {url}"))?;
    let status = response.status();

    if !status.is_success() {
        bail!("TonCenter getConfigAll request failed with status {status}");
    }

    let response: TonCenterConfigAllResponse = response
        .json()
        .context("failed to parse TonCenter getConfigAll response JSON")?;

    if !response.ok {
        bail!("TonCenter returned ok=false for getConfigAll");
    }

    Boc::decode_base64(&response.result.config.bytes)
        .context("TonCenter getConfigAll config bytes are not a valid BOC")?;

    Ok(response.result.config.bytes)
}
