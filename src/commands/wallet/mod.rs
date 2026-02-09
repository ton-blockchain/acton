use crate::commands::common::{create_symlink, error_fmt, select_wallet};
use crate::wallets;
use acton_config::config;
use acton_config::config::{ActonConfig, WalletsFile, global_wallets_path};
use anyhow::{Context, anyhow};
use clap::Subcommand;
use inquire::{Confirm, Select, Text};
use log::error;
use owo_colors::OwoColorize;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use toml_edit::{DocumentMut, Item, Table, value};
use ton_api::{Network, TonApiClient};
use tonlib_core::TonAddress;
use tonlib_core::wallet::mnemonic::Mnemonic;
use tonlib_core::wallet::ton_wallet::TonWallet;
use tonlib_core::wallet::wallet_version::WalletVersion;

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
            WalletVersionArg::HighloadV1R1 => WalletVersion::HighloadV1R1,
            WalletVersionArg::HighloadV1R2 => WalletVersion::HighloadV1R2,
            WalletVersionArg::HighloadV2 => WalletVersion::HighloadV2,
            WalletVersionArg::HighloadV2R1 => WalletVersion::HighloadV2R1,
            WalletVersionArg::HighloadV2R2 => WalletVersion::HighloadV2R2,
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
    #[command(about = "Get wallet mnemonic")]
    Get {
        #[arg(help = "Name of the wallet (prompts if not provided)")]
        name: Option<String>,
    },
    #[command(about = "Request testnet TONs from faucet")]
    Airdrop {
        #[arg(help = "Name of the wallet (prompts if not provided)")]
        name: Option<String>,
        #[arg(long, help = "Faucet URL", default_value = "http://localhost:3001")]
        faucet_url: String,
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
            json,
        } => new_wallet(name, version, global, local, secure, json),
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
        WalletCommand::Get { name } => get_mnemonic(name),
        WalletCommand::Airdrop {
            name,
            faucet_url,
            json,
        } => airdrop_wallet(name, faucet_url, json),
    }
}

fn airdrop_wallet(name: Option<String>, faucet_url: String, json: bool) -> anyhow::Result<()> {
    let config = ActonConfig::load()?;

    let name = select_wallet(name, &config)?;

    let wallet = config
        .get_wallet(&name)
        .ok_or_else(|| anyhow!(error_fmt::wallet_not_found(&config, &name)))?;

    let address = get_wallet_address(wallet)?;

    if !json {
        println!(
            "{} Requesting airdrop for wallet {} {}",
            "→".blue().bold(),
            name.cyan().bold(),
            address
        );
    }

    let client = reqwest::blocking::Client::new();

    // Faucet for testnet TON uses Proof-of-Work so we need to solve it to get coins

    // 1. Get challenge
    if !json {
        println!("{} Fetching PoW challenge...", "→".blue().bold());
    }
    let challenge_res = client
        .get(format!("{faucet_url}/challenge"))
        .send()
        .context("Failed to get challenge from faucet")?;

    if !challenge_res.status().is_success() {
        anyhow::bail!("Failed to get challenge: status {}", challenge_res.status());
    }

    let challenge_data: serde_json::Value = challenge_res
        .json()
        .context("Failed to parse challenge response")?;
    let challenge = challenge_data["challenge"]
        .as_str()
        .context("No challenge in response")?;
    let difficulty = challenge_data["difficulty"]
        .as_u64()
        .context("No difficulty in response")? as u32;

    // 2. Solve challenge
    if !json {
        println!(
            "{} Solving challenge (difficulty: {} bits)...",
            "→".blue().bold(),
            difficulty
        );
    }
    let start = std::time::Instant::now();
    let nonce = solve_challenge(challenge, difficulty);
    let duration = start.elapsed();
    if !json {
        println!("{} Challenge solved in {:?}", "✓".green(), duration);
    }

    // 3. Send claim
    let response = client
        .post(format!("{faucet_url}/claim"))
        .json(&serde_json::json!({
            "address": address,
            "challenge": challenge,
            "nonce": nonce
        }))
        .send()
        .context("Failed to send request to faucet")?;

    if response.status().is_success() {
        let res: serde_json::Value = response.json().context("Failed to parse faucet response")?;
        if json {
            println!(
                "{}",
                serde_json::to_string(&serde_json::json!({
                    "success": true,
                    "message": res.get("message").and_then(|m| m.as_str()).unwrap_or("Success")
                }))?
            );
        } else if let Some(msg) = res.get("message").and_then(|m| m.as_str()) {
            println!("{} {}", "✓".green(), msg);
        } else {
            println!("{} Success", "✓".green());
        }
    } else {
        let status = response.status();
        let error_msg = if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            "Too many requests from your IP. Only 2 requests per 24 hours are allowed. Please try again later.".to_string()
        } else {
            let body_text = response.text().unwrap_or_default();
            if let Ok(res) = serde_json::from_str::<serde_json::Value>(&body_text) {
                res.get("error")
                    .and_then(|e| e.as_str())
                    .map_or_else(|| body_text.clone(), ToString::to_string)
            } else {
                body_text
            }
        };

        if json {
            println!(
                "{}",
                serde_json::to_string(&serde_json::json!({
                    "success": false,
                    "error": format!("Faucet returned error {}: {}", status, error_msg)
                }))?
            );
        } else {
            anyhow::bail!("Faucet returned error {status}: {error_msg}");
        }
    }

    Ok(())
}

fn solve_challenge(challenge: &str, difficulty: u32) -> u64 {
    let mut nonce: u64 = 0;
    loop {
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
            return nonce;
        }

        nonce += 1;
    }
}

fn get_mnemonic(name: Option<String>) -> anyhow::Result<()> {
    let config = ActonConfig::load()?;

    let name = select_wallet(name, &config)?;

    let wallet = config
        .get_wallet(&name)
        .ok_or_else(|| anyhow!(error_fmt::wallet_not_found(&config, &name)))?;

    let mnemonic = wallets::load_mnemonic(wallet)?;

    println!("Mnemonic for wallet {}:", name.cyan().bold());
    println!("{}", mnemonic.green());

    Ok(())
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

    let config = ActonConfig::load().unwrap_or_default();
    let custom_networks = config.custom_networks();
    let api_key = api_key.or_else(|| env::var("TONCENTER_API_KEY").ok());
    let client = TonApiClient::new(Network::Testnet, custom_networks, api_key)?;

    if !json {
        println!("Available wallets:");
    }

    let mut wallets_data = Vec::new();
    for (name, wallet_config) in wallets {
        let Ok(address) = get_wallet_address(wallet_config) else {
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
        match client.get_account_states(&addresses) {
            Ok(states) => {
                for state in states {
                    if let Some(b) = state.balance
                        && let Ok(b_int) = b.parse::<i128>()
                        && let Ok(address) = TonAddress::from_str(&state.address)
                    {
                        balances.insert(address.to_base64_url_flags(false, true), b_int);
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

fn get_wallet_address(wallet: &config::WalletConfig) -> anyhow::Result<String> {
    if let Some(expected) = &wallet.expected
        && let Some(addr) = &expected.address_testnet
    {
        let addr = TonAddress::from_str(addr)?;
        return Ok(addr.to_base64_url_flags(false, true));
    }

    let mnemonic_str = wallets::load_mnemonic(wallet)?;

    let mnemonic = Mnemonic::from_str(&mnemonic_str, &None)?;
    let version = parse_wallet_version(&wallet.kind)?;
    let wallet_id = wallets::wallet_id(version, &Network::Testnet);
    let ton_wallet = TonWallet::new_with_params(
        version,
        mnemonic.to_key_pair()?,
        wallet.workchain.unwrap_or(0),
        wallet_id,
    )?;
    Ok(ton_wallet.address.to_base64_url_flags(false, true))
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
        WalletVersion::HighloadV1R1 => "highloadv1r1",
        WalletVersion::HighloadV1R2 => "highloadv1r2",
        WalletVersion::HighloadV2 => "highloadv2",
        WalletVersion::HighloadV2R1 => "highloadv2r1",
        WalletVersion::HighloadV2R2 => "highloadv2r2",
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
        let config_path = PathBuf::from("wallets.toml");
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
            WalletVersion::HighloadV2R2,
            WalletVersion::HighloadV2R1,
            WalletVersion::HighloadV2,
            WalletVersion::HighloadV1R2,
            WalletVersion::HighloadV1R1,
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
        let symlink_path = Path::new("global.wallets.toml");
        if !symlink_path.exists() {
            if let Err(e) = create_symlink(config_path, symlink_path) {
                println!(
                    "  {} Failed to create symlink: {}",
                    "Warning:".yellow().bold(),
                    e
                );
            } else {
                println!(
                    "{} Created symlink {} -> {}",
                    "✓".green(),
                    symlink_path.display(),
                    config_path.display()
                );
            }
        }
    }

    Ok(())
}

fn new_wallet(
    name: Option<String>,
    version: Option<WalletVersionArg>,
    global_flag: bool,
    local_flag: bool,
    secure: Option<bool>,
    json: bool,
) -> anyhow::Result<()> {
    let config = ActonConfig::load().ok();
    let name = get_or_prompt_name(name)?;
    let is_global = get_is_global(global_flag, local_flag)?;
    let config_path = get_config_path(&name, is_global)?;
    let version = get_or_prompt_version(version)?;

    let mnemonic_words = wallets::new_mnemonic()?;
    let mnemonic_str = mnemonic_words.join(" ");

    let mnemonic = Mnemonic::from_str(&mnemonic_str, &None)?;
    let key_pair = mnemonic.to_key_pair()?;

    let wallet_id = wallets::wallet_id(version, &Network::Testnet);
    let wallet = TonWallet::new_with_params(version, key_pair, 0, wallet_id)?;

    let wallet_address = wallet.address.to_base64_url_flags(false, true);

    let use_secure_store = get_or_prompt_use_keystore(secure)?;

    let project_name = if is_global {
        None
    } else {
        config.map(|c| c.package.name)
    };

    let (mnemonic_str_opt, mnemonic_keyring_opt) =
        maybe_store_mnemonic_in_keystore(&name, &mnemonic_str, use_secure_store, project_name)?;

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
        println!(
            "{} Wallet successfully created and added to {}",
            "✓".green(),
            config_path.display().cyan(),
        );
        println!("{} Wallet address is {}", "✓".green(), wallet_address);

        if use_secure_store {
            println!(
                "{} The mnemonic is securely stored in your system's keyring",
                "✓".green()
            );
        }

        println!(
            "\n{}",
            "NOTE: This is a testnet wallet. Coins in testnet have NO VALUE.".yellow()
        );
        println!(
            "\nTo get testnet coins, check official documentation: {}",
            "https://docs.ton.org/ecosystem/wallet-apps/get-coins#how-to-get-coins-on-testnet"
                .underline(),
        );
        if !use_secure_store {
            show_security_warning(config_path);
        }
    }

    Ok(())
}

fn maybe_store_mnemonic_in_keystore(
    name: &str,
    mnemonic_str: &str,
    use_secure_store: bool,
    project_name: Option<String>,
) -> anyhow::Result<(Option<String>, Option<String>)> {
    let (mnemonic_str_opt, mnemonic_keyring_opt) = if use_secure_store {
        let keyring_id = keyring_id_for_wallet(name, project_name);
        wallets::store_mnemonic_in_keyring(&keyring_id, mnemonic_str)?;
        (None, Some(keyring_id))
    } else {
        (Some(mnemonic_str.to_owned()), None)
    };
    Ok((mnemonic_str_opt, mnemonic_keyring_opt))
}

fn keyring_id_for_wallet(name: &str, project_name: Option<String>) -> String {
    if let Some(pn) = project_name {
        format!("{pn}:{name}")
    } else {
        name.to_string()
    }
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
    let config = ActonConfig::load().ok();
    let name = get_or_prompt_name(name)?;
    let is_global = get_is_global(global_flag, local_flag)?;
    let config_path = get_config_path(&name, is_global)?;

    let mnemonic_str = if mnemonics.is_empty() {
        Text::new("Enter mnemonic (24 words):").prompt()?
    } else {
        mnemonics.join(" ")
    };

    let mnemonic =
        Mnemonic::from_str(mnemonic_str.trim(), &None).context("Invalid mnemonic phrase")?;
    let key_pair = mnemonic.to_key_pair()?;

    let version = get_or_prompt_version(version)?;

    let wallet_id = wallets::wallet_id(version, &Network::Testnet);
    let wallet = TonWallet::new_with_params(version, key_pair, 0, wallet_id)?;

    let wallet_address = wallet.address.to_base64_url_flags(false, true);

    let use_secure_store = get_or_prompt_use_keystore(secure)?;

    let project_name = if is_global {
        None
    } else {
        config.map(|c| c.package.name)
    };

    let (mnemonic_str_opt, mnemonic_keyring_opt) =
        maybe_store_mnemonic_in_keystore(&name, &mnemonic_str, use_secure_store, project_name)?;

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
        println!(
            "\n{} Wallet successfully created and added to {}",
            "✓".green(),
            config_path.display().cyan(),
        );
        println!("{} Wallet address is {}", "✓".green(), wallet_address);
        if use_secure_store {
            println!(
                "\n{} The mnemonic is securely stored in your system's keyring.",
                "✓".green()
            );
        }

        if !use_secure_store {
            show_security_warning(config_path);
        }
    }
    Ok(())
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
    let use_secure_store = if wallets::is_keyring_supported() {
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
        "highloadv1r1" => Ok(WalletVersion::HighloadV1R1),
        "highloadv1r2" => Ok(WalletVersion::HighloadV1R2),
        "highloadv2" => Ok(WalletVersion::HighloadV2),
        "highloadv2r1" => Ok(WalletVersion::HighloadV2R1),
        "highloadv2r2" => Ok(WalletVersion::HighloadV2R2),
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
}
