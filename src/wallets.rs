use crate::config::ActonConfig;
use crate::context::Wallet;
use anyhow::anyhow;
use owo_colors::OwoColorize;
use std::collections::BTreeMap;
use std::fs;
use std::str::FromStr;
use tonlib_core::TonAddress;
use tonlib_core::wallet::ton_wallet::TonWallet;
use tonlib_core::wallet::versioned::{
    DEFAULT_WALLET_ID, DEFAULT_WALLET_ID_V5R1, DEFAULT_WALLET_ID_V5R1_TESTNET,
};
use tonlib_core::wallet::wallet_version::WalletVersion;

pub fn open_wallets(
    config: &ActonConfig,
    net: &str,
    broadcast: bool,
) -> anyhow::Result<BTreeMap<String, Wallet>> {
    if !broadcast {
        return Ok(BTreeMap::new());
    }

    let wallets = config
        .wallets
        .as_ref()
        .map(|w| w.wallets.clone())
        .unwrap_or_default();

    let mut open_wallets: BTreeMap<String, Wallet> = BTreeMap::new();

    for (name, wallet) in wallets {
        let mnemonic = if let Some(env) = wallet.keys.mnemonic_env {
            Some(std::env::var(&env).map_err(|err| {
                anyhow!(
                    "Cannot access env variable {} for wallet mnemonic: {err}",
                    env.yellow()
                )
            })?)
        } else if let Some(file) = wallet.keys.mnemonic_file {
            Some(
                fs::read_to_string(&file)
                    .map_err(|err| {
                        anyhow!(
                            "Cannot access file {} for wallet mnemonic: {err}",
                            file.yellow()
                        )
                    })?
                    .trim()
                    .to_string(),
            )
        } else {
            None
        };

        let Some(mnemonic) = mnemonic else {
            anyhow::bail!("No mnemonic found for '{name}' wallet")
        };

        let mnemonic = tonlib_core::wallet::mnemonic::Mnemonic::from_str(&mnemonic, &None)?;

        let wallet_version = parse_wallet_version(&wallet.kind)?;
        let wallet_id = wallet_id(wallet_version, net);

        let ton_wallet = TonWallet::new_with_params(
            wallet_version,
            mnemonic.to_key_pair()?,
            wallet.workchain.unwrap_or(0),
            wallet_id,
        )?;

        if let Some(expected) = &wallet.expected {
            let expected_address = match net {
                "mainnet" => expected
                    .address_mainnet
                    .as_ref()
                    .map(|a| TonAddress::from_str(&a.to_string())),
                "testnet" => expected
                    .address_testnet
                    .as_ref()
                    .map(|a| TonAddress::from_str(&a.to_string())),
                _ => None,
            };

            if let Some(expected_addr) = expected_address {
                match expected_addr {
                    Ok(expected_addr) => {
                        if ton_wallet.address != expected_addr {
                            anyhow::bail!(
                                "Wallet address mismatch for '{name}' on '{net}':\n  Expected: {expected_addr}\n  Derived:  {}\n\nPossible causes:\n  - Wrong mnemonic/private key\n  - Incorrect 'kind' or 'workchain'\n  - Keys rotated but expected.address-{net} not updated",
                                ton_wallet.address.to_base64_std(),
                            );
                        }
                    }
                    Err(err) => {
                        let expected_address = match net {
                            "mainnet" => expected.address_mainnet.as_deref(),
                            "testnet" => expected.address_testnet.as_deref(),
                            _ => None,
                        }
                        .unwrap_or("<unknown>");
                        anyhow::bail!(
                            "Wallet address {expected_address} for {net} is not a valid address: {err}"
                        );
                    }
                }
            }
        }

        open_wallets.insert(
            name.clone(),
            Wallet {
                name: name.clone(),
                wallet: ton_wallet,
                seqno: None,
            },
        );
    }

    Ok(open_wallets)
}

fn wallet_id(wallet: WalletVersion, net: &str) -> i32 {
    match wallet {
        WalletVersion::V5R1 => {
            if net == "testnet" {
                return DEFAULT_WALLET_ID_V5R1_TESTNET;
            }
            DEFAULT_WALLET_ID_V5R1
        }
        _ => DEFAULT_WALLET_ID,
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
        "highloadv1r1" => Ok(WalletVersion::HighloadV1R1),
        "highloadv1r2" => Ok(WalletVersion::HighloadV1R2),
        "highloadv2" => Ok(WalletVersion::HighloadV2),
        "highloadv2r1" => Ok(WalletVersion::HighloadV2R1),
        "highloadv2r2" => Ok(WalletVersion::HighloadV2R2),
        _ => Err(anyhow!(
            "Unsupported wallet kind: {}. Supported kinds: v1r1, v1r2, v1r3, v2r1, v2r2, v3r1, v3r2, v4r1, v4r2, v5r1, highloadv1r1, highloadv1r2, highloadv2, highloadv2r1, highloadv2r2",
            kind
        )),
    }
}
