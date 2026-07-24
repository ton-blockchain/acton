use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use std::str::FromStr;
use ton::ton_core::types::TonAddress;

const BOUNCEABLE_TAG: u8 = 0x11;
const NON_BOUNCEABLE_TAG: u8 = 0x51;
const TESTNET_FLAG: u8 = 0x80;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum AddressValidationError {
    Invalid,
    Mainnet,
}

pub(super) fn parse_testnet_address(value: &str) -> Result<TonAddress, AddressValidationError> {
    let address = TonAddress::from_str(value).map_err(|_| AddressValidationError::Invalid)?;

    // Raw addresses do not encode a network. Friendly addresses do, so require
    // their testnet-only flag to prevent accidental transfers to mainnet users.
    if value.len() != 48 {
        return Ok(address);
    }

    let bytes = if value.contains(['-', '_']) {
        URL_SAFE_NO_PAD.decode(value)
    } else {
        STANDARD.decode(value)
    }
    .map_err(|_| AddressValidationError::Invalid)?;

    let tag = *bytes.first().ok_or(AddressValidationError::Invalid)?;
    let address_tag = tag & !TESTNET_FLAG;
    if !matches!(address_tag, BOUNCEABLE_TAG | NON_BOUNCEABLE_TAG) {
        return Err(AddressValidationError::Invalid);
    }
    if tag & TESTNET_FLAG == 0 {
        return Err(AddressValidationError::Mainnet);
    }

    Ok(address)
}

#[cfg(test)]
mod tests {
    use super::{AddressValidationError, parse_testnet_address};
    use ton::ton_core::types::TonAddress;

    const RAW_ADDRESS: &str = "0:2cf55953e92efbeadab7ba725c3f93a0b23f842cbba72d7b8e6f510a70e422e3";

    #[test]
    fn accepts_testnet_friendly_addresses_from_ton_core_test_vectors() {
        for address in [
            "0QAs9VlT6S776tq3unJcP5Ogsj-ELLunLXuOb1EKcOQi4-QO",
            "kQAs9VlT6S776tq3unJcP5Ogsj-ELLunLXuOb1EKcOQi47nL",
            "0QAs9VlT6S776tq3unJcP5Ogsj+ELLunLXuOb1EKcOQi4+QO",
            "kQAs9VlT6S776tq3unJcP5Ogsj+ELLunLXuOb1EKcOQi47nL",
        ] {
            assert_eq!(
                parse_testnet_address(address).map(|address| address.to_hex()),
                Ok(RAW_ADDRESS.to_string())
            );
        }
    }

    #[test]
    fn rejects_mainnet_friendly_addresses_from_ton_core_test_vectors() {
        for address in [
            "EQAs9VlT6S776tq3unJcP5Ogsj-ELLunLXuOb1EKcOQi4wJB",
            "UQAs9VlT6S776tq3unJcP5Ogsj-ELLunLXuOb1EKcOQi41-E",
            "EQAs9VlT6S776tq3unJcP5Ogsj+ELLunLXuOb1EKcOQi4wJB",
            "UQAs9VlT6S776tq3unJcP5Ogsj+ELLunLXuOb1EKcOQi41+E",
        ] {
            assert_eq!(
                parse_testnet_address(address),
                Err(AddressValidationError::Mainnet)
            );
        }
    }

    #[test]
    fn accepts_network_agnostic_raw_addresses() {
        let address = TonAddress::ZERO.to_hex();

        assert_eq!(parse_testnet_address(&address), Ok(TonAddress::ZERO));
    }

    #[test]
    fn rejects_invalid_addresses() {
        assert_eq!(
            parse_testnet_address("not-an-address"),
            Err(AddressValidationError::Invalid)
        );
    }
}
