use crate::config::ActonConfig;
use crate::wallets;
use anyhow::{Context, anyhow};
use clap::Subcommand;
use inquire::{Select, Text};
use owo_colors::OwoColorize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
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
    },
}

pub fn wallet_cmd(command: WalletCommand) -> anyhow::Result<()> {
    match command {
        WalletCommand::New { name, version } => new_wallet(name, version),
    }
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

fn new_wallet(name: Option<String>, version: Option<String>) -> anyhow::Result<()> {
    let name = if let Some(n) = name {
        n
    } else {
        Text::new("Wallet name:").with_default("wallet").prompt()?
    };

    let config = ActonConfig::load()?;
    let wallets = config
        .wallets
        .as_ref()
        .map(|w| w.wallets.clone())
        .unwrap_or_default();

    if wallets.contains_key(&name) {
        anyhow::bail!("Wallet {} already exists", name.yellow());
    }

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
    let wallet = TonWallet::new(version, key_pair)?;

    let mnemonic_file = format!("{}.mnemonic", name);
    if Path::new(&mnemonic_file).exists() {
        anyhow::bail!("File {} already exists", mnemonic_file);
    }
    fs::write(&mnemonic_file, &mnemonic_str).context("Failed to write mnemonic file")?;

    let wallet_address = wallet.address.to_base64_std_flags(false, true);
    let config_entry = format!(
        "\n[wallets.{}]
kind = \"{}\"
workchain = 0
keys = {{ mnemonic-file = \"{}\" }}

[wallets.{}.expected]
address-testnet = \"{}\"

",
        name,
        wallet_version_to_string(&version),
        mnemonic_file,
        name,
        wallet_address,
    );

    let mut file = OpenOptions::new()
        .append(true)
        .open("Acton.toml")
        .context("Failed to open Acton.toml")?;

    file.write_all(config_entry.as_bytes())
        .context("Failed to append to Acton.toml")?;

    println!(
        "{} Wallet successfully created and added to {}",
        "✓".green(),
        "Acton.toml".cyan(),
    );
    println!("{} Mnemonic saved to {}", "✓".green(), mnemonic_file.cyan());
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
        mnemonic_file.cyan()
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
