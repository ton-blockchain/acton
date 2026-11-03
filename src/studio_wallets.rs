use std::borrow::Cow;
use std::collections::BTreeMap;

use acton_config::config::ActonConfig;
use acton_studio::{
    EnvironmentConfig, PublicTonNetwork, StudioEnvironment, StudioWallet, WalletRuntime,
    WalletRuntimeError, WalletRuntimeFuture,
};
use ed25519_dalek::{Signer, SigningKey};
use rston::models::{Base64StdAddrFlags, DisplayBase64StdAddr};
use ton_retrace::Network;

use crate::context::Wallet;
use crate::wallets::open_selected_wallets;

pub(crate) struct ProjectWalletRuntime {
    config: ActonConfig,
    localnet_wallets: BTreeMap<String, Wallet>,
    mainnet_wallets: BTreeMap<String, Wallet>,
}

impl ProjectWalletRuntime {
    pub(crate) fn new(config: &ActonConfig) -> anyhow::Result<Self> {
        let wallet_names = config
            .wallets()
            .into_iter()
            .flatten()
            .filter(|(_, wallet)| {
                matches!(wallet.kind.to_ascii_lowercase().as_str(), "v4r2" | "v5r1")
            })
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        if wallet_names.is_empty() {
            return Ok(Self {
                config: config.clone(),
                localnet_wallets: BTreeMap::new(),
                mainnet_wallets: BTreeMap::new(),
            });
        }

        Ok(Self {
            config: config.clone(),
            localnet_wallets: open_selected_wallets(config, &wallet_names, &Network::Localnet)?,
            mainnet_wallets: open_selected_wallets(config, &wallet_names, &Network::Mainnet)?,
        })
    }

    fn wallets_for(
        &self,
        environment: &StudioEnvironment,
    ) -> Result<Cow<'_, BTreeMap<String, Wallet>>, WalletRuntimeError> {
        if let EnvironmentConfig::FullTonNetwork { global_id, .. } = &environment.config {
            if *global_id == acton_config::config::TESTNET_GLOBAL_ID {
                return Ok(Cow::Borrowed(&self.localnet_wallets));
            }
            let mut wallets = self.localnet_wallets.clone();
            crate::wallets::apply_wallet_global_id(&self.config, &mut wallets, *global_id)
                .map_err(|error| WalletRuntimeError::Internal {
                    code: "wallet_genesis_invalid",
                    message: error.to_string(),
                })?;
            return Ok(Cow::Owned(wallets));
        }
        Ok(Cow::Borrowed(match &environment.config {
            EnvironmentConfig::ActonSimulatedLocalnet { .. }
            | EnvironmentConfig::FullTonNetwork { .. }
            | EnvironmentConfig::RemoteTonNetwork {
                network: PublicTonNetwork::Testnet,
            } => &self.localnet_wallets,
            EnvironmentConfig::RemoteTonNetwork {
                network: PublicTonNetwork::Mainnet,
            } => &self.mainnet_wallets,
        }))
    }
}

impl WalletRuntime for ProjectWalletRuntime {
    fn prepare_localnet_accounts(
        &self,
        names: Vec<String>,
        global_id: i32,
    ) -> WalletRuntimeFuture<'_, Vec<acton_localnet::StartupWallet>> {
        Box::pin(async move {
            crate::wallets::prepare_localnet_wallets(&self.config, &names, global_id).map_err(
                |error| WalletRuntimeError::InvalidRequest {
                    code: "startup_wallet_invalid",
                    message: error.to_string(),
                },
            )
        })
    }

    fn list(&self, environment: &StudioEnvironment) -> WalletRuntimeFuture<'_, Vec<StudioWallet>> {
        let result = self.wallets_for(environment).map(|wallets| {
            wallets
                .values()
                .map(|wallet| StudioWallet {
                    name: wallet.name.clone(),
                    address: DisplayBase64StdAddr {
                        addr: &wallet.wallet.address,
                        flags: Base64StdAddrFlags {
                            testnet: true,
                            bounceable: false,
                            base64_url: true,
                        },
                    }
                    .to_string(),
                    public_key: format!("0x{}", hex::encode(wallet.wallet.key_pair.public_key)),
                    version: crate::commands::simulator::wallet_version_to_string(
                        wallet.wallet.version,
                    )
                    .to_owned(),
                    wallet_id: wallet.wallet.wallet_id,
                    workchain: i32::from(wallet.wallet.address.workchain),
                })
                .collect()
        });
        Box::pin(async move { result })
    }

    fn sign(
        &self,
        environment: &StudioEnvironment,
        wallet_name: &str,
        bytes: Vec<u8>,
    ) -> WalletRuntimeFuture<'_, [u8; 64]> {
        let result = self.wallets_for(environment).and_then(|wallets| {
            wallets
                .get(wallet_name)
                .ok_or_else(|| WalletRuntimeError::NotFound {
                    wallet_name: wallet_name.to_owned(),
                })
                .and_then(|wallet| {
                    let signing_key =
                        SigningKey::from_keypair_bytes(&wallet.wallet.key_pair.secret_key)
                            .map_err(|error| WalletRuntimeError::Internal {
                                code: "wallet_signing_key_invalid",
                                message: format!(
                                    "Failed to load the signing key for wallet {}: {error}",
                                    wallet.name
                                ),
                            })?;
                    if signing_key.verifying_key().to_bytes() != wallet.wallet.key_pair.public_key {
                        return Err(WalletRuntimeError::Internal {
                            code: "wallet_signing_key_invalid",
                            message: format!(
                                "The signing key for wallet {} does not match its public key",
                                wallet.name
                            ),
                        });
                    }
                    Ok(signing_key.sign(&bytes).to_bytes())
                })
        });
        Box::pin(async move { result })
    }
}
