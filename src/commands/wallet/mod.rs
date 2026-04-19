use crate::commands::common::{create_symlink, error_fmt, select_wallet};
use crate::wallets;
use acton_config::color::OwoColorize;
use acton_config::config;
use acton_config::config::{
    ActonConfig, WalletsFile, global_wallets_path, project_root as configured_project_root,
};
use anyhow::{Context, anyhow};
use clap::Subcommand;
use inquire::{Confirm, Select, Text};
use log::error;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::io::{IsTerminal, Read, Write, stdin, stdout};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;
use toml_edit::{DocumentMut, Item, Table, value};
use ton::ton_core::cell::TonCell;
use ton::ton_core::traits::tlb::TLB;
use ton::ton_core::types::TonAddress;
use ton::ton_wallet::{Mnemonic, TonWallet, WalletVersion};
use ton_api::{CustomNetworkUrls, Network, TonApiClient};

#[derive(clap::ValueEnum, Debug, Copy, Clone, PartialEq, Eq)]
#[clap(rename_all = "lowercase")]
pub enum WalletVersionArg {
    V1R1,
    V1R2,
    V1R3,
    V2R1,
    V2R2,
    V3R1,
    V3R2,
    V4R1,
    V4R2,
    V5R1,
    HighloadV1R1,
    HighloadV1R2,
    HighloadV2,
    HighloadV2R1,
    HighloadV2R2,
}

#[derive(clap::ValueEnum, Debug, Copy, Clone, PartialEq, Eq)]
#[clap(rename_all = "lowercase")]
pub enum WalletAirdropNetworkArg {
    Testnet,
    Localnet,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum SignMessageFormat {
    Hex,
    Base64,
}

#[derive(serde::Deserialize)]
struct FaucetChallengeResponse {
    challenge: String,
    difficulty: u32,
}

#[derive(serde::Deserialize)]
struct FaucetMessageResponse {
    message: Option<String>,
    error: Option<String>,
}

struct AirdropResult {
    address: String,
    difficulty: Option<u32>,
    nonce: Option<u64>,
    solve_duration: Option<Duration>,
    message: Option<String>,
}

enum AirdropTarget {
    Testnet { faucet_url: String },
    Localnet { port: u16, amount_ton: f64 },
}

impl AirdropTarget {
    const fn network(&self) -> Network {
        match self {
            Self::Testnet { .. } => Network::Testnet,
            Self::Localnet { .. } => Network::Localnet,
        }
    }
}

const HTTP_RETRY_ATTEMPTS: usize = 3;
const HTTP_RETRY_BACKOFF_MS: [u64; 3] = [200, 500, 1000];
const POW_MAX_SOLVE_DURATION: Duration = Duration::from_secs(60);
const POW_MAX_NONCE_ATTEMPTS: u64 = 1_000_000_000;
const DEFAULT_FAUCET_URL: &str = "https://acton.monster/faucet/";
const DEFAULT_LOCALNET_PORT: u16 = 5411;
const LOCALNET_WALLET_AIRDROP_AMOUNT_TON: f64 = 100.0;
const AIRDROP_BALANCE_WAIT_ATTEMPTS: usize = 10;
const AIRDROP_BALANCE_WAIT_INTERVAL: Duration = Duration::from_secs(2);
const TEST_WALLET_KEYRING_SUPPORTED_ENV: &str = "ACTON_TEST_WALLET_KEYRING_SUPPORTED"; // integration tests only
const TEST_TONCENTER_V3_URL_ENV: &str = "ACTON_TEST_TONCENTER_V3_URL"; // integration tests only

impl SignMessageFormat {
    const fn as_str(self) -> &'static str {
        match self {
            SignMessageFormat::Hex => "hex",
            SignMessageFormat::Base64 => "base64",
        }
    }
}

impl From<WalletVersionArg> for WalletVersion {
    fn from(arg: WalletVersionArg) -> Self {
        match arg {
            WalletVersionArg::V1R1 => WalletVersion::V1R1,
            WalletVersionArg::V1R2 => WalletVersion::V1R2,
            WalletVersionArg::V1R3 => WalletVersion::V1R3,
            WalletVersionArg::V2R1 => WalletVersion::V2R1,
            WalletVersionArg::V2R2 => WalletVersion::V2R2,
            WalletVersionArg::V3R1 => WalletVersion::V3R1,
            WalletVersionArg::V3R2 => WalletVersion::V3R2,
            WalletVersionArg::V4R1 => WalletVersion::V4R1,
            WalletVersionArg::V4R2 => WalletVersion::V4R2,
            WalletVersionArg::V5R1 => WalletVersion::V5R1,
            WalletVersionArg::HighloadV1R1 => WalletVersion::HLV1R1,
            WalletVersionArg::HighloadV1R2 => WalletVersion::HLV1R2,
            WalletVersionArg::HighloadV2 => WalletVersion::HLV2,
            WalletVersionArg::HighloadV2R1 => WalletVersion::HLV2R1,
            WalletVersionArg::HighloadV2R2 => WalletVersion::HLV2R2,
        }
    }
}

#[derive(Subcommand)]
pub enum WalletCommand {
    #[command(about = "Generate a new testnet wallet")]
    New {
        #[arg(long, help = "Name of the wallet (prompts if not provided)")]
        name: Option<String>,
        #[arg(long, help = "Version of the wallet (prompts if not provided)")]
        version: Option<WalletVersionArg>,
        #[arg(long, help = "Save wallet to global global.wallets.toml")]
        global: bool,
        #[arg(long, help = "Save wallet to local wallets.toml")]
        local: bool,
        #[arg(
            long,
            help = "Use secure native store for mnemonic (defaults to true if available)",
            default_missing_value = "true",
            num_args = 0..=1
        )]
        secure: Option<bool>,
        #[arg(
            long,
            help = "Request testnet TON from faucet after wallet creation",
            default_value_t = false
        )]
        airdrop: bool,
        #[arg(long, help = "Faucet URL for automatic testnet airdrop")]
        faucet_url: Option<String>,
        #[arg(
            long,
            help = "Do not wait for testnet funds to appear after a successful automatic airdrop",
            default_value_t = false
        )]
        no_wait_airdrop: bool,
        #[arg(long, help = "Output result as JSON")]
        json: bool,
    },
    #[command(about = "Import an existing wallet from mnemonic")]
    Import {
        #[arg(long, help = "Name of the wallet (prompts if not provided)")]
        name: Option<String>,
        #[arg(help = "Mnemonic words of the wallet")]
        mnemonics: Vec<String>,
        #[arg(long, help = "Version of the wallet (prompts if not provided)")]
        version: Option<WalletVersionArg>,
        #[arg(long, help = "Save wallet to global global.wallets.toml")]
        global: bool,
        #[arg(long, help = "Save wallet to local wallets.toml")]
        local: bool,
        #[arg(
            long,
            help = "Use secure native store for mnemonic (defaults to true if available)",
            default_missing_value = "true",
            num_args = 0..=1
        )]
        secure: Option<bool>,
        #[arg(long, help = "Output result as JSON")]
        json: bool,
    },
    #[command(about = "List available wallets")]
    List {
        #[arg(short, long, help = "Show wallet balance")]
        balance: bool,
        #[arg(long, help = "TonCenter API key for blockchain queries")]
        api_key: Option<String>,
        #[arg(long, help = "Output result as JSON")]
        json: bool,
    },
    #[command(about = "Export wallet mnemonic (interactive confirmation required)")]
    ExportMnemonic {
        #[arg(help = "Name of the wallet (prompts if not provided)")]
        name: Option<String>,
    },
    #[command(about = "Sign a message with wallet private key")]
    Sign {
        #[arg(help = "Name of the wallet (prompts if not provided)")]
        name: Option<String>,
        #[arg(
            long,
            alias = "message",
            help = "External body BoC to sign in hex or base64. If omitted, reads from stdin when piped or prompts interactively"
        )]
        body: Option<String>,
        #[arg(long, help = "Output result as JSON")]
        json: bool,
    },
    #[command(about = "Remove wallet")]
    Remove {
        #[arg(help = "Name of the wallet (prompts if not provided)")]
        name: Option<String>,
        #[arg(short = 'y', long, help = "Skip confirmation prompt")]
        yes: bool,
        #[arg(long, help = "Output result as JSON")]
        json: bool,
    },
    #[command(about = "Request TON coins from testnet or localnet faucet")]
    Airdrop {
        #[arg(help = "Name of the wallet (prompts if not provided)")]
        name: Option<String>,
        #[arg(
            long,
            value_enum,
            default_value = "testnet",
            help = "Airdrop network backend"
        )]
        net: WalletAirdropNetworkArg,
        #[arg(long, help = "Faucet URL for testnet airdrop backend")]
        faucet_url: Option<String>,
        #[arg(
            long,
            help = "Do not wait for testnet funds to appear after a successful testnet airdrop",
            default_value_t = false
        )]
        no_wait_airdrop: bool,
        #[arg(long, help = "Output result as JSON")]
        json: bool,
    },
}

pub fn wallet_cmd(command: WalletCommand) -> anyhow::Result<()> {
    match command {
        WalletCommand::New {
            name,
            version,
            global,
            local,
            secure,
            airdrop,
            faucet_url,
            no_wait_airdrop,
            json,
        } => new_wallet(
            name,
            version,
            global,
            local,
            secure,
            airdrop,
            faucet_url,
            no_wait_airdrop,
            json,
        ),
        WalletCommand::Import {
            name,
            mnemonics,
            version,
            global,
            local,
            secure,
            json,
        } => import_wallet(name, mnemonics, version, global, local, secure, json),
        WalletCommand::List {
            balance,
            api_key,
            json,
        } => list_wallets(balance, api_key, json),
        WalletCommand::ExportMnemonic { name } => export_mnemonic(name),
        WalletCommand::Sign { name, body, json } => sign_wallet_external_body(name, body, json),
        WalletCommand::Remove { name, yes, json } => remove_wallet(name, yes, json),
        WalletCommand::Airdrop {
            name,
            net,
            faucet_url,
            no_wait_airdrop,
            json,
        } => airdrop_wallet(name, net, faucet_url, no_wait_airdrop, json),
    }
}

fn airdrop_wallet(
    name: Option<String>,
    net: WalletAirdropNetworkArg,
    faucet_url: Option<String>,
    no_wait_airdrop: bool,
    json: bool,
) -> anyhow::Result<()> {
    let target = resolve_airdrop_target(net, faucet_url)?;
    let wait_for_balance = matches!(&target, AirdropTarget::Testnet { .. });
    let run_result = perform_airdrop(name, target, json);

    match run_result {
        Ok(result) => {
            let message = result.message.as_deref().unwrap_or("Success");
            if json {
                let mut payload = serde_json::json!({
                    "success": true,
                    "message": message,
                    "address": result.address,
                });
                if let Some(difficulty) = result.difficulty {
                    payload["difficulty"] = serde_json::json!(difficulty);
                }
                if let Some(nonce) = result.nonce {
                    payload["nonce"] = serde_json::json!(nonce);
                }
                if let Some(solve_duration) = result.solve_duration {
                    payload["solve_ms"] = serde_json::json!(solve_duration.as_millis());
                }
                println!("{}", serde_json::to_string(&payload)?);
            } else {
                println!("{} {}", "✓".green(), message);
                if wait_for_balance {
                    maybe_wait_for_testnet_airdrop_balance(&result.address, no_wait_airdrop);
                }
            }
            Ok(())
        }
        Err(err) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string(&serde_json::json!({
                        "success": false,
                        "error": err.to_string()
                    }))?
                );
            }
            Err(err)
        }
    }
}

fn perform_airdrop(
    name: Option<String>,
    target: AirdropTarget,
    json: bool,
) -> anyhow::Result<AirdropResult> {
    let config = ActonConfig::load()?;

    let name = select_wallet(name, &config)?;

    let wallet = config
        .get_wallet(&name)
        .ok_or_else(|| anyhow!(error_fmt::wallet_not_found(&config, &name)))?;

    let address = get_wallet_address(&name, wallet, target.network())?;

    if !json {
        println!(
            "{} Requesting airdrop for wallet {} {}",
            "→".blue().bold(),
            name.cyan().bold(),
            address
        );
    }

    match target {
        AirdropTarget::Testnet { faucet_url } => perform_testnet_airdrop(address, faucet_url, json),
        AirdropTarget::Localnet { port, amount_ton } => {
            perform_localnet_airdrop(address, amount_ton, port)
        }
    }
}

fn resolve_airdrop_target(
    net: WalletAirdropNetworkArg,
    faucet_url: Option<String>,
) -> anyhow::Result<AirdropTarget> {
    match net {
        WalletAirdropNetworkArg::Testnet => Ok(AirdropTarget::Testnet {
            faucet_url: faucet_url.unwrap_or_else(|| DEFAULT_FAUCET_URL.to_owned()),
        }),
        WalletAirdropNetworkArg::Localnet => {
            if faucet_url.is_some() {
                anyhow::bail!("--faucet-url can only be used with --net testnet");
            }
            let port = ActonConfig::load()
                .ok()
                .and_then(|config| config.localnet)
                .and_then(|localnet| localnet.port)
                .unwrap_or(DEFAULT_LOCALNET_PORT);

            Ok(AirdropTarget::Localnet {
                port,
                amount_ton: LOCALNET_WALLET_AIRDROP_AMOUNT_TON,
            })
        }
    }
}

fn perform_testnet_airdrop(
    address: String,
    faucet_url: String,
    json: bool,
) -> anyhow::Result<AirdropResult> {
    let faucet_base = parse_faucet_base_url(&faucet_url)?;
    let challenge_url = faucet_base.join("challenge").with_context(|| {
        format!("Failed to build challenge URL from faucet base URL {faucet_base}")
    })?;
    let claim_url = faucet_base
        .join("claim")
        .with_context(|| format!("Failed to build claim URL from faucet base URL {faucet_base}"))?;

    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .build()
        .context("Failed to build HTTP client")?;

    // Faucet for testnet TON uses Proof-of-Work so we need to solve it to get coins

    // 1. Get challenge
    if !json {
        println!("{} Fetching PoW challenge...", "→".blue().bold());
    }
    let challenge_res = send_with_retry(
        || client.get(challenge_url.clone()).send(),
        "challenge",
        "Failed to get challenge from faucet",
        json,
    )?;

    if !challenge_res.status().is_success() {
        let status = challenge_res.status();
        let body = challenge_res.text().unwrap_or_default();
        if body.is_empty() {
            anyhow::bail!("Failed to get challenge: status {status}");
        }
        anyhow::bail!("Failed to get challenge: status {status}: {body}");
    }

    let challenge_data: FaucetChallengeResponse = challenge_res
        .json()
        .context("Failed to parse challenge response")?;
    if challenge_data.difficulty > 256 {
        anyhow::bail!(
            "Invalid PoW difficulty from faucet: {} (max 256)",
            challenge_data.difficulty
        );
    }

    // 2. Solve challenge
    if !json {
        println!(
            "{} Solving challenge (difficulty: {} bits)...",
            "→".blue().bold(),
            challenge_data.difficulty
        );
    }
    let start = std::time::Instant::now();
    let nonce = solve_challenge(&challenge_data.challenge, challenge_data.difficulty)?;
    let duration = start.elapsed();
    if !json {
        println!("{} Challenge solved in {:?}", "✓".green(), duration);
    }

    // 3. Send claim
    let claim_payload = serde_json::json!({
        "address": address,
        "challenge": challenge_data.challenge,
        "nonce": nonce
    });
    let response = send_with_retry(
        || client.post(claim_url.clone()).json(&claim_payload).send(),
        "claim",
        "Failed to send request to faucet",
        json,
    )?;

    if response.status().is_success() {
        let res: FaucetMessageResponse =
            response.json().context("Failed to parse faucet response")?;
        Ok(AirdropResult {
            address,
            difficulty: Some(challenge_data.difficulty),
            nonce: Some(nonce),
            solve_duration: Some(duration),
            message: res.message,
        })
    } else {
        let status = response.status();
        let error_msg = if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            "Too many requests from your IP. Only 2 requests per 24 hours are allowed. Please try again later.".to_string()
        } else {
            let body_text = response.text().unwrap_or_default();
            if let Ok(res) = serde_json::from_str::<FaucetMessageResponse>(&body_text) {
                res.error
                    .or(res.message)
                    .unwrap_or_else(|| body_text.clone())
            } else {
                body_text
            }
        };

        anyhow::bail!("Faucet returned error {status}: {error_msg}");
    }
}

fn perform_localnet_airdrop(
    address: String,
    amount_ton: f64,
    port: u16,
) -> anyhow::Result<AirdropResult> {
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
        .context("Failed to build HTTP client")?;
    let amount_nanotons = (amount_ton * 1_000_000_000.0) as u128;

    let response = client
        .post(format!("http://localhost:{port}/admin/faucet"))
        .json(&serde_json::json!({
            "address": address,
            "amount": amount_nanotons,
        }))
        .send()
        .context(
            "Failed to send request to localnet faucet. Make sure `acton localnet start` is running",
        )?;

    if response.status().is_success() {
        let json: serde_json::Value = response
            .json()
            .context("Failed to parse localnet faucet response")?;
        if json
            .get("ok")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
            || json
                .get("success")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        {
            let message = format!("Successfully airdropped {amount_ton} TON on localnet");
            Ok(AirdropResult {
                address,
                difficulty: None,
                nonce: None,
                solve_duration: None,
                message: Some(message),
            })
        } else {
            let error = json
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error");
            anyhow::bail!("Localnet faucet returned error: {error}");
        }
    } else {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        anyhow::bail!("Localnet faucet returned error {status}: {body}");
    }
}

fn send_with_retry<F>(
    mut send_request: F,
    request_name: &str,
    transport_error_context: &str,
    json: bool,
) -> anyhow::Result<reqwest::blocking::Response>
where
    F: FnMut() -> Result<reqwest::blocking::Response, reqwest::Error>,
{
    for attempt in 0..HTTP_RETRY_ATTEMPTS {
        return match send_request() {
            Ok(response) => {
                if response.status().is_server_error() && attempt + 1 < HTTP_RETRY_ATTEMPTS {
                    if !json {
                        println!(
                            "{} {} request returned {}. Retrying ({}/{})...",
                            "↻".yellow().bold(),
                            request_name,
                            response.status(),
                            attempt + 2,
                            HTTP_RETRY_ATTEMPTS
                        );
                    }
                    std::thread::sleep(http_retry_backoff(attempt));
                    continue;
                }
                Ok(response)
            }
            Err(err) => {
                if attempt + 1 < HTTP_RETRY_ATTEMPTS {
                    if !json {
                        println!(
                            "{} {} request failed: {}. Retrying ({}/{})...",
                            "↻".yellow().bold(),
                            request_name,
                            err,
                            attempt + 2,
                            HTTP_RETRY_ATTEMPTS
                        );
                    }
                    std::thread::sleep(http_retry_backoff(attempt));
                    continue;
                }
                Err(err).context(transport_error_context.to_owned())
            }
        };
    }

    unreachable!("retry loop must return on success or final failure")
}

fn create_testnet_ton_api_client(api_key: Option<String>) -> anyhow::Result<TonApiClient> {
    let config = ActonConfig::load().unwrap_or_default();
    let mut custom_networks = config.custom_networks();
    let api_key = api_key.or_else(|| env::var("TONCENTER_API_KEY").ok());
    let toncenter_v3_override = env::var(TEST_TONCENTER_V3_URL_ENV).ok();

    if let Some(url) = toncenter_v3_override {
        // test only code
        let network_name = "__wallet_list_testnet_override".to_string();
        let normalized = Arc::<str>::from(url.trim_end_matches('/').to_owned());
        custom_networks.insert(
            network_name.clone(),
            CustomNetworkUrls {
                v2_url: Arc::clone(&normalized),
                v3_url: Some(normalized),
                explorer_url: None,
            },
        );
        TonApiClient::new(
            Network::Custom(Arc::from(network_name)),
            custom_networks,
            api_key,
        )
    } else {
        TonApiClient::new(Network::Testnet, custom_networks, api_key)
    }
}

fn http_retry_backoff(attempt: usize) -> Duration {
    let index = attempt.min(HTTP_RETRY_BACKOFF_MS.len() - 1);
    Duration::from_millis(HTTP_RETRY_BACKOFF_MS[index])
}

fn parse_faucet_base_url(faucet_url: &str) -> anyhow::Result<reqwest::Url> {
    let faucet_url = faucet_url.trim();
    if faucet_url.is_empty() {
        anyhow::bail!("Faucet URL cannot be empty");
    }

    let mut normalized = faucet_url.to_owned();
    if !normalized.ends_with('/') {
        normalized.push('/');
    }

    let url = reqwest::Url::parse(&normalized)
        .with_context(|| format!("Invalid faucet URL: {faucet_url}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        anyhow::bail!("Faucet URL scheme must be http or https");
    }
    if url.query().is_some() || url.fragment().is_some() {
        anyhow::bail!("Faucet URL must not contain query parameters or fragments");
    }
    Ok(url)
}

fn solve_challenge(challenge: &str, difficulty: u32) -> anyhow::Result<u64> {
    solve_challenge_with_limits(
        challenge,
        difficulty,
        POW_MAX_SOLVE_DURATION,
        POW_MAX_NONCE_ATTEMPTS,
    )
}

fn solve_challenge_with_limits(
    challenge: &str,
    difficulty: u32,
    max_duration: Duration,
    max_nonce_attempts: u64,
) -> anyhow::Result<u64> {
    if difficulty > 256 {
        anyhow::bail!("PoW difficulty must be at most 256 bits");
    }

    let started_at = std::time::Instant::now();
    let mut nonce: u64 = 0;
    loop {
        if started_at.elapsed() >= max_duration {
            anyhow::bail!(
                "PoW solve exceeded time limit of {}s",
                max_duration.as_secs()
            );
        }
        if nonce >= max_nonce_attempts {
            anyhow::bail!("PoW solve exceeded nonce limit of {max_nonce_attempts}");
        }

        let mut hasher = Sha256::new();
        hasher.update(challenge.as_bytes());
        hasher.update(nonce.to_be_bytes());
        let result = hasher.finalize();

        let mut zero_bits = 0;
        for &byte in &result {
            let leading_zeros = byte.leading_zeros();
            zero_bits += leading_zeros;
            if leading_zeros < 8 {
                break;
            }
        }
        if zero_bits >= difficulty {
            return Ok(nonce);
        }

        nonce = nonce
            .checked_add(1)
            .context("PoW nonce overflow while solving challenge")?;
    }
}

fn export_mnemonic(name: Option<String>) -> anyhow::Result<()> {
    let config = ActonConfig::load()?;

    let name = select_wallet(name, &config)?;

    if !stdin().is_terminal() || !stdout().is_terminal() {
        anyhow::bail!(
            "Exporting mnemonic is only allowed in interactive mode for security reasons."
        );
    }

    let confirmation = Text::new("Type wallet name to confirm mnemonic export:")
        .prompt()
        .context("Failed to read confirmation")?;
    if confirmation.trim() != name {
        anyhow::bail!("Confirmation failed: wallet name does not match");
    }

    let wallet = config
        .get_wallet(&name)
        .ok_or_else(|| anyhow!(error_fmt::wallet_not_found(&config, &name)))?;

    let mnemonic = wallets::load_mnemonic(&name, wallet)?;

    println!("{mnemonic}");

    Ok(())
}

fn sign_wallet_external_body(
    name: Option<String>,
    body: Option<String>,
    json: bool,
) -> anyhow::Result<()> {
    let config = ActonConfig::load()?;

    let name = select_wallet(name, &config)?;

    let wallet = config
        .get_wallet(&name)
        .ok_or_else(|| anyhow!(error_fmt::wallet_not_found(&config, &name)))?;

    let body = read_sign_body(body)?;
    let (external_body, input) = decode_sign_input(&body)?;

    let mnemonic_str = wallets::load_mnemonic(&name, wallet)?;
    let mnemonic = Mnemonic::from_str(&mnemonic_str, None)?;
    let key_pair = mnemonic.to_key_pair()?;
    let version = parse_wallet_version(&wallet.kind)?;
    let wallet_id = wallets::wallet_id(version, &Network::Testnet);
    let ton_wallet =
        TonWallet::new_with_params(version, key_pair, wallet.workchain.unwrap_or(0), wallet_id)?;

    let signed_body = ton_wallet
        .sign_ext_in_body(&external_body)
        .context("Failed to sign external body")?;
    let signed_body_hex = signed_body
        .to_boc_hex()
        .context("Failed to encode signed body to hex BoC")?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "success": true,
                "wallet": name,
                "input": input.as_str(),
                "output": "hex",
                "signed_body": signed_body_hex,
            }))?
        );
    } else {
        println!("{signed_body_hex}");
    }

    Ok(())
}

fn read_sign_body(body: Option<String>) -> anyhow::Result<String> {
    if let Some(body) = body {
        return Ok(body);
    }

    if !stdin().is_terminal() {
        let mut input = stdin();
        return read_sign_body_from_reader(&mut input).context("Failed to read body from stdin");
    }

    Text::new("External body BoC (hex/base64) to sign:")
        .prompt()
        .context("Failed to read body")
}

fn read_sign_body_from_reader(reader: &mut impl Read) -> anyhow::Result<String> {
    let mut body = String::new();
    reader.read_to_string(&mut body)?;
    Ok(body)
}

fn decode_sign_input(body: &str) -> anyhow::Result<(TonCell, SignMessageFormat)> {
    let body = body.trim();
    if body.is_empty() {
        anyhow::bail!("Body cannot be empty");
    }

    if is_hex_payload(body)
        && let Ok(cell) = TonCell::from_boc_hex(body)
    {
        return Ok((cell, SignMessageFormat::Hex));
    }

    if let Ok(cell) = TonCell::from_boc_base64(body) {
        return Ok((cell, SignMessageFormat::Base64));
    }

    anyhow::bail!("Body must be a valid BoC encoded as hex or base64")
}

fn is_hex_payload(value: &str) -> bool {
    value.len().is_multiple_of(2) && value.as_bytes().iter().all(u8::is_ascii_hexdigit)
}

fn format_testnet_wallet_address(address: &TonAddress) -> String {
    address.to_base64(false, true, true)
}

fn remove_wallet(name: Option<String>, yes: bool, json: bool) -> anyhow::Result<()> {
    let config = ActonConfig::load()?;

    let name = select_wallet(name, &config)?;

    let wallet = config
        .get_wallet(&name)
        .ok_or_else(|| anyhow!(error_fmt::wallet_not_found(&config, &name)))?;

    let remove_confirmed = confirm_wallet_removal(&name, yes, json)?;
    if !remove_confirmed {
        return Ok(());
    }

    let is_global = remove_wallet_with_merged_precedence(&name, &config)?;

    let keyring_mnemonic_removed = if let Some(keyring_id) = wallet.keys.mnemonic_keyring.as_deref()
    {
        wallets::delete_mnemonic_from_keyring(keyring_id, &name)?;
        true
    } else {
        false
    };

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "success": true,
                "name": name,
                "is_global": is_global,
                "keyring_mnemonic_removed": keyring_mnemonic_removed,
            }))?
        );
    } else {
        let config_label = if is_global {
            "global.wallets.toml"
        } else {
            "wallets.toml"
        };
        if keyring_mnemonic_removed {
            println!(
                "{} Wallet mnemonic removed from system keyring",
                "✓".green()
            );
        }
        println!(
            "{} Wallet {} removed from {}",
            "✓".green(),
            name.cyan().bold(),
            config_label.cyan()
        );
    }

    Ok(())
}

fn confirm_wallet_removal(name: &str, yes: bool, json: bool) -> anyhow::Result<bool> {
    if yes {
        return Ok(true);
    }

    if !stdin().is_terminal() || !stdout().is_terminal() {
        anyhow::bail!(
            "Confirmation required to remove wallet {}. This action cannot be undone.\nRe-run with -y/--yes in non-interactive mode.",
            name.yellow()
        );
    }

    let confirmed = Confirm::new(&format!(
        "Remove wallet '{name}'? This action cannot be undone."
    ))
    .with_default(false)
    .prompt()?;

    if !confirmed {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "success": false,
                    "cancelled": true,
                    "message": "Wallet removal cancelled by user"
                }))?
            );
        } else {
            println!("{}", "Wallet removal cancelled.".yellow());
        }
    }

    Ok(confirmed)
}

fn remove_wallet_with_merged_precedence(name: &str, config: &ActonConfig) -> anyhow::Result<bool> {
    // global first, then local override.
    // So removal should target local first (effective winner), then global.
    let local_path = configured_project_root().join("wallets.toml");
    if remove_wallet_from_config_file(&local_path, name)? {
        return Ok(false);
    }

    if let Some(global_path) = global_wallets_path()
        && remove_wallet_from_config_file(&global_path, name)?
    {
        return Ok(true);
    }

    anyhow::bail!(error_fmt::wallet_not_found(config, name));
}

fn list_wallets(balance: bool, api_key: Option<String>, json: bool) -> anyhow::Result<()> {
    let config = ActonConfig::load()?;

    let mut wallets_info = Vec::new();

    let global_path = global_wallets_path();
    let global_wallets: HashSet<String> = if let Some(path) = &global_path
        && path.exists()
    {
        let content = fs::read_to_string(path)?;
        let wallets: WalletsFile = toml::from_str(&content)?;
        wallets
            .wallets
            .map(|w| w.wallets.keys().cloned().collect())
            .unwrap_or_default()
    } else {
        Default::default()
    };

    let wallets = config
        .wallets()
        .ok_or_else(|| anyhow!(error_fmt::no_wallets_found()))?;

    if wallets.is_empty() {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "success": true,
                    "wallets": []
                }))?
            );
        } else {
            println!("No wallets found");
        }
        return Ok(());
    }

    let client = if balance {
        Some(create_testnet_ton_api_client(api_key)?)
    } else {
        None
    };

    if !json {
        println!("Available wallets:");
    }

    let mut wallets_data = Vec::new();
    for (name, wallet_config) in wallets {
        let Ok(address) = get_wallet_address(name, wallet_config, Network::Testnet) else {
            error!("cannot get wallet address for {name}"); // very unlikely
            continue;
        };
        wallets_data.push((name, wallet_config, address));
    }

    let mut balances = HashMap::new();
    if balance {
        let addresses: Vec<&str> = wallets_data
            .iter()
            .map(|(_, _, addr)| addr.as_str())
            .collect();
        let client = client
            .as_ref()
            .expect("TonApiClient must be initialized when --balance is set");
        match client.get_account_states(&addresses) {
            Ok(states) => {
                for state in states {
                    if let Some(b) = state.balance
                        && let Ok(b_int) = b.parse::<i128>()
                        && let Ok(address) = TonAddress::from_str(&state.address)
                    {
                        balances.insert(format_testnet_wallet_address(&address), b_int);
                    }
                }
            }
            Err(e) => {
                error!("failed to fetch balances: {e}");
            }
        }
    }

    for (name, wallet_config, address) in wallets_data {
        let is_global = global_wallets.contains(name);
        let mut balance_info = String::new();
        let mut balance_val = None;

        if balance {
            if let Some(b) = balances.get(&address) {
                let balance_ton = *b as f64 / 1_000_000_000.0;
                balance_val = Some(*b);
                balance_info = format!("— {}", format!("{balance_ton:.4} TON").green());
            } else {
                balance_val = Some(0.into());
                balance_info = format!("— {}", "0 TON".dimmed());
            }
        }

        if json {
            wallets_info.push(serde_json::json!({
                "name": name,
                "address": address,
                "kind": wallet_config.kind,
                "is_global": is_global,
                "balance": balance_val,
            }));
        } else {
            println!(
                "  {} {} {} {} {balance_info}",
                name.cyan().bold(),
                address,
                format!("({})", wallet_config.kind).dimmed(),
                if is_global {
                    "[global]".blue().to_string()
                } else {
                    "[local]".yellow().to_string()
                },
            );
        }
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "success": true,
                "wallets": wallets_info
            }))?
        );
    }

    Ok(())
}

fn get_wallet_address(
    wallet_name: &str,
    wallet: &config::WalletConfig,
    network: Network,
) -> anyhow::Result<String> {
    if let Some(expected) = &wallet.expected
        && let Some(addr) = &expected.address_testnet
    {
        let addr = TonAddress::from_str(addr)?;
        return Ok(format_testnet_wallet_address(&addr));
    }

    let mnemonic_str = wallets::load_mnemonic(wallet_name, wallet)?;

    let mnemonic = Mnemonic::from_str(&mnemonic_str, None)?;
    let version = parse_wallet_version(&wallet.kind)?;
    let wallet_id = wallets::wallet_id(version, &network);
    let ton_wallet = TonWallet::new_with_params(
        version,
        mnemonic.to_key_pair()?,
        wallet.workchain.unwrap_or(0),
        wallet_id,
    )?;
    Ok(format_testnet_wallet_address(&ton_wallet.address))
}

fn remove_wallet_from_config_file(config_path: &Path, name: &str) -> anyhow::Result<bool> {
    if !config_path.exists() {
        return Ok(false);
    }

    let content = fs::read_to_string(config_path)
        .with_context(|| format!("Failed to read {}", config_path.display()))?;
    let mut doc = content
        .parse::<DocumentMut>()
        .with_context(|| format!("Failed to parse {} as TOML", config_path.display()))?;

    let Some(wallets_item) = doc.get_mut("wallets") else {
        return Ok(false);
    };
    let wallets = wallets_item
        .as_table_mut()
        .context("wallets is not a table")?;

    if wallets.remove(name).is_none() {
        return Ok(false);
    }

    if wallets.is_empty() {
        doc.remove("wallets");
    }

    fs::write(config_path, doc.to_string())
        .with_context(|| format!("Failed to write to {}", config_path.display()))?;

    Ok(true)
}

fn wallet_version_to_string(v: &WalletVersion) -> String {
    match v {
        WalletVersion::V1R1 => "v1r1",
        WalletVersion::V1R2 => "v1r2",
        WalletVersion::V1R3 => "v1r3",
        WalletVersion::V2R1 => "v2r1",
        WalletVersion::V2R2 => "v2r2",
        WalletVersion::V3R1 => "v3r1",
        WalletVersion::V3R2 => "v3r2",
        WalletVersion::V4R1 => "v4r1",
        WalletVersion::V4R2 => "v4r2",
        WalletVersion::V5R1 => "v5r1",
        WalletVersion::HLV1R1 => "highloadv1r1",
        WalletVersion::HLV1R2 => "highloadv1r2",
        WalletVersion::HLV2 => "highloadv2",
        WalletVersion::HLV2R1 => "highloadv2r1",
        WalletVersion::HLV2R2 => "highloadv2r2",
    }
    .to_string()
}

fn get_or_prompt_name(name: Option<String>) -> anyhow::Result<String> {
    match name {
        Some(n) => {
            let normalized = normalize_wallet_name(&n);
            if normalized.is_empty() {
                anyhow::bail!("Wallet name '{n}' is invalid");
            }
            Ok(normalized)
        }
        None => loop {
            let n = Text::new("Wallet name:").with_default("wallet").prompt()?;
            let normalized = normalize_wallet_name(&n);
            if !normalized.is_empty() {
                break Ok(normalized);
            }
            println!(
                "{}",
                "Wallet name is invalid. Please try again.".yellow().bold()
            );
        },
    }
}

fn get_is_global(global_flag: bool, local_flag: bool) -> anyhow::Result<bool> {
    if global_flag && local_flag {
        anyhow::bail!(
            "Cannot use both {} and {} flags",
            "--global".yellow(),
            "--local".yellow()
        );
    }

    if global_flag {
        Ok(true)
    } else if local_flag {
        Ok(false)
    } else {
        let options = vec![
            "Local (wallets.toml)",
            "Global (~/.config/acton/wallets/global.wallets.toml)",
        ];
        let selection = Select::new("Save wallet to:", options).prompt()?;
        Ok(selection.starts_with("Global"))
    }
}

const fn should_prompt_auto_airdrop(
    airdrop_flag: bool,
    json: bool,
    stdin_is_tty: bool,
    stdout_is_tty: bool,
) -> bool {
    !airdrop_flag && !json && stdin_is_tty && stdout_is_tty
}

fn resolve_auto_airdrop(airdrop_flag: bool, json: bool) -> anyhow::Result<bool> {
    if airdrop_flag {
        return Ok(true);
    }

    if !should_prompt_auto_airdrop(
        airdrop_flag,
        json,
        stdin().is_terminal(),
        stdout().is_terminal(),
    ) {
        return Ok(false);
    }

    Confirm::new("Request testnet TON from faucet now?")
        .with_default(false)
        .prompt()
        .context("Failed to read auto-airdrop confirmation")
}

const fn should_wait_for_testnet_airdrop_balance(
    no_wait_airdrop: bool,
    stdin_is_tty: bool,
    stdout_is_tty: bool,
) -> bool {
    !no_wait_airdrop && stdin_is_tty && stdout_is_tty
}

fn spawn_wait_skip_listener() -> mpsc::Receiver<()> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let _ = stdin().read_line(&mut line);
        let _ = tx.send(());
    });
    rx
}

fn fetch_testnet_account_balance(
    client: &TonApiClient,
    address: &str,
) -> anyhow::Result<Option<i128>> {
    let Some(state) = client.get_account_states(&[address])?.into_iter().next() else {
        return Ok(None);
    };

    let Some(balance) = state.balance else {
        return Ok(Some(0));
    };

    let balance = balance
        .parse::<i128>()
        .with_context(|| format!("Failed to parse testnet balance `{balance}`"))?;
    Ok(Some(balance))
}

fn maybe_wait_for_testnet_airdrop_balance(address: &str, no_wait_airdrop: bool) {
    if !should_wait_for_testnet_airdrop_balance(
        no_wait_airdrop,
        stdin().is_terminal(),
        stdout().is_terminal(),
    ) {
        return;
    }

    let client = match create_testnet_ton_api_client(None) {
        Ok(client) => client,
        Err(err) => {
            println!(
                "{} Faucet accepted the request, but balance confirmation could not start: {}",
                "Warning:".yellow().bold(),
                err
            );
            println!(
                "  Check later with {}.",
                "acton wallet list --balance".yellow()
            );
            return;
        }
    };

    println!(
        "{} Waiting for testnet funds to appear... Press Enter to skip waiting.",
        "→".blue().bold()
    );
    let _ = stdout().flush();

    let skip_rx = spawn_wait_skip_listener();
    let mut last_error = None::<String>;

    for attempt in 0..AIRDROP_BALANCE_WAIT_ATTEMPTS {
        if matches!(skip_rx.try_recv(), Ok(())) {
            println!(
                "{} Skipping wait. You can check later with {}.",
                "→".blue().bold(),
                "acton wallet list --balance".yellow()
            );
            return;
        }

        match fetch_testnet_account_balance(&client, address) {
            Ok(Some(balance)) if balance > 0 => {
                let balance_ton = balance as f64 / 1_000_000_000.0;
                println!(
                    "{} Testnet funds are available: {}",
                    "✓".green(),
                    format!("{balance_ton:.4} TON").green()
                );
                return;
            }
            Ok(_) => {}
            Err(err) => last_error = Some(err.to_string()),
        }

        if attempt + 1 < AIRDROP_BALANCE_WAIT_ATTEMPTS
            && matches!(skip_rx.recv_timeout(AIRDROP_BALANCE_WAIT_INTERVAL), Ok(()))
        {
            println!(
                "{} Skipping wait. You can check later with {}.",
                "→".blue().bold(),
                "acton wallet list --balance".yellow()
            );
            return;
        }
    }

    if let Some(err) = last_error {
        println!(
            "{} Faucet accepted the request, but balance confirmation failed: {}",
            "Warning:".yellow().bold(),
            err
        );
    } else {
        println!(
            "{} Faucet accepted the request, but funds are not visible yet.",
            "Warning:".yellow().bold()
        );
    }
    println!(
        "  Check later with {}.",
        "acton wallet list --balance".yellow()
    );
}

fn get_config_path(name: &str, is_global: bool) -> anyhow::Result<PathBuf> {
    if is_global {
        let global_dir = global_wallets_path()
            .ok_or_else(|| anyhow!("Could not determine global wallets path"))?
            .parent()
            .ok_or_else(|| anyhow!("Invalid global wallets path"))?
            .to_path_buf();

        fs::create_dir_all(&global_dir)?;

        let config_path = global_dir.join("global.wallets.toml");

        if config_path.exists() {
            let content = fs::read_to_string(&config_path)?;
            let wallets: WalletsFile = toml::from_str(&content)?;
            if let Some(w) = wallets.wallets
                && w.wallets.contains_key(name)
            {
                anyhow::bail!("Wallet {} already exists in global config", name.yellow());
            }
        }

        Ok(config_path)
    } else {
        let config_path = configured_project_root().join("wallets.toml");
        if config_path.exists() {
            let content = fs::read_to_string(&config_path)?;
            let wallets: WalletsFile = toml::from_str(&content)?;
            if let Some(w) = wallets.wallets
                && w.wallets.contains_key(name)
            {
                anyhow::bail!("Wallet {} already exists in local config", name.yellow());
            }
        }

        Ok(config_path)
    }
}

fn get_or_prompt_version(version: Option<WalletVersionArg>) -> anyhow::Result<WalletVersion> {
    if let Some(v) = version {
        Ok(v.into())
    } else {
        let versions = [
            WalletVersion::V5R1,
            WalletVersion::V4R2,
            WalletVersion::V3R2,
            WalletVersion::V3R1,
            WalletVersion::V2R2,
            WalletVersion::V2R1,
            WalletVersion::V1R3,
            WalletVersion::V1R2,
            WalletVersion::V1R1,
            WalletVersion::HLV2R2,
            WalletVersion::HLV2R1,
            WalletVersion::HLV2,
            WalletVersion::HLV1R2,
            WalletVersion::HLV1R1,
        ];

        let versions_str: Vec<String> = versions.iter().map(wallet_version_to_string).collect();
        let selected_str = Select::new("Wallet type:", versions_str)
            .with_starting_cursor(0)
            .prompt()?;
        parse_wallet_version(&selected_str)
    }
}

fn save_wallet_to_config(
    config_path: &Path,
    name: &str,
    version: WalletVersion,
    mnemonic_str: Option<String>,
    mnemonic_keyring: Option<String>,
    wallet_address: &str,
    is_global: bool,
) -> anyhow::Result<()> {
    let mut doc = if config_path.exists() {
        let content = fs::read_to_string(config_path)
            .with_context(|| format!("Failed to read {}", config_path.display()))?;
        content
            .parse::<DocumentMut>()
            .with_context(|| format!("Failed to parse {} as TOML", config_path.display()))?
    } else {
        DocumentMut::new()
    };

    let wallets = doc
        .entry("wallets")
        .or_insert({
            let mut t = Table::new();
            t.set_implicit(true);
            Item::Table(t)
        })
        .as_table_mut()
        .context("wallets is not a table")?;

    let wallet = wallets
        .entry(name)
        .or_insert(Item::Table(Table::new()))
        .as_table_mut()
        .with_context(|| format!("wallets.{name} is not a table"))?;

    wallet["kind"] = value(wallet_version_to_string(&version));
    wallet["workchain"] = value(0i64);

    let mut keys = toml_edit::InlineTable::new();
    if let Some(m) = mnemonic_str {
        keys.insert("mnemonic", m.into());
    }
    if let Some(k) = mnemonic_keyring {
        keys.insert("mnemonic-keyring", k.into());
    }
    wallet["keys"] = value(keys);

    let expected = wallet
        .entry("expected")
        .or_insert(Item::Table(Table::new()))
        .as_table_mut()
        .with_context(|| format!("wallets.{name}.expected is not a table"))?;

    expected["address-testnet"] = value(wallet_address);

    fs::write(config_path, doc.to_string())
        .with_context(|| format!("Failed to write to {}", config_path.display()))?;

    if is_global {
        let symlink_path = configured_project_root().join("global.wallets.toml");
        if !symlink_path.exists() {
            if let Err(e) = create_symlink(config_path, &symlink_path) {
                println!(
                    "  {} Failed to create symlink: {}",
                    "Warning:".yellow().bold(),
                    e
                );
            } else {
                println!(
                    "{} Created symlink global.wallets.toml -> {}",
                    "✓".green(),
                    config_path.display()
                );
            }
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn new_wallet(
    name: Option<String>,
    version: Option<WalletVersionArg>,
    global_flag: bool,
    local_flag: bool,
    secure: Option<bool>,
    airdrop: bool,
    faucet_url: Option<String>,
    no_wait_airdrop: bool,
    json: bool,
) -> anyhow::Result<()> {
    let name = get_or_prompt_name(name)?;
    let is_global = get_is_global(global_flag, local_flag)?;
    let config_path = get_config_path(&name, is_global)?;
    let version = get_or_prompt_version(version)?;

    let mnemonic_words = wallets::new_mnemonic()?;
    let mnemonic_str = mnemonic_words.join(" ");

    let mnemonic = Mnemonic::from_str(&mnemonic_str, None)?;
    let key_pair = mnemonic.to_key_pair()?;

    let wallet_id = wallets::wallet_id(version, &Network::Testnet);
    let wallet = TonWallet::new_with_params(version, key_pair, 0, wallet_id)?;

    let wallet_address = format_testnet_wallet_address(&wallet.address);

    let use_secure_store = get_or_prompt_use_keystore(secure)?;

    let (mnemonic_str_opt, mnemonic_keyring_opt) = maybe_store_mnemonic_in_keystore(
        &config_path,
        &name,
        &mnemonic_str,
        use_secure_store,
        is_global,
    )?;

    save_wallet_to_config(
        &config_path,
        &name,
        version,
        mnemonic_str_opt,
        mnemonic_keyring_opt,
        &wallet_address,
        is_global,
    )?;
    let auto_airdrop = resolve_auto_airdrop(airdrop, json)?;
    let airdrop_faucet_url = auto_airdrop.then(|| new_wallet_airdrop_faucet_url(faucet_url));

    if json {
        let mut output = serde_json::json!({
            "success": true,
            "name": name,
            "address": wallet_address,
            "kind": wallet_version_to_string(&version),
            "is_global": is_global,
            "airdrop_requested": auto_airdrop,
        });

        if auto_airdrop {
            let airdrop_output = match perform_airdrop(
                Some(name),
                AirdropTarget::Testnet {
                    faucet_url: airdrop_faucet_url.expect("auto_airdrop implies faucet URL exists"),
                },
                true,
            ) {
                Ok(result) => {
                    let mut airdrop_json = serde_json::json!({
                        "success": true,
                        "message": result.message.as_deref().unwrap_or("Success"),
                        "address": result.address,
                    });
                    if let Some(difficulty) = result.difficulty {
                        airdrop_json["difficulty"] = serde_json::json!(difficulty);
                    }
                    if let Some(nonce) = result.nonce {
                        airdrop_json["nonce"] = serde_json::json!(nonce);
                    }
                    if let Some(solve_duration) = result.solve_duration {
                        airdrop_json["solve_ms"] = serde_json::json!(solve_duration.as_millis());
                    }
                    airdrop_json
                }
                Err(err) => serde_json::json!({
                    "success": false,
                    "error": err.to_string(),
                }),
            };
            output["airdrop"] = airdrop_output;
        }

        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        let config_label = if is_global {
            "global.wallets.toml"
        } else {
            "wallets.toml"
        };
        println!(
            "{} Wallet successfully created and added to {}",
            "✓".green(),
            config_label.cyan(),
        );
        println!("{} Wallet address is {}", "✓".green(), wallet_address);

        if use_secure_store {
            println!(
                "{} The mnemonic is securely stored in your system's keyring",
                "✓".green()
            );
        }

        if auto_airdrop {
            match perform_airdrop(
                Some(name),
                AirdropTarget::Testnet {
                    faucet_url: airdrop_faucet_url.expect("auto_airdrop implies faucet URL exists"),
                },
                false,
            ) {
                Ok(result) => {
                    println!(
                        "{} {}",
                        "✓".green(),
                        result.message.as_deref().unwrap_or("Success")
                    );
                    maybe_wait_for_testnet_airdrop_balance(&result.address, no_wait_airdrop);
                }
                Err(err) => {
                    println!(
                        "{} Wallet was created, but automatic airdrop failed: {}",
                        "Warning:".yellow().bold(),
                        err
                    );
                }
            }
            println!(
                "\n{}",
                "NOTE: This is a testnet wallet. Coins in testnet have NO VALUE.".yellow()
            );
        } else {
            println!(
                "\n{}",
                "NOTE: This is a testnet wallet. Coins in testnet have NO VALUE.".yellow()
            );

            println!(
                "\nTo get testnet coins run {} or check official documentation: {}",
                "acton wallet airdrop".yellow(),
                "https://docs.ton.org/ecosystem/wallet-apps/get-coins#how-to-get-coins-on-testnet"
                    .underline(),
            );
        }

        print_wallet_balance_hint();

        if !use_secure_store {
            show_security_warning(config_path);
        }
    }

    Ok(())
}

fn new_wallet_airdrop_faucet_url(faucet_url: Option<String>) -> String {
    faucet_url.unwrap_or_else(|| DEFAULT_FAUCET_URL.to_owned())
}

fn maybe_store_mnemonic_in_keystore(
    config_path: &Path,
    name: &str,
    mnemonic_str: &str,
    use_secure_store: bool,
    is_global: bool,
) -> anyhow::Result<(Option<String>, Option<String>)> {
    let (mnemonic_str_opt, mnemonic_keyring_opt) = if use_secure_store {
        let keyring_id = keyring_id_for_scope(config_path, is_global)?;
        wallets::store_mnemonic_in_keyring(&keyring_id, name, mnemonic_str)?;
        (None, Some(keyring_id))
    } else {
        (Some(mnemonic_str.to_owned()), None)
    };
    Ok((mnemonic_str_opt, mnemonic_keyring_opt))
}

fn keyring_id_for_scope(config_path: &Path, is_global: bool) -> anyhow::Result<String> {
    if let Some(existing) = existing_keyring_id(config_path)? {
        return Ok(existing);
    }

    if is_global {
        return Ok("global".to_string());
    }

    let project_root = dunce::canonicalize(configured_project_root())
        .unwrap_or_else(|_| configured_project_root().to_path_buf());
    let digest = Sha256::digest(project_root.as_os_str().to_string_lossy().as_bytes());
    Ok(format!("local:{}", hex::encode(&digest[..8])))
}

fn existing_keyring_id(config_path: &Path) -> anyhow::Result<Option<String>> {
    if !config_path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(config_path)
        .with_context(|| format!("Failed to read {}", config_path.display()))?;
    let wallets: WalletsFile = toml::from_str(&content)
        .with_context(|| format!("Failed to parse {} as TOML", config_path.display()))?;

    Ok(wallets.wallets.and_then(|wallets| {
        wallets
            .wallets
            .into_values()
            .find_map(|wallet| wallet.keys.mnemonic_keyring)
    }))
}

fn import_wallet(
    name: Option<String>,
    mnemonics: Vec<String>,
    version: Option<WalletVersionArg>,
    global_flag: bool,
    local_flag: bool,
    secure: Option<bool>,
    json: bool,
) -> anyhow::Result<()> {
    let name = get_or_prompt_name(name)?;
    let is_global = get_is_global(global_flag, local_flag)?;
    let config_path = get_config_path(&name, is_global)?;

    let mnemonic_str = if mnemonics.is_empty() {
        Text::new("Enter mnemonic (24 words):").prompt()?
    } else {
        mnemonics.join(" ")
    };

    let mnemonic =
        Mnemonic::from_str(mnemonic_str.trim(), None).context("Invalid mnemonic phrase")?;
    let key_pair = mnemonic.to_key_pair()?;

    let version = get_or_prompt_version(version)?;

    let wallet_id = wallets::wallet_id(version, &Network::Testnet);
    let wallet = TonWallet::new_with_params(version, key_pair, 0, wallet_id)?;

    let wallet_address = format_testnet_wallet_address(&wallet.address);

    let use_secure_store = get_or_prompt_use_keystore(secure)?;

    let (mnemonic_str_opt, mnemonic_keyring_opt) = maybe_store_mnemonic_in_keystore(
        &config_path,
        &name,
        &mnemonic_str,
        use_secure_store,
        is_global,
    )?;

    save_wallet_to_config(
        &config_path,
        &name,
        version,
        mnemonic_str_opt,
        mnemonic_keyring_opt,
        &wallet_address,
        is_global,
    )?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "success": true,
                "name": name,
                "address": wallet_address,
                "kind": wallet_version_to_string(&version),
                "is_global": is_global,
            }))?
        );
    } else {
        let config_label = if is_global {
            "global.wallets.toml"
        } else {
            "wallets.toml"
        };
        println!(
            "\n{} Wallet successfully created and added to {}",
            "✓".green(),
            config_label.cyan(),
        );
        println!("{} Wallet address is {}", "✓".green(), wallet_address);
        if use_secure_store {
            println!(
                "\n{} The mnemonic is securely stored in your system's keyring.",
                "✓".green()
            );
        }

        print_wallet_balance_hint();

        if !use_secure_store {
            show_security_warning(config_path);
        }
    }
    Ok(())
}

fn print_wallet_balance_hint() {
    println!(
        "\nTo check wallet balances run {}.",
        "acton wallet list --balance".yellow()
    );
}

fn show_security_warning(config_path: PathBuf) {
    println!("\n{}", "SECURITY WARNING:".red());
    println!(
        "- The mnemonic is stored in plain text in {}",
        config_path.display().cyan()
    );
    println!("- Do NOT commit this file to version control (already added to .gitignore)");
    println!("- Keep your mnemonic safe and secret");
}

fn get_or_prompt_use_keystore(secure: Option<bool>) -> anyhow::Result<bool> {
    let use_secure_store = if is_keyring_supported_for_wallet_cmd()? {
        if let Some(s) = secure {
            s
        } else {
            Confirm::new("Store mnemonic in secure native store?")
                .with_default(true)
                .with_help_message("This will store your mnemonic in the system keychain instead of plain text in Acton.toml")
                .prompt()?
        }
    } else {
        if secure == Some(true) {
            anyhow::bail!(
                "Secure native store is not supported or accessible in this environment, but --secure was explicitly requested."
            );
        }
        false
    };
    Ok(use_secure_store)
}

fn is_keyring_supported_for_wallet_cmd() -> anyhow::Result<bool> {
    match env::var(TEST_WALLET_KEYRING_SUPPORTED_ENV) {
        Ok(raw) => {
            let normalized = raw.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "1" => Ok(true),
                "0" => Ok(false),
                _ => anyhow::bail!(
                    "Invalid value for {}: {}. Expected one of: 1, 0",
                    TEST_WALLET_KEYRING_SUPPORTED_ENV.yellow(),
                    raw.yellow()
                ),
            }
        }
        Err(env::VarError::NotPresent) => Ok(wallets::is_keyring_supported()),
        Err(err) => Err(anyhow!(
            "Failed to read {}: {err}",
            TEST_WALLET_KEYRING_SUPPORTED_ENV.yellow()
        )),
    }
}

fn parse_wallet_version(kind: &str) -> anyhow::Result<WalletVersion> {
    match kind.to_lowercase().as_str() {
        "v1r1" => Ok(WalletVersion::V1R1),
        "v1r2" => Ok(WalletVersion::V1R2),
        "v1r3" => Ok(WalletVersion::V1R3),
        "v2r1" => Ok(WalletVersion::V2R1),
        "v2r2" => Ok(WalletVersion::V2R2),
        "v3r1" => Ok(WalletVersion::V3R1),
        "v3r2" => Ok(WalletVersion::V3R2),
        "v4r1" => Ok(WalletVersion::V4R1),
        "v4r2" => Ok(WalletVersion::V4R2),
        "v5r1" => Ok(WalletVersion::V5R1),
        "highloadv1r1" => Ok(WalletVersion::HLV1R1),
        "highloadv1r2" => Ok(WalletVersion::HLV1R2),
        "highloadv2" => Ok(WalletVersion::HLV2),
        "highloadv2r1" => Ok(WalletVersion::HLV2R1),
        "highloadv2r2" => Ok(WalletVersion::HLV2R2),
        _ => Err(anyhow!(
            "Unsupported wallet version {}. Supported versions: v1r1, v1r2, v1r3, v2r1, v2r2, v3r1, v3r2, v4r1, v4r2, v5r1, highloadv1r1, highloadv1r2, highloadv2, highloadv2r1, highloadv2r2",
            kind.yellow()
        )),
    }
}

fn normalize_wallet_name(name: &str) -> String {
    name.trim()
        .to_lowercase()
        .replace(' ', "-")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

#[cfg(test)]
mod wallet_name_tests {
    use super::*;

    #[test]
    fn test_normalize_wallet_name() {
        assert_eq!(normalize_wallet_name("My Wallet"), "my-wallet");
        assert_eq!(normalize_wallet_name("  Trim Me  "), "trim-me");
        assert_eq!(normalize_wallet_name("Keep_Underscore"), "keep_underscore");
        assert_eq!(normalize_wallet_name("Remove!@#$%Symbols"), "removesymbols");
        assert_eq!(
            normalize_wallet_name("Multiple   Spaces"),
            "multiple---spaces"
        );
        assert_eq!(normalize_wallet_name("v5r1"), "v5r1");
        assert_eq!(normalize_wallet_name("!!!"), "");
    }

    #[test]
    fn test_decode_sign_input_hex() {
        let cell = TonCell::empty().clone();
        let body_hex = cell.to_boc_hex().expect("must encode hex boc");
        let (decoded, format) = decode_sign_input(&body_hex).expect("must decode hex");
        assert_eq!(decoded, cell);
        assert_eq!(format, SignMessageFormat::Hex);
    }

    #[test]
    fn test_decode_sign_input_base64() {
        let cell = TonCell::empty().clone();
        let body_b64 = cell.to_boc_base64().expect("must encode base64 boc");
        let (decoded, format) = decode_sign_input(&body_b64).expect("must decode base64");
        assert_eq!(decoded, cell);
        assert_eq!(format, SignMessageFormat::Base64);
    }

    #[test]
    fn test_decode_sign_input_trims_surrounding_whitespace() {
        let cell = TonCell::empty().clone();
        let body_b64 = cell.to_boc_base64().expect("must encode base64 boc");
        let padded = format!(" \n{body_b64}\t");
        let (decoded, format) = decode_sign_input(&padded).expect("must decode trimmed input");
        assert_eq!(decoded, cell);
        assert_eq!(format, SignMessageFormat::Base64);
    }

    #[test]
    fn test_decode_sign_input_empty() {
        let err = decode_sign_input("").expect_err("must fail for empty payload");
        assert!(
            err.to_string().contains("Body cannot be empty"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_decode_sign_input_invalid() {
        let err = decode_sign_input("!@#$%").expect_err("must fail for invalid payload");
        assert!(
            err.to_string()
                .contains("Body must be a valid BoC encoded as hex or base64"),
            "unexpected error: {err}"
        );
    }

    #[allow(clippy::string_lit_as_bytes)]
    #[test]
    fn test_read_sign_body_from_reader() {
        let mut reader = "te6ccgEBAQEAAgAAAA==\n".as_bytes();
        let body = read_sign_body_from_reader(&mut reader).expect("must read piped body");
        assert_eq!(body, "te6ccgEBAQEAAgAAAA==\n");
    }

    #[test]
    fn test_get_wallet_address_uses_mnemonic_when_expected_testnet_missing() {
        let mnemonic_str = "cupboard match uphold miracle fog balance unknown region share hand trophy million toy narrow ability exchange first toast fresh maid report cram strong later";

        let wallet = config::WalletConfig {
            kind: "v5r1".to_string(),
            workchain: Some(0),
            keys: config::WalletKeys {
                mnemonic_env: None,
                mnemonic_file: None,
                mnemonic: Some(mnemonic_str.to_string()),
                mnemonic_keyring: None,
            },
            expected: Some(config::WalletExpectedAddresses {
                // Should be ignored because address-testnet is missing.
                address_mainnet: Some("invalid-mainnet-address".to_string()),
                address_testnet: None,
            }),
        };

        let actual = get_wallet_address("wallet", &wallet, Network::Testnet)
            .expect("must derive testnet address from mnemonic");

        let mnemonic = Mnemonic::from_str(mnemonic_str, None).expect("valid mnemonic");
        let key_pair = mnemonic.to_key_pair().expect("keypair from mnemonic");
        let version = WalletVersion::V5R1;
        let wallet_id = wallets::wallet_id(version, &Network::Testnet);
        let expected_wallet = TonWallet::new_with_params(version, key_pair, 0, wallet_id)
            .expect("wallet from mnemonic");
        let expected = format_testnet_wallet_address(&expected_wallet.address);

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_parse_faucet_base_url_normalizes_trailing_slash() {
        let base = parse_faucet_base_url("https://example.com/faucet").expect("must parse");
        assert_eq!(base.as_str(), "https://example.com/faucet/");
        assert_eq!(
            base.join("challenge").expect("must join").as_str(),
            "https://example.com/faucet/challenge"
        );
    }

    #[test]
    fn test_parse_faucet_base_url_rejects_invalid_scheme() {
        let err = parse_faucet_base_url("ftp://example.com").expect_err("must reject ftp");
        assert!(
            err.to_string()
                .contains("Faucet URL scheme must be http or https"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_parse_faucet_base_url_rejects_query_and_fragment() {
        let query_err =
            parse_faucet_base_url("https://example.com/faucet?token=123").expect_err("must fail");
        assert!(
            query_err
                .to_string()
                .contains("must not contain query parameters or fragments"),
            "unexpected error: {query_err}"
        );

        let fragment_err =
            parse_faucet_base_url("https://example.com/faucet#frag").expect_err("must fail");
        assert!(
            fragment_err
                .to_string()
                .contains("must not contain query parameters or fragments"),
            "unexpected error: {fragment_err}"
        );
    }

    #[test]
    fn test_resolve_airdrop_target_uses_default_testnet_faucet_url() {
        let target = resolve_airdrop_target(WalletAirdropNetworkArg::Testnet, None)
            .expect("testnet target must resolve");
        match target {
            AirdropTarget::Testnet { faucet_url } => assert_eq!(faucet_url, DEFAULT_FAUCET_URL),
            AirdropTarget::Localnet { .. } => panic!("expected testnet target"),
        }
    }

    #[test]
    fn test_new_wallet_airdrop_faucet_url_uses_explicit_flag_or_default() {
        let explicit = new_wallet_airdrop_faucet_url(Some("https://example.com/faucet".to_owned()));
        assert_eq!(explicit, "https://example.com/faucet");

        let fallback = new_wallet_airdrop_faucet_url(None);
        assert_eq!(fallback, DEFAULT_FAUCET_URL);
    }

    #[test]
    fn test_solve_challenge_rejects_too_high_difficulty() {
        let err = solve_challenge("abc", 257).expect_err("difficulty > 256 must fail");
        assert!(
            err.to_string()
                .contains("PoW difficulty must be at most 256 bits"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_solve_challenge_respects_nonce_limit() {
        let err = solve_challenge_with_limits("abc", 1, Duration::from_secs(10), 0)
            .expect_err("must stop on nonce limit");
        assert!(
            err.to_string().contains("PoW solve exceeded nonce limit"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_solve_challenge_respects_time_limit() {
        let err = solve_challenge_with_limits("abc", 1, Duration::ZERO, u64::MAX)
            .expect_err("must stop on time limit");
        assert!(
            err.to_string().contains("PoW solve exceeded time limit"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_should_prompt_auto_airdrop_interactive_defaults_to_prompt() {
        assert!(should_prompt_auto_airdrop(false, false, true, true));
    }

    #[test]
    fn test_should_prompt_auto_airdrop_disabled_for_json_or_non_tty() {
        assert!(!should_prompt_auto_airdrop(false, true, true, true));
        assert!(!should_prompt_auto_airdrop(false, false, false, true));
        assert!(!should_prompt_auto_airdrop(false, false, true, false));
    }

    #[test]
    fn test_should_prompt_auto_airdrop_disabled_when_flag_is_set() {
        assert!(!should_prompt_auto_airdrop(true, false, true, true));
    }

    #[test]
    fn test_should_wait_for_testnet_airdrop_balance_interactive_by_default() {
        assert!(should_wait_for_testnet_airdrop_balance(false, true, true));
    }

    #[test]
    fn test_should_wait_for_testnet_airdrop_balance_disabled_by_flag_or_non_tty() {
        assert!(!should_wait_for_testnet_airdrop_balance(true, true, true));
        assert!(!should_wait_for_testnet_airdrop_balance(false, false, true));
        assert!(!should_wait_for_testnet_airdrop_balance(false, true, false));
    }

    #[test]
    fn test_send_with_retry_retries_transport_errors() {
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .connect_timeout(Duration::from_millis(50))
            .timeout(Duration::from_millis(100))
            .build()
            .expect("failed to create reqwest client");

        let mut attempts = 0;
        let err = send_with_retry(
            || {
                attempts += 1;
                client.get("http://127.0.0.1:1/").send()
            },
            "challenge",
            "Failed to get challenge from faucet",
            true,
        )
        .expect_err("transport error path must fail");

        assert_eq!(attempts, HTTP_RETRY_ATTEMPTS);
        assert!(
            err.to_string()
                .contains("Failed to get challenge from faucet"),
            "unexpected error: {err}"
        );
    }
}
