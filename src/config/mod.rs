use crate::commands::test::TestConfig;
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActonConfig {
    pub package: PackageConfig,
    pub test: Option<TestSettings>,
    pub contracts: Option<ContractsConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageConfig {
    pub name: String,
    pub description: String,
    pub version: String,
    pub license: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct TestSettings {
    pub filter: Option<String>,
    pub teamcity: Option<bool>,
    pub debug: Option<bool>,
    pub debug_port: Option<u16>,
    pub backtrace: Option<String>,
    pub coverage: Option<bool>,
    pub coverage_format: Option<String>,
    pub exclude: Option<Vec<String>>,
    pub include: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContractsConfig {
    #[serde(flatten)]
    pub contracts: HashMap<String, ContractConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractConfig {
    pub name: String,
    pub root: String,
    pub depends: Option<Vec<String>>,
    pub output: Option<String>,
}

impl Default for ActonConfig {
    fn default() -> Self {
        Self {
            package: PackageConfig {
                name: "my-acton-project".to_string(),
                description: "A TON blockchain project".to_string(),
                version: "0.1.0".to_string(),
                license: Some("MIT".to_string()),
            },
            test: None,
            contracts: None,
        }
    }
}

impl ActonConfig {
    pub fn load() -> Result<Self> {
        let config_path = Path::new("Acton.toml");
        if !config_path.exists() {
            return Err(anyhow!(
                "Acton.toml not found. Run 'acton init' to create a new project."
            ));
        }

        let content = fs::read_to_string(config_path)?;
        let config: ActonConfig = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let content = toml::to_string_pretty(self)?;
        fs::write("Acton.toml", content)?;
        Ok(())
    }

    pub fn contracts(&self) -> Option<&HashMap<String, ContractConfig>> {
        self.contracts.as_ref().map(|c| &c.contracts)
    }

    pub fn get_contract(&self, name: &str) -> Option<&ContractConfig> {
        self.contracts.as_ref()?.contracts.get(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_parsing() {
        let toml_content = r#"
[package]
name = "test-project"
description = "Test project"
version = "0.1.0"

[contracts.counter]
name = "Counter Contract"
root = "counter.tolk"
depends = []

[contracts.wallet-v5]
name = "Wallet V5"
root = "wallet-v5.tolk"
depends = []
"#;

        let config: ActonConfig = toml::from_str(toml_content).unwrap();
        assert_eq!(config.package.name, "test-project");

        let contracts = config.contracts().unwrap();
        assert_eq!(contracts.len(), 2);

        let counter = config.get_contract("counter").unwrap();
        assert_eq!(counter.name, "Counter Contract");
        assert_eq!(counter.root, "counter.tolk");
        assert_eq!(counter.depends, Some(vec![]));

        let wallet = config.get_contract("wallet-v5").unwrap();
        assert_eq!(wallet.name, "Wallet V5");
        assert_eq!(wallet.root, "wallet-v5.tolk");
        assert_eq!(wallet.depends, Some(vec![]));
    }
}

impl TestSettings {
    pub fn to_test_config(
        &self,
        filter_override: Option<String>,
        teamcity_override: Option<bool>,
        debug_override: Option<bool>,
        debug_port_override: Option<u16>,
        backtrace_override: Option<String>,
        coverage_override: Option<bool>,
        coverage_format_override: Option<String>,
        exclude_override: Option<Vec<String>>,
        include_override: Option<Vec<String>>,
        clear_cache_override: Option<bool>,
    ) -> TestConfig {
        TestConfig {
            filter: filter_override.or_else(|| self.filter.clone()),
            teamcity: teamcity_override.unwrap_or_else(|| self.teamcity.unwrap_or(false)),
            debug: debug_override.unwrap_or_else(|| self.debug.unwrap_or(false)),
            debug_port: debug_port_override.unwrap_or_else(|| self.debug_port.unwrap_or(12345)),
            backtrace: backtrace_override.or_else(|| self.backtrace.clone()),
            coverage: coverage_override.unwrap_or_else(|| self.coverage.unwrap_or(false)),
            coverage_format: coverage_format_override.or_else(|| self.coverage_format.clone()),
            exclude_patterns: exclude_override
                .unwrap_or_else(|| self.exclude.clone().unwrap_or_default()),
            include_patterns: include_override
                .unwrap_or_else(|| self.include.clone().unwrap_or_default()),
            clear_cache: clear_cache_override.unwrap_or(false),
        }
    }
}
