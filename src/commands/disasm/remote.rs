use crate::commands::common::error_fmt;
use anyhow::Context;
use std::str::FromStr;
use ton_api::{Network, TonApiClient};
use tonlib_core::TonAddress;

pub(super) fn fetch_contract_boc(address: &str, api_key: Option<&str>) -> anyhow::Result<String> {
    TonAddress::from_str(address).with_context(|| error_fmt::invalid_address(address))?;

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
            color_print::cformat!(
                "Contract with address <yellow>{address}</> not found on both mainnet and testnet",
            )
        })
    }
}
