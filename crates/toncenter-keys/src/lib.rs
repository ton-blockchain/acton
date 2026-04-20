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
    let env_name = env_var_name(network)?;
    lookup_api_key(env_name, |name| std::env::var(name).ok())
}

fn lookup_api_key<F>(env_name: &str, lookup: F) -> Option<String>
where
    F: FnOnce(&str) -> Option<String>,
{
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
            TONCENTER_TESTNET_API_KEY_ENV => None,
            _ => None,
        };

        assert_eq!(
            lookup_api_key(TONCENTER_MAINNET_API_KEY_ENV, lookup),
            Some("mainnet-key".to_string())
        );
        assert_eq!(
            env_var_name(&Network::Testnet),
            Some(TONCENTER_TESTNET_API_KEY_ENV)
        );
    }

    #[test]
    fn resolves_testnet_key_from_testnet_env() {
        let lookup = |name: &str| match name {
            TONCENTER_TESTNET_API_KEY_ENV => Some("testnet-key".to_string()),
            TONCENTER_MAINNET_API_KEY_ENV => None,
            _ => None,
        };

        assert_eq!(
            lookup_api_key(TONCENTER_TESTNET_API_KEY_ENV, lookup),
            Some("testnet-key".to_string())
        );
        assert_eq!(
            env_var_name(&Network::Mainnet),
            Some(TONCENTER_MAINNET_API_KEY_ENV)
        );
    }

    #[test]
    fn does_not_resolve_for_localnet_or_custom_networks() {
        assert_eq!(api_key(&Network::Localnet), None);
        assert_eq!(api_key(&Network::Custom("sandbox".into())), None);
    }

    #[test]
    fn ignores_empty_values_after_trimming() {
        assert_eq!(
            lookup_api_key(TONCENTER_MAINNET_API_KEY_ENV, |_| Some("   ".into())),
            None
        );
    }
}
