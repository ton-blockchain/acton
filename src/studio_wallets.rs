use std::collections::BTreeMap;

use acton_config::config::ActonConfig;
use acton_studio::{
    EnvironmentConfig, PublicTonNetwork, StudioEnvironment, StudioWallet, WalletRuntime,
    WalletRuntimeError, WalletRuntimeFuture,
};
use ed25519_dalek::{Signer, SigningKey};
use ton_retrace::Network;

use crate::context::Wallet;
use crate::wallets::open_selected_wallets;

pub(crate) struct ProjectWalletRuntime {
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
                localnet_wallets: BTreeMap::new(),
                mainnet_wallets: BTreeMap::new(),
            });
        }

        Ok(Self {
            localnet_wallets: open_selected_wallets(config, &wallet_names, &Network::Localnet)?,
            mainnet_wallets: open_selected_wallets(config, &wallet_names, &Network::Mainnet)?,
        })
    }

    const fn wallets_for(&self, environment: &StudioEnvironment) -> &BTreeMap<String, Wallet> {
        match &environment.config {
            EnvironmentConfig::ActonLocalnet { .. }
            | EnvironmentConfig::RemoteTonNetwork {
                network: PublicTonNetwork::Testnet,
            } => &self.localnet_wallets,
            EnvironmentConfig::FullTonNetwork { .. }
            | EnvironmentConfig::RemoteTonNetwork {
                network: PublicTonNetwork::Mainnet,
            } => &self.mainnet_wallets,
        }
    }
}

impl WalletRuntime for ProjectWalletRuntime {
    fn list(&self, environment: &StudioEnvironment) -> WalletRuntimeFuture<'_, Vec<StudioWallet>> {
        let wallets = self
            .wallets_for(environment)
            .values()
            .map(|wallet| StudioWallet {
                name: wallet.name.clone(),
                address: wallet.wallet.address.to_base64(false, false, true),
                public_key: format!("0x{}", hex::encode(wallet.wallet.key_pair.public_key)),
                version: crate::commands::localnet::wallet_version_to_string(wallet.wallet.version)
                    .to_owned(),
                wallet_id: wallet.wallet.wallet_id,
                workchain: wallet.wallet.address.workchain,
            })
            .collect();
        Box::pin(async move { Ok(wallets) })
    }

    fn sign(
        &self,
        environment: &StudioEnvironment,
        wallet_name: &str,
        bytes: Vec<u8>,
    ) -> WalletRuntimeFuture<'_, [u8; 64]> {
        let result =
            self.wallets_for(environment)
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
                });
        Box::pin(async move { result })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use acton_studio::{EnvironmentEndpoints, EnvironmentStatus};

    #[test]
    fn public_networks_use_wallets_with_their_global_id() {
        let runtime = ProjectWalletRuntime {
            localnet_wallets: BTreeMap::new(),
            mainnet_wallets: BTreeMap::new(),
        };
        let testnet = remote_environment(PublicTonNetwork::Testnet);
        let mainnet = remote_environment(PublicTonNetwork::Mainnet);

        assert!(std::ptr::eq(
            runtime.wallets_for(&testnet),
            &runtime.localnet_wallets
        ));
        assert!(std::ptr::eq(
            runtime.wallets_for(&mainnet),
            &runtime.mainnet_wallets
        ));
    }

    fn remote_environment(network: PublicTonNetwork) -> StudioEnvironment {
        StudioEnvironment::new_external(
            "network",
            "Network",
            EnvironmentStatus::Running,
            EnvironmentConfig::RemoteTonNetwork { network },
            EnvironmentEndpoints::default(),
        )
    }
}
