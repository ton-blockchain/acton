use crate::commands::common::error_fmt;
use anyhow::Context;
use std::str::FromStr;
use ton_api::{Network, TonApiClient};
use tonlib_core::TonAddress;

pub fn fetch_contract_boc(address: &str, api_key: Option<&str>) -> anyhow::Result<String> {
    TonAddress::from_str(address).with_context(|| error_fmt::invalid_address(address))?;

    let mainnet_client = TonApiClient::new(Network::Mainnet, api_key.map(|s| s.to_string()));
    match mainnet_client.get_contract_boc(address) {
        Ok(boc) => Ok(boc),
        Err(_) => {
            let testnet_client =
                TonApiClient::new(Network::Testnet, api_key.map(|s| s.to_string()));
            testnet_client.get_contract_boc(address).with_context(|| {
                color_print::cformat!("Contract with address <yellow>{address}</> not found on both mainnet and testnet",)
            })
        }
    }
}
