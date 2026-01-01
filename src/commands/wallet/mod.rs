use crate::commands::common::create_symlink;
use crate::config::{ActonConfig, WalletsFile, global_wallets_path};
use crate::wallets;
use anyhow::{Context, anyhow};
use clap::Subcommand;
use inquire::{Select, Text};
use log::error;
use owo_colors::OwoColorize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use toml_edit::{DocumentMut, Item, Table, value};
use ton_api::{Network, TonApiClient};
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
        #[arg(long, help = "Save wallet to global config")]
        global: bool,
        #[arg(long, help = "Save wallet to local wallets.toml")]
        local: bool,
    },
    #[command(about = "Import an existing wallet from mnemonic")]
    Import {
        #[arg(long, help = "Name of the wallet (prompts if not provided)")]
        name: Option<String>,
        #[arg(help = "Mnemonic words of the wallet")]
        mnemonics: Vec<String>,
        #[arg(long, help = "Version of the wallet (prompts if not provided)")]
        version: Option<WalletVersionArg>,
        #[arg(long, help = "Save wallet to global config")]
        global: bool,
        #[arg(long, help = "Save wallet to local wallets.toml")]
        local: bool,
    },
    #[command(about = "List available wallets")]
    List {
        #[arg(short, long, help = "Show wallet balance")]
        balance: bool,
        #[arg(long, help = "TonCenter API key for blockchain queries")]
        api_key: Option<String>,
    },
}

pub fn wallet_cmd(command: WalletCommand) -> anyhow::Result<()> {
    match command {
        WalletCommand::New {
            name,
            version,
            global,
            local,
        } => new_wallet(name, version, global, local),
        WalletCommand::Import {
            name,
            mnemonics,
            version,
            global,
            local,
        } => import_wallet(name, mnemonics, version, global, local),
        WalletCommand::List { balance, api_key } => list_wallets(balance, api_key),
    }
}

fn list_wallets(balance: bool, api_key: Option<String>) -> anyhow::Result<()> {
    let config = ActonConfig::load()?;
    let wallets = config
        .wallets
        .as_ref()
        .map(|w| &w.wallets)
        .ok_or_else(|| anyhow!("No wallets found"))?;

    if wallets.is_empty() {
        println!("No wallets found");
        return Ok(());
    }

    let api_key = api_key.or_else(|| env::var("TONCENTER_API_KEY").ok());
    let have_api_key = api_key.is_some();
    let client = TonApiClient::new(Network::Testnet, api_key);

    println!("Available wallets:");

    for (name, wallet_config) in wallets {
        let mut balance_info = String::new();
        let Ok(address) = get_wallet_address(name, wallet_config) else {
            error!("cannot get wallet address for {name}"); // very unlikely
            continue;
        };

        if balance {
            balance_info = match client.get_address_balance(&address) {
                Ok(b) => {
                    let balance_ton = b.to_string().parse::<f64>().unwrap_or(0.0) / 1_000_000_000.0;
                    format!(" — {}", format!("{:.4} TON", balance_ton).green())
                }
                Err(e) => {
                    format!(" — {}", format!("error: {}", e).red())
                }
            };

            if !have_api_key {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }

        println!(
            "  {} {} {}{balance_info}",
            name.cyan().bold(),
            address,
            format!("({})", wallet_config.kind).dimmed(),
        );
    }

    Ok(())
}

fn get_wallet_address(name: &str, wallet: &crate::config::WalletConfig) -> anyhow::Result<String> {
    if let Some(expected) = &wallet.expected
        && let Some(addr) = &expected.address_testnet
    {
        return Ok(addr.clone());
    }

    let mnemonic_str = if let Some(env_var) = &wallet.keys.mnemonic_env {
        env::var(env_var).context(format!("Env var {} not set", env_var))?
    } else if let Some(file) = &wallet.keys.mnemonic_file {
        fs::read_to_string(file)
            .context(format!("Could not read mnemonic file {}", file))?
            .trim()
            .to_string()
    } else if let Some(m) = &wallet.keys.mnemonic {
        m.clone()
    } else {
        anyhow::bail!("No mnemonic or expected address for wallet {}", name);
    };

    let mnemonic = Mnemonic::from_str(&mnemonic_str, &None)?;
    let version = parse_wallet_version(&wallet.kind)?;
    let wallet_id = wallets::wallet_id(version, "testnet");
    let ton_wallet = TonWallet::new_with_params(
        version,
        mnemonic.to_key_pair()?,
        wallet.workchain.unwrap_or(0),
        wallet_id,
    )?;
    Ok(ton_wallet.address.to_base64_std_flags(false, true))
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
                anyhow::bail!("Wallet name '{}' is invalid", n);
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
            "Global (~/.acton/wallets/global.wallets.toml)",
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
    mnemonic_str: &str,
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
        .with_context(|| format!("wallets.{} is not a table", name))?;

    wallet["kind"] = value(wallet_version_to_string(&version));
    wallet["workchain"] = value(0i64);

    let mut keys = toml_edit::InlineTable::new();
    keys.insert("mnemonic", mnemonic_str.into());
    wallet["keys"] = value(keys);

    let expected = wallet
        .entry("expected")
        .or_insert(Item::Table(Table::new()))
        .as_table_mut()
        .with_context(|| format!("wallets.{}.expected is not a table", name))?;

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
) -> anyhow::Result<()> {
    let _ = ActonConfig::load()?;
    let name = get_or_prompt_name(name)?;
    let is_global = get_is_global(global_flag, local_flag)?;
    let config_path = get_config_path(&name, is_global)?;
    let version = get_or_prompt_version(version)?;

    let mnemonic_words = wallets::new_mnemonic()?;
    let mnemonic_str = mnemonic_words.join(" ");

    let mnemonic = Mnemonic::from_str(&mnemonic_str, &None)?;
    let key_pair = mnemonic.to_key_pair()?;

    let wallet_id = wallets::wallet_id(version, "testnet");
    let wallet = TonWallet::new_with_params(version, key_pair, 0, wallet_id)?;

    let wallet_address = wallet.address.to_base64_std_flags(false, true);

    save_wallet_to_config(
        &config_path,
        &name,
        version,
        &mnemonic_str,
        &wallet_address,
        is_global,
    )?;

    println!(
        "{} Wallet successfully created and added to {}",
        "✓".green(),
        config_path.display().cyan(),
    );
    println!("{} Wallet address is {}", "✓".green(), wallet_address);

    println!(
        "\n{}",
        "NOTE: This is a testnet wallet. Coins in testnet have NO VALUE.".yellow()
    );
    println!(
        "\nTo get testnet coins, check official documentation: {}",
        "https://docs.ton.org/ecosystem/wallet-apps/get-coins#how-to-get-coins-on-testnet"
            .underline(),
    );
    println!("\n{}", "SECURITY WARNING:".red());
    println!(
        "  - The mnemonic is stored in plain text in {}",
        config_path.display().cyan()
    );
    println!("  - Do NOT commit this file to version control (already added to .gitignore)");
    println!("  - Keep your mnemonic safe and secret");

    Ok(())
}

fn import_wallet(
    name: Option<String>,
    mnemonics: Vec<String>,
    version: Option<WalletVersionArg>,
    global_flag: bool,
    local_flag: bool,
) -> anyhow::Result<()> {
    let _ = ActonConfig::load()?;
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

    let wallet_id = wallets::wallet_id(version, "testnet");
    let wallet = TonWallet::new_with_params(version, key_pair, 0, wallet_id)?;

    let wallet_address = wallet.address.to_base64_std_flags(false, true);

    save_wallet_to_config(
        &config_path,
        &name,
        version,
        &mnemonic_str,
        &wallet_address,
        is_global,
    )?;

    println!(
        "\n{} Wallet successfully created and added to {}",
        "✓".green(),
        config_path.display().cyan(),
    );
    println!("{} Wallet address is {}", "✓".green(), wallet_address);

    println!("\n{}", "SECURITY WARNING:".red());
    println!(
        "  - The mnemonic is stored in plain text in {}",
        config_path.display().cyan()
    );
    println!("  - Do NOT commit this file to version control (already added to .gitignore)");
    println!("  - Keep your mnemonic safe and secret");
    Ok(())
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
