use crate::context::Wallet;
use acton_config::color::OwoColorize;
use acton_config::config;
use acton_config::config::ActonConfig;
use anyhow::{Context, anyhow};
use hmac::{Hmac, Mac};
use keyring::{Entry, Error as KeyringError};
use rand::Rng;
use ring::pbkdf2;
use sha2::Sha512;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{LazyLock, Mutex};
use ton::ton_core::types::TonAddress;
use ton::ton_wallet::{
    Mnemonic, TonWallet, WALLET_ID_DEFAULT, WALLET_V5R1_ID_DEFAULT, WALLET_V5R1_ID_DEFAULT_TESTNET,
    WORDLIST_EN_SET, WalletVersion,
};
use ton_retrace::Network;

const KEYRING_SERVICE: &str = "ton.acton.wallet";
const TEST_KEYRING_DIR_ENV: &str = "ACTON_TEST_KEYRING_DIR"; // integration tests only

type MnemonicBundle = BTreeMap<String, String>;

static KEYRING_BUNDLE_CACHE: LazyLock<Mutex<HashMap<String, MnemonicBundle>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn format_ton_address(address: &TonAddress, testnet: bool, bounceable: bool) -> String {
    address.to_base64(!testnet, bounceable, true)
}

fn test_keyring_file_path(id: &str) -> Option<PathBuf> {
    let dir = std::env::var(TEST_KEYRING_DIR_ENV).ok()?;
    let encoded_id = id
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    Some(PathBuf::from(dir).join(format!("{encoded_id}.mnemonic")))
}

fn keyring_cache_key(id: &str) -> String {
    match std::env::var(TEST_KEYRING_DIR_ENV) {
        Ok(dir) => format!("test:{dir}:{id}"),
        Err(_) => format!("native:{id}"),
    }
}

fn parse_keyring_bundle(id: &str, raw: &str) -> anyhow::Result<MnemonicBundle> {
    serde_json::from_str(raw)
        .with_context(|| format!("Failed to decode keyring bundle for {id} as JSON"))
}

fn serialize_keyring_bundle(id: &str, bundle: &MnemonicBundle) -> anyhow::Result<String> {
    serde_json::to_string(bundle)
        .with_context(|| format!("Failed to encode keyring bundle for {id} as JSON"))
}

fn load_keyring_bundle(id: &str) -> anyhow::Result<MnemonicBundle> {
    let cache_key = keyring_cache_key(id);
    let bundle = KEYRING_BUNDLE_CACHE
        .lock()
        .expect("keyring bundle cache mutex poisoned")
        .get(&cache_key)
        .cloned();
    if let Some(bundle) = bundle {
        return Ok(bundle);
    }

    let bundle = if let Some(path) = test_keyring_file_path(id) {
        match fs::read_to_string(&path) {
            Ok(raw) => parse_keyring_bundle(id, &raw).with_context(|| {
                format!("Failed to load mnemonic bundle from test keyring for {id}")
            })?,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => MnemonicBundle::new(),
            Err(err) => {
                return Err(err).with_context(|| {
                    format!("Failed to load mnemonic bundle from test keyring for {id}")
                });
            }
        }
    } else {
        let entry = Entry::new(KEYRING_SERVICE, id)?;
        match entry.get_password() {
            Ok(raw) => parse_keyring_bundle(id, &raw)
                .with_context(|| format!("Failed to load mnemonic bundle from keyring for {id}"))?,
            Err(KeyringError::NoEntry) => MnemonicBundle::new(),
            Err(err) => {
                return Err(err).with_context(|| {
                    format!("Failed to load mnemonic bundle from keyring for {id}")
                });
            }
        }
    };

    KEYRING_BUNDLE_CACHE
        .lock()
        .expect("keyring bundle cache mutex poisoned")
        .insert(cache_key, bundle.clone());
    Ok(bundle)
}

fn write_keyring_bundle(id: &str, bundle: &MnemonicBundle) -> anyhow::Result<()> {
    let serialized = serialize_keyring_bundle(id, bundle)?;

    if let Some(path) = test_keyring_file_path(id) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "Failed to create test keyring directory {}",
                    parent.display()
                )
            })?;
        }
        fs::write(&path, serialized)
            .with_context(|| format!("Failed to store mnemonic bundle in test keyring for {id}"))?;
    } else {
        let entry = Entry::new(KEYRING_SERVICE, id)?;
        entry
            .set_password(&serialized)
            .with_context(|| format!("Failed to store mnemonic bundle in keyring for {id}"))?;
    }

    KEYRING_BUNDLE_CACHE
        .lock()
        .expect("keyring bundle cache mutex poisoned")
        .insert(keyring_cache_key(id), bundle.clone());
    Ok(())
}

fn delete_keyring_bundle(id: &str) -> anyhow::Result<()> {
    if let Some(path) = test_keyring_file_path(id) {
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err).with_context(|| {
                format!("Failed to delete mnemonic bundle in test keyring for {id}")
            }),
        }?;
    } else {
        let entry = Entry::new(KEYRING_SERVICE, id)?;
        match entry.delete_credential() {
            Ok(()) | Err(KeyringError::NoEntry) => {}
            Err(err) => {
                return Err(err).with_context(|| {
                    format!("Failed to delete mnemonic bundle in keyring for {id}")
                });
            }
        }
    }

    KEYRING_BUNDLE_CACHE
        .lock()
        .expect("keyring bundle cache mutex poisoned")
        .remove(&keyring_cache_key(id));
    Ok(())
}

pub fn load_mnemonic_from_keyring(id: &str, wallet_name: &str) -> anyhow::Result<String> {
    let bundle = load_keyring_bundle(id)?;
    bundle.get(wallet_name).cloned().ok_or_else(|| {
        anyhow!(
            "Failed to load mnemonic from keyring for wallet {wallet_name}: no entry found in bundle {id}"
        )
    })
}

pub fn store_mnemonic_in_keyring(
    id: &str,
    wallet_name: &str,
    mnemonic: &str,
) -> anyhow::Result<()> {
    let mut bundle = load_keyring_bundle(id)?;
    bundle.insert(wallet_name.to_owned(), mnemonic.to_owned());
    write_keyring_bundle(id, &bundle)
}

pub fn delete_mnemonic_from_keyring(id: &str, wallet_name: &str) -> anyhow::Result<()> {
    let mut bundle = load_keyring_bundle(id)?;
    if !bundle.contains_key(wallet_name) {
        return Ok(());
    }

    bundle.remove(wallet_name);
    if bundle.is_empty() {
        delete_keyring_bundle(id)
    } else {
        write_keyring_bundle(id, &bundle)
    }
}

#[must_use]
pub fn is_keyring_supported() -> bool {
    if let Ok(dir) = std::env::var(TEST_KEYRING_DIR_ENV) {
        if fs::create_dir_all(&dir).is_err() {
            return false;
        }
        return true;
    }

    // Try to perform a dummy operation to check if the keyring backend is functional.
    // Real native backends will succeed (or return NoEntry for get),
    // while the default no-op mock will fail on set_password.
    let Ok(entry) = Entry::new("ton.acton.check", "healthcheck") else {
        return false;
    };

    match entry.set_password("test") {
        Ok(()) => {
            let _ = entry.delete_credential();
            true
        }
        Err(_) => false,
    }
}

pub fn load_mnemonic(wallet_name: &str, wallet: &config::WalletConfig) -> anyhow::Result<String> {
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
        load_mnemonic_from_keyring(keyring_id, wallet_name)
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
        let mnemonic_str = load_mnemonic(&name, &wallet)
            .with_context(|| format!("No mnemonic found for '{name}' wallet"))?;

        let mnemonic = Mnemonic::from_str(&mnemonic_str, None)?;

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
                    .map(|a| TonAddress::from_str(&a.clone())),
                Network::Testnet | Network::Localnet => expected
                    .address_testnet
                    .as_ref()
                    .map(|a| TonAddress::from_str(&a.clone())),
                _ => None,
            };

            if let Some(expected_addr) = expected_address {
                match expected_addr {
                    Ok(expected_addr) => {
                        if ton_wallet.address != expected_addr {
                            let derived_address = ton_wallet.address;
                            let derived_address = format_ton_address(
                                &derived_address,
                                net.uses_testnet_address_format(),
                                false,
                            );
                            anyhow::bail!(
                                "Wallet address mismatch for '{name}' on '{net}':\n  Expected: {expected_addr}\n  Derived:  {derived_address}\n\nPossible causes:\n  - Wrong mnemonic/private key\n  - Incorrect 'kind' or 'workchain'\n  - Keys rotated but expected.address-{net} not updated",
                            );
                        }
                    }
                    Err(err) => {
                        let expected_address = match net {
                            Network::Mainnet => expected.address_mainnet.as_deref(),
                            Network::Testnet | Network::Localnet => {
                                expected.address_testnet.as_deref()
                            }
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

pub fn open_selected_wallets(
    config: &ActonConfig,
    wallet_names: &[String],
    net: &Network,
) -> anyhow::Result<BTreeMap<String, Wallet>> {
    let configured_wallets = config
        .wallets
        .as_ref()
        .map(|wallets| wallets.wallets.clone())
        .unwrap_or_default();

    if configured_wallets.is_empty() {
        anyhow::bail!("No wallets are configured in Acton.toml");
    }

    let mut selected_wallets = BTreeMap::new();
    let mut missing_wallets = Vec::new();

    for wallet_name in wallet_names {
        let wallet_name = wallet_name.trim();
        if wallet_name.is_empty() {
            continue;
        }

        if selected_wallets.contains_key(wallet_name) {
            continue;
        }

        let Some(wallet_config) = configured_wallets.get(wallet_name).cloned() else {
            missing_wallets.push(wallet_name.to_owned());
            continue;
        };

        selected_wallets.insert(wallet_name.to_owned(), wallet_config);
    }

    if !missing_wallets.is_empty() {
        anyhow::bail!(
            "Wallets are not found in Acton.toml: {}",
            missing_wallets.join(", ")
        );
    }

    let mut selected_config = config.clone();
    selected_config.wallets = Some(config::WalletsConfig {
        wallets: selected_wallets,
    });

    open_wallets(&selected_config, Some(net), true)
}

#[must_use]
pub const fn wallet_id(wallet: WalletVersion, net: &Network) -> i32 {
    match wallet {
        WalletVersion::V5R1 => {
            if net.uses_testnet_address_format() {
                return WALLET_V5R1_ID_DEFAULT_TESTNET;
            }
            WALLET_V5R1_ID_DEFAULT
        }
        _ => WALLET_ID_DEFAULT,
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
            "Unsupported wallet kind: {kind}. Supported kinds: v1r1, v1r2, v1r3, v2r1, v2r2, v3r1, v3r2, v4r1, v4r2, v5r1, highloadv1r1, highloadv1r2, highloadv2, highloadv2r1, highloadv2r2"
        )),
    }
}

pub fn new_mnemonic() -> anyhow::Result<Vec<String>> {
    let wordlist: Vec<&str> = WORDLIST_EN_SET.iter().copied().collect();
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
