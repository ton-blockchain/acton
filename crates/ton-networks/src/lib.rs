use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, OnceLock};

static TESTNET_URL_OVERRIDE: OnceLock<CustomNetworkUrls> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct CustomNetworkUrls {
    pub v2_url: Arc<str>,
    pub v3_url: Option<Arc<str>>,
    pub explorer_url: Option<Arc<str>>,
}

/// Installs process-local endpoints for the built-in testnet.
///
/// Explicit `ACTON_TEST_TONCENTER_*` endpoint overrides still take precedence.
pub fn set_runtime_testnet_urls(urls: CustomNetworkUrls) -> Result<(), CustomNetworkUrls> {
    TESTNET_URL_OVERRIDE.set(urls)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Network {
    Mainnet,
    Testnet,
    Localnet,
    #[serde(untagged)]
    Custom(Arc<str>),
}

impl Network {
    #[must_use]
    pub fn as_str(&self) -> String {
        match self {
            Network::Mainnet => "mainnet".to_string(),
            Network::Testnet => "testnet".to_string(),
            Network::Localnet => "localnet".to_string(),
            Network::Custom(s) => s.to_string(),
        }
    }

    #[must_use]
    pub const fn uses_testnet_address_format(&self) -> bool {
        matches!(self, Network::Testnet | Network::Localnet)
    }

    fn localnet_urls(
        custom_networks: &HashMap<String, CustomNetworkUrls>,
    ) -> anyhow::Result<&CustomNetworkUrls> {
        custom_networks.get("localnet").ok_or_else(|| {
            anyhow::anyhow!("localnet urls are not available in network configuration")
        })
    }

    fn mainnet_toncenter_v2_url() -> String {
        env_value("ACTON_TEST_TONCENTER_MAINNET_V2_URL")
            .unwrap_or_else(|| "https://toncenter.com/api/v2".to_owned())
    }

    fn mainnet_toncenter_v3_url() -> String {
        env_value("ACTON_TEST_TONCENTER_MAINNET_V3_URL")
            .unwrap_or_else(|| "https://toncenter.com/api/v3".to_owned())
    }

    fn testnet_toncenter_v2_url() -> String {
        resolve_testnet_v2_url(
            env_value("ACTON_TEST_TONCENTER_TESTNET_V2_URL"),
            TESTNET_URL_OVERRIDE.get(),
        )
    }

    fn testnet_toncenter_v3_url() -> String {
        resolve_testnet_v3_url(
            env_value("ACTON_TEST_TONCENTER_TESTNET_V3_URL")
                .or_else(|| env_value("ACTON_TEST_TONCENTER_V3_URL")),
            TESTNET_URL_OVERRIDE.get(),
        )
    }

    pub fn toncenter_v3_url(
        &self,
        custom_networks: &HashMap<String, CustomNetworkUrls>,
    ) -> anyhow::Result<String> {
        match self {
            Network::Mainnet => Ok(Self::mainnet_toncenter_v3_url()),
            Network::Testnet => Ok(Self::testnet_toncenter_v3_url()),
            Network::Localnet => Network::localnet_urls(custom_networks)?
                .v3_url
                .as_ref()
                .map(ToString::to_string)
                .ok_or_else(|| anyhow::anyhow!("v3_url not configured for localnet network")),
            Network::Custom(name) => {
                let Some(urls) = custom_networks.get(name.as_ref()) else {
                    anyhow::bail!("unknown custom network: {name}")
                };
                urls.v3_url
                    .as_ref()
                    .map(ToString::to_string)
                    .ok_or_else(|| {
                        anyhow::anyhow!("v3_url not configured for custom network: {name}")
                    })
            }
        }
    }

    pub fn toncenter_v2_url(
        &self,
        custom_networks: &HashMap<String, CustomNetworkUrls>,
    ) -> anyhow::Result<String> {
        match self {
            Network::Mainnet => Ok(Self::mainnet_toncenter_v2_url()),
            Network::Testnet => Ok(Self::testnet_toncenter_v2_url()),
            Network::Localnet => Ok(Network::localnet_urls(custom_networks)?.v2_url.to_string()),
            Network::Custom(name) => {
                let Some(urls) = custom_networks.get(name.as_ref()) else {
                    anyhow::bail!("unknown custom network: {name}")
                };
                Ok(urls.v2_url.to_string())
            }
        }
    }
}

fn resolve_testnet_v2_url(
    explicit_override: Option<String>,
    runtime_override: Option<&CustomNetworkUrls>,
) -> String {
    explicit_override
        .or_else(|| runtime_override.map(|urls| urls.v2_url.to_string()))
        .unwrap_or_else(|| "https://testnet.toncenter.com/api/v2".to_owned())
}

fn resolve_testnet_v3_url(
    explicit_override: Option<String>,
    runtime_override: Option<&CustomNetworkUrls>,
) -> String {
    explicit_override
        .or_else(|| {
            runtime_override
                .and_then(|urls| urls.v3_url.as_ref())
                .map(ToString::to_string)
        })
        .unwrap_or_else(|| "https://testnet.toncenter.com/api/v3".to_owned())
}

fn env_value(env_name: &str) -> Option<String> {
    std::env::var(env_name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

impl FromStr for Network {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.to_lowercase();
        match s.as_str() {
            "mainnet" => Ok(Network::Mainnet),
            "testnet" => Ok(Network::Testnet),
            "localnet" => Ok(Network::Localnet),
            _ if s.starts_with("custom:") => {
                let custom_name = s.trim_start_matches("custom:");
                if custom_name == "localnet" {
                    Ok(Network::Localnet)
                } else {
                    Ok(Network::Custom(Arc::from(custom_name)))
                }
            }
            _ => anyhow::bail!(
                "Unknown network '{s}', supported networks: 'mainnet', 'testnet', 'localnet' and 'custom:<network-name>'"
            ),
        }
    }
}

impl std::fmt::Display for Network {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Network::Mainnet => write!(f, "mainnet"),
            Network::Testnet => write!(f, "testnet"),
            Network::Localnet => write!(f, "localnet"),
            Network::Custom(s) => write!(f, "{s}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_localnet_as_first_class_network() {
        assert_eq!(
            Network::from_str("localnet").expect("localnet should parse"),
            Network::Localnet
        );
        assert_eq!(
            Network::from_str("custom:localnet").expect("custom:localnet should normalize"),
            Network::Localnet
        );
    }

    #[test]
    fn resolves_localnet_urls_from_config() {
        let mut custom_networks = HashMap::new();
        custom_networks.insert(
            "localnet".to_string(),
            CustomNetworkUrls {
                v2_url: Arc::from("http://127.0.0.1:3010/api/v2"),
                v3_url: Some(Arc::from("http://127.0.0.1:3010/api/v3")),
                explorer_url: None,
            },
        );

        assert_eq!(
            Network::Localnet
                .toncenter_v2_url(&custom_networks)
                .expect("v2 url should resolve"),
            "http://127.0.0.1:3010/api/v2"
        );
        assert_eq!(
            Network::Localnet
                .toncenter_v3_url(&custom_networks)
                .expect("v3 url should resolve"),
            "http://127.0.0.1:3010/api/v3"
        );
    }

    #[test]
    fn resolves_testnet_urls_from_runtime_override() {
        let runtime_override = CustomNetworkUrls {
            v2_url: Arc::from("http://127.0.0.1:3016/api/v1/environments/testnet/rpc/api/v2"),
            v3_url: Some(Arc::from(
                "http://127.0.0.1:3016/api/v1/environments/testnet/rpc/api/v3",
            )),
            explorer_url: None,
        };

        assert_eq!(
            resolve_testnet_v2_url(None, Some(&runtime_override)),
            "http://127.0.0.1:3016/api/v1/environments/testnet/rpc/api/v2"
        );
        assert_eq!(
            resolve_testnet_v3_url(None, Some(&runtime_override)),
            "http://127.0.0.1:3016/api/v1/environments/testnet/rpc/api/v3"
        );
        assert_eq!(
            resolve_testnet_v2_url(
                Some("http://explicit.test/api/v2".to_owned()),
                Some(&runtime_override)
            ),
            "http://explicit.test/api/v2"
        );
        assert_eq!(
            resolve_testnet_v3_url(
                Some("http://explicit.test/api/v3".to_owned()),
                Some(&runtime_override)
            ),
            "http://explicit.test/api/v3"
        );
    }
}
