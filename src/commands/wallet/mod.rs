use crate::commands::common::create_symlink;
use crate::config::{ActonConfig, WalletsFile, global_wallets_path};
use crate::wallets;
use anyhow::{Context, anyhow};
use clap::Subcommand;
use inquire::{Select, Text};
use log::error;
use owo_colors::OwoColorize;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use ton_api::{Network, TonApiClient};
use tonlib_core::wallet::mnemonic::Mnemonic;
use tonlib_core::wallet::ton_wallet::TonWallet;
use tonlib_core::wallet::wallet_version::WalletVersion;

#[derive(Subcommand)]
pub enum WalletCommand {
    #[command(about = "Generate a new testnet wallet")]
    New {
        #[arg(
            long,
            help = "Name of the wallet (optional, will prompt if not provided)"
        )]
        name: Option<String>,
        #[arg(
            long,
            help = "Version of the wallet (optional, will prompt if not provided)"
        )]
        version: Option<String>,
        #[arg(long, help = "Save wallet to global config")]
        global: bool,
        #[arg(long, help = "Save wallet to local wallets.toml")]
        local: bool,
    },
    #[command(about = "List available wallets")]
    List {
        #[arg(short, long, help = "Show wallet balance")]
        balance: bool,
        #[arg(long, help = "Toncenter API key")]
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

fn new_wallet(
    name: Option<String>,
    version: Option<String>,
    global_flag: bool,
    local_flag: bool,
) -> anyhow::Result<()> {
    let name = if let Some(n) = name {
        n
    } else {
        Text::new("Wallet name:").with_default("wallet").prompt()?
    };

    let _config = ActonConfig::load()?;

    let is_global = if global_flag {
        true
    } else if local_flag {
        false
    } else {
        let options = vec![
            "Local (wallets.toml)",
            "Global (~/.acton/wallets/global.wallets.toml)",
        ];
        let selection = Select::new("Save wallet to:", options).prompt()?;
        selection.starts_with("Global")
    };

    let config_path = if is_global {
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
                && w.wallets.contains_key(&name)
            {
                anyhow::bail!("Wallet {} already exists in global config", name.yellow());
            }
        }

        config_path
    } else {
        let config_path = PathBuf::from("wallets.toml");
        if config_path.exists() {
            let content = fs::read_to_string(&config_path)?;
            let wallets: WalletsFile = toml::from_str(&content)?;
            if let Some(w) = wallets.wallets
                && w.wallets.contains_key(&name)
            {
                anyhow::bail!("Wallet {} already exists in local config", name.yellow());
            }
        }

        config_path
    };

    let version = if let Some(k) = version {
        parse_wallet_version(&k)?
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
        parse_wallet_version(&selected_str)?
    };

    let mnemonic_words = wallets::new_mnemonic()?;
    let mnemonic_str = mnemonic_words.join(" ");

    let mnemonic = Mnemonic::from_str(&mnemonic_str, &None)?;
    let key_pair = mnemonic.to_key_pair()?;

    let wallet_id = wallets::wallet_id(version, "testnet");
    let wallet = TonWallet::new_with_params(version, key_pair, 0, wallet_id)?;

    let wallet_address = wallet.address.to_base64_std_flags(false, true);

    let config_entry = format!(
        "[wallets.{}]
kind = \"{}\"
workchain = 0
keys = {{ mnemonic = \"{}\" }}

[wallets.{}.expected]
address-testnet = \"{}\"
",
        name,
        wallet_version_to_string(&version),
        mnemonic_str,
        name,
        wallet_address,
    );

    let config_exists = config_path.exists();
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(&config_path)
        .context(format!("Failed to open {}", config_path.display()))?;

    if config_exists {
        // add separator between wallets if there are any
        file.write_all(b"\n")
            .context(format!("Failed to append to {}", config_path.display()))?;
    }

    file.write_all(config_entry.as_bytes())
        .context(format!("Failed to append to {}", config_path.display()))?;

    if is_global {
        let symlink_path = Path::new("global.wallets.toml");
        if !symlink_path.exists() {
            if let Err(e) = create_symlink(&config_path, symlink_path) {
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
