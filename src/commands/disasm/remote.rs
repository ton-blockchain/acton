use crate::commands::common::error_fmt;
use acton_config::color::OwoColorize;
use anyhow::Context;
use std::str::FromStr;
use ton_api::{Network, TonApiClient};
use tonlib_core::TonAddress;

pub(super) fn fetch_contract_boc(
    network: Option<Network>,
    address: &str,
    api_key: Option<&str>,
) -> anyhow::Result<String> {
    TonAddress::from_str(address).with_context(|| error_fmt::invalid_address(address))?;

    if let Some(network) = network {
        let config = acton_config::config::ActonConfig::load().unwrap_or_default();
        let custom_networks = config.custom_networks();
        let client = TonApiClient::new(
            network.clone(),
            custom_networks,
            api_key.map(ToString::to_string),
        )?;
        return client
            .get_contract_boc(address)
            .with_context(|| format!("Failed to fetch contract boc from {network}"));
    }

    // No explicit network given, trying both
    let config = acton_config::config::ActonConfig::load().unwrap_or_default();
    let custom_networks = config.custom_networks();
    let mainnet_client = TonApiClient::new(
        Network::Mainnet,
        custom_networks.clone(),
        api_key.map(ToString::to_string),
    )?;
    if let Ok(boc) = mainnet_client.get_contract_boc(address) {
        Ok(boc)
    } else {
        let testnet_client = TonApiClient::new(
            Network::Testnet,
            custom_networks,
            api_key.map(ToString::to_string),
        )?;
        testnet_client.get_contract_boc(address).with_context(|| {
            format!(
                "Contract with address {} not found on both mainnet and testnet",
                address.yellow()
            )
        })
    }
}
