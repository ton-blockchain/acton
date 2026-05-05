//! `TonCenter` API key resolution shared across Acton crates.

use ton_networks::Network;

/// Environment variable for the mainnet `TonCenter` API key.
pub const TONCENTER_MAINNET_API_KEY_ENV: &str = "TONCENTER_MAINNET_API_KEY";

/// Environment variable for the testnet `TonCenter` API key.
pub const TONCENTER_TESTNET_API_KEY_ENV: &str = "TONCENTER_TESTNET_API_KEY";

/// Returns the `TonCenter` API key env var name for the selected network.
#[must_use]
pub fn env_var_name(network: &Network) -> Option<String> {
    match network {
        Network::Mainnet => Some(TONCENTER_MAINNET_API_KEY_ENV.to_string()),
        Network::Testnet => Some(TONCENTER_TESTNET_API_KEY_ENV.to_string()),
        Network::Localnet => None,
        Network::Custom(name) => custom_env_var_name(name),
    }
}

/// Resolves the `TonCenter` API key for the selected network from the process environment.
#[must_use]
pub fn api_key(network: &Network) -> Option<String> {
    api_key_with(network, |name| std::env::var(name).ok())
}

fn api_key_with<F>(network: &Network, lookup: F) -> Option<String>
where
    F: FnOnce(&str) -> Option<String>,
{
    let env_name = env_var_name(network)?;
    lookup(&env_name)
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// Returns the TonCenter-compatible API key env var name for a custom network.
///
/// `custom:foo` becomes `FOO_API_KEY`, and non-alphanumeric characters are normalized to `_`.
#[must_use]
pub fn custom_env_var_name(name: &str) -> Option<String> {
    let mut normalized = String::new();
    let mut last_was_separator = false;

    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch.to_ascii_uppercase());
            last_was_separator = false;
        } else if !last_was_separator {
            normalized.push('_');
            last_was_separator = true;
        }
    }

    let normalized = normalized.trim_matches('_');
    if normalized.is_empty() {
        None
    } else {
        Some(format!("{normalized}_API_KEY"))
    }
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
    }

    #[test]
    fn resolves_custom_network_key_from_uppercase_env() {
        let lookup = |name: &str| match name {
            "SANDBOX_API_KEY" => Some("custom-key".to_string()),
            _ => None,
        };

        assert_eq!(
            api_key_with(&Network::Custom("sandbox".into()), lookup),
            Some("custom-key".to_string())
        );
    }

    #[test]
    fn normalizes_custom_network_env_names() {
        assert_eq!(
            custom_env_var_name("mock-remote"),
            Some("MOCK_REMOTE_API_KEY".to_string())
        );
        assert_eq!(
            custom_env_var_name("alpha.beta/gamma"),
            Some("ALPHA_BETA_GAMMA_API_KEY".to_string())
        );
        assert_eq!(custom_env_var_name("---"), None);
    }

    #[test]
    fn ignores_empty_values_after_trimming() {
        assert_eq!(
            api_key_with(&Network::Mainnet, |_| Some("   ".into())),
            None
        );
    }
}
