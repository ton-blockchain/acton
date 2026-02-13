use std::num::NonZeroU32;
use crate::context::Wallet;
use acton_config::config;
use acton_config::config::ActonConfig;
use anyhow::{Context, anyhow};
use hmac::{Hmac, Mac};
use keyring::Entry;
use owo_colors::OwoColorize;
use ring::pbkdf2;
use rand::Rng;
use retrace::Network;
use sha2::Sha512;
use std::collections::BTreeMap;
use std::fs;
use std::str::FromStr;
use tonlib_core::TonAddress;
use tonlib_core::wallet::mnemonic::WORDLIST_EN_SET;
use tonlib_core::wallet::ton_wallet::TonWallet;
use tonlib_core::wallet::versioned::{
    DEFAULT_WALLET_ID, DEFAULT_WALLET_ID_V5R1, DEFAULT_WALLET_ID_V5R1_TESTNET,
};
use tonlib_core::wallet::wallet_version::WalletVersion;

const KEYRING_SERVICE: &str = "ton.acton.wallet";

pub fn load_mnemonic_from_keyring(id: &str) -> anyhow::Result<String> {
    let entry = Entry::new(KEYRING_SERVICE, id)?;
    entry
        .get_password()
        .with_context(|| format!("Failed to load mnemonic from keyring for {id}"))
}

pub fn store_mnemonic_in_keyring(id: &str, mnemonic: &str) -> anyhow::Result<()> {
    let entry = Entry::new(KEYRING_SERVICE, id)?;
    entry
        .set_password(mnemonic)
        .with_context(|| format!("Failed to store mnemonic in keyring for {id}"))
}

#[must_use]
pub fn is_keyring_supported() -> bool {
    // Try to perform a dummy operation to check if the keyring backend is functional.
    // Real native backends will succeed (or return NoEntry for get),
    // while the default no-op mock will fail on set_password.
    let entry = match Entry::new("ton.acton.check", "healthcheck") {
        Ok(e) => e,
        Err(_) => return false,
    };

    match entry.set_password("test") {
        Ok(()) => {
            let _ = entry.delete_credential();
            true
        }
        Err(_) => false,
    }
}

pub fn load_mnemonic(wallet: &config::WalletConfig) -> anyhow::Result<String> {
    if let Some(env) = &wallet.keys.mnemonic_env {
        std::env::var(env).map_err(|err| {
            anyhow!(
                "Cannot access env variable {} for wallet mnemonic: {err}",
                env.yellow()
            )
        })
    } else if let Some(file) = &wallet.keys.mnemonic_file {
        fs::read_to_string(file)
            .map_err(|err| {
                anyhow!(
                    "Cannot access file {} for wallet mnemonic: {err}",
                    file.yellow()
                )
            })
            .map(|s| s.trim().to_string())
    } else if let Some(keyring_id) = &wallet.keys.mnemonic_keyring {
        load_mnemonic_from_keyring(keyring_id)
    } else if let Some(mnemonic) = &wallet.keys.mnemonic {
        Ok(mnemonic.clone())
    } else {
        anyhow::bail!("No mnemonic source found for wallet")
    }
}

pub fn open_wallets(
    config: &ActonConfig,
    net: Option<&Network>,
    broadcast: bool,
) -> anyhow::Result<BTreeMap<String, Wallet>> {
    if !broadcast {
        return Ok(BTreeMap::new());
    }

    let net = net.unwrap_or(&Network::Testnet);
    let wallets = config
        .wallets
        .as_ref()
        .map(|w| w.wallets.clone())
        .unwrap_or_default();

    let mut open_wallets: BTreeMap<String, Wallet> = BTreeMap::new();

    for (name, wallet) in wallets {
        let mnemonic_str = load_mnemonic(&wallet)
            .with_context(|| format!("No mnemonic found for '{name}' wallet"))?;

        let mnemonic = tonlib_core::wallet::mnemonic::Mnemonic::from_str(&mnemonic_str, &None)?;

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
                Network::Mainnet => expected
                    .address_mainnet
                    .as_ref()
                    .map(|a| TonAddress::from_str(&a.to_string())),
                Network::Testnet => expected
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
                                ton_wallet
                                    .address
                                    .to_base64_url_flags(true, net == &Network::Testnet),
                            );
                        }
                    }
                    Err(err) => {
                        let expected_address = match net {
                            Network::Mainnet => expected.address_mainnet.as_deref(),
                            Network::Testnet => expected.address_testnet.as_deref(),
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

#[must_use]
pub fn wallet_id(wallet: WalletVersion, net: &Network) -> i32 {
    match wallet {
        WalletVersion::V5R1 => {
            if net == &Network::Testnet {
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
            "Unsupported wallet kind: {kind}. Supported kinds: v1r1, v1r2, v1r3, v2r1, v2r2, v3r1, v3r2, v4r1, v4r2, v5r1, highloadv1r1, highloadv1r2, highloadv2, highloadv2r1, highloadv2r2"
        )),
    }
}

pub fn new_mnemonic() -> anyhow::Result<Vec<String>> {
    let wordlist: Vec<&str> = WORDLIST_EN_SET.keys().copied().collect();
    let mut rng = rand::thread_rng();
    let mut indices = [0usize; 24];
    let mut joined = String::with_capacity(256);

    loop {
        joined.clear();

        for (i, idx) in indices.iter_mut().enumerate() {
            *idx = rng.gen_range(0..wordlist.len());
            if i > 0 {
                joined.push(' ');
            }
            joined.push_str(wordlist[*idx]);
        }

        let mac = Hmac::<Sha512>::new_from_slice(joined.as_bytes())
            .map_err(|e| anyhow!("HMAC error: {e}"))?;
        let entropy = mac.finalize().into_bytes();

        let mut seed = [0u8; 64];
        pbkdf2::derive(
            pbkdf2::PBKDF2_HMAC_SHA512,
            PBKDF_ITERATIONS_SEED,
            b"TON seed version",
            &entropy,
            &mut seed,
        );

        if seed[0] != 0 {
            continue;
        }

        return Ok(indices.iter().map(|&i| wordlist[i].to_string()).collect());
    }
}

const PBKDF_ITERATIONS: u32 = 100000;
const PBKDF_ITERATIONS_SEED: NonZeroU32 = match NonZeroU32::new(PBKDF_ITERATIONS / 256) {
    Some(v) => v,
    None => panic!("PBKDF_ITERATIONS / 256 must be non-zero"),
};
