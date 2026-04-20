//! TonCenter API key resolution shared across Acton crates.

use ton_networks::Network;

/// Environment variable for the mainnet TonCenter API key.
pub const TONCENTER_MAINNET_API_KEY_ENV: &str = "TONCENTER_MAINNET_API_KEY";

/// Environment variable for the testnet TonCenter API key.
pub const TONCENTER_TESTNET_API_KEY_ENV: &str = "TONCENTER_TESTNET_API_KEY";

/// Returns the TonCenter API key env var name for the selected network.
#[must_use]
pub const fn env_var_name(network: &Network) -> Option<&'static str> {
    match network {
        Network::Mainnet => Some(TONCENTER_MAINNET_API_KEY_ENV),
        Network::Testnet => Some(TONCENTER_TESTNET_API_KEY_ENV),
        Network::Localnet | Network::Custom(_) => None,
    }
}

/// Resolves the TonCenter API key for the selected network from the process environment.
#[must_use]
pub fn api_key(network: &Network) -> Option<String> {
    api_key_with(network, |name| std::env::var(name).ok())
}

fn api_key_with<F>(network: &Network, lookup: F) -> Option<String>
where
    F: FnOnce(&str) -> Option<String>,
{
    let env_name = env_var_name(network)?;
    lookup(env_name)
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_mainnet_key_from_mainnet_env() {
        let lookup = |name: &str| match name {
            TONCENTER_MAINNET_API_KEY_ENV => Some(" mainnet-key ".to_string()),
            _ => None,
        };

        assert_eq!(
            api_key_with(&Network::Mainnet, lookup),
            Some("mainnet-key".to_string())
        );
        assert_eq!(api_key_with(&Network::Testnet, |_| None), None);
    }

    #[test]
    fn resolves_testnet_key_from_testnet_env() {
        let lookup = |name: &str| match name {
            TONCENTER_TESTNET_API_KEY_ENV => Some("testnet-key".to_string()),
            _ => None,
        };

        assert_eq!(
            api_key_with(&Network::Testnet, lookup),
            Some("testnet-key".to_string())
        );
        assert_eq!(api_key_with(&Network::Mainnet, |_| None), None);
    }

    #[test]
    fn does_not_resolve_for_localnet_or_custom_networks() {
        assert_eq!(api_key(&Network::Localnet), None);
        assert_eq!(api_key(&Network::Custom("sandbox".into())), None);
    }

    #[test]
    fn ignores_empty_values_after_trimming() {
        assert_eq!(
            api_key_with(&Network::Mainnet, |_| Some("   ".into())),
            None
        );
    }
}
