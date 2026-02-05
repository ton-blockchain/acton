use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct CustomNetworkUrls {
    pub v2_url: Arc<str>,
    pub v3_url: Option<Arc<str>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Network {
    Mainnet,
    Testnet,
    #[serde(untagged)]
    Custom(Arc<str>),
}

impl Network {
    #[must_use]
    pub fn as_str(&self) -> String {
        match self {
            Network::Mainnet => "mainnet".to_string(),
            Network::Testnet => "testnet".to_string(),
            Network::Custom(s) => s.to_string(),
        }
    }

    pub fn toncenter_v3_url(
        &self,
        custom_networks: &HashMap<String, CustomNetworkUrls>,
    ) -> anyhow::Result<String> {
        match self {
            Network::Mainnet => Ok("https://toncenter.com/api/v3".to_owned()),
            Network::Testnet => Ok("https://testnet.toncenter.com/api/v3".to_owned()),
            Network::Custom(name) => {
                let Some(urls) = custom_networks.get(name.as_ref()) else {
                    anyhow::bail!("unknown custom network: {name}")
                };
                urls.v3_url.as_ref().map(|s| s.to_string()).ok_or_else(|| {
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
            Network::Mainnet => Ok("https://toncenter.com/api/v2".to_owned()),
            Network::Testnet => Ok("https://testnet.toncenter.com/api/v2".to_owned()),
            Network::Custom(name) => {
                let Some(urls) = custom_networks.get(name.as_ref()) else {
                    anyhow::bail!("unknown custom network: {name}")
                };
                Ok(urls.v2_url.to_string())
            }
        }
    }
}

impl FromStr for Network {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.to_lowercase();
        match s.as_str() {
            "mainnet" => Ok(Network::Mainnet),
            "testnet" => Ok(Network::Testnet),
            _ if s.starts_with("custom:") => {
                Ok(Network::Custom(Arc::from(s.trim_start_matches("custom:"))))
            }
            _ => anyhow::bail!(
                "Unknown network '{s}', supported networks: 'mainnet', 'testnet' and 'custom:<network-name>'"
            ),
        }
    }
}

impl std::fmt::Display for Network {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Network::Mainnet => write!(f, "mainnet"),
            Network::Testnet => write!(f, "testnet"),
            Network::Custom(s) => write!(f, "{}", s),
        }
    }
}
