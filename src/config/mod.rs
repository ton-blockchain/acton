use crate::commands::test::{ReportFormat, TestConfig};
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq, Default)]
pub enum DependencyKind {
    #[serde(rename = "embed_code")]
    #[default]
    EmbedCode,
    #[serde(rename = "library_ref")]
    LibraryRef,
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
#[serde(untagged)]
pub enum ContractDependency {
    Simple(String),
    Detailed {
        name: String,
        #[serde(default)]
        kind: DependencyKind,
        function: Option<String>,
        path: Option<String>,
    },
}

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
    pub reporter: Option<Vec<String>>,
    pub debug: Option<bool>,
    pub debug_port: Option<u16>,
    pub backtrace: Option<String>,
    pub coverage: Option<bool>,
    pub coverage_format: Option<String>,
    pub exclude: Option<Vec<String>>,
    pub include: Option<Vec<String>>,
    pub junit_path: Option<String>,
    pub junit_merge: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContractsConfig {
    #[serde(flatten)]
    pub contracts: HashMap<String, ContractConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractConfig {
    pub name: String,
    pub src: String,
    pub depends: Option<Vec<ContractDependency>>,
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

impl std::fmt::Display for ContractDependency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContractDependency::Simple(name) => write!(f, "{}", name),
            ContractDependency::Detailed { name, .. } => write!(f, "{}", name),
        }
    }
}

impl ContractDependency {
    pub fn name(&self) -> &str {
        match self {
            ContractDependency::Simple(name) => name,
            ContractDependency::Detailed { name, .. } => name,
        }
    }

    pub fn kind(&self) -> DependencyKind {
        match self {
            ContractDependency::Simple(_) => DependencyKind::EmbedCode,
            ContractDependency::Detailed { kind, .. } => kind.clone(),
        }
    }

    pub fn compiled_code_function(&self) -> Option<&str> {
        match self {
            ContractDependency::Simple(_) => None,
            ContractDependency::Detailed { function, .. } => function.as_deref(),
        }
    }

    pub fn compiled_code_out_path(&self) -> Option<&str> {
        match self {
            ContractDependency::Simple(_) => None,
            ContractDependency::Detailed { path, .. } => path.as_deref(),
        }
    }
}

impl ContractConfig {
    pub fn dependency_names(&self) -> Vec<&str> {
        self.depends
            .as_ref()
            .map(|deps| deps.iter().map(|dep| dep.name()).collect())
            .unwrap_or_default()
    }

    pub fn get_dependency(&self, name: &str) -> Option<&ContractDependency> {
        self.depends.as_ref()?.iter().find(|dep| dep.name() == name)
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
src = "counter.tolk"
depends = []

[contracts.wallet-v5]
name = "Wallet V5"
src = "wallet-v5.tolk"
depends = []
"#;

        let config: ActonConfig = toml::from_str(toml_content).unwrap();
        assert_eq!(config.package.name, "test-project");

        let contracts = config.contracts().unwrap();
        assert_eq!(contracts.len(), 2);

        let counter = config.get_contract("counter").unwrap();
        assert_eq!(counter.name, "Counter Contract");
        assert_eq!(counter.src, "counter.tolk");
        assert_eq!(counter.depends, Some(vec![]));

        let wallet = config.get_contract("wallet-v5").unwrap();
        assert_eq!(wallet.name, "Wallet V5");
        assert_eq!(wallet.src, "wallet-v5.tolk");
        assert_eq!(wallet.depends, Some(vec![]));
    }

    #[test]
    fn test_test_config_parsing() {
        let toml_content = r#"
[package]
name = "test-project"
description = "Test project"
version = "0.1.0"

[test]
filter = "test-unit.*"
reporter = ["console", "junit"]
debug = true
debug-port = 9999
backtrace = "full"
coverage = true
coverage-format = "lcov"
exclude = ["**/integration/**"]
include = ["**/unit/**"]
junit-path = "custom-reports"
junit-merge = true
"#;

        let config: ActonConfig = toml::from_str(toml_content).unwrap();

        let test_settings = config.test.as_ref().unwrap();
        assert_eq!(test_settings.filter, Some("test-unit.*".to_string()));
        assert_eq!(
            test_settings.reporter,
            Some(vec!["console".to_string(), "junit".to_string()])
        );
        assert_eq!(test_settings.debug, Some(true));
        assert_eq!(test_settings.debug_port, Some(9999));
        assert_eq!(test_settings.backtrace, Some("full".to_string()));
        assert_eq!(test_settings.coverage, Some(true));
        assert_eq!(test_settings.coverage_format, Some("lcov".to_string()));
        assert_eq!(
            test_settings.exclude,
            Some(vec!["**/integration/**".to_string()])
        );
        assert_eq!(test_settings.include, Some(vec!["**/unit/**".to_string()]));
        assert_eq!(test_settings.junit_path, Some("custom-reports".to_string()));
        assert_eq!(test_settings.junit_merge, Some(true));
    }
}

impl TestSettings {
    pub fn to_test_config(
        &self,
        filter_override: Option<String>,
        debug_override: Option<bool>,
        debug_port_override: Option<u16>,
        backtrace_override: Option<String>,
        coverage_override: Option<bool>,
        coverage_format_override: Option<String>,
        exclude_override: Option<Vec<String>>,
        include_override: Option<Vec<String>>,
        clear_cache_override: Option<bool>,
        junit_path_override: Option<String>,
        junit_merge_override: bool,
    ) -> TestConfig {
        let mut report_formats = Vec::new();
        if let Some(reporters) = &self.reporter {
            for reporter in reporters {
                match reporter.to_lowercase().as_str() {
                    "console" => report_formats.push(ReportFormat::Console),
                    "teamcity" => report_formats.push(ReportFormat::TeamCity),
                    "junit" => report_formats.push(ReportFormat::JUnit),
                    "dot" => report_formats.push(ReportFormat::Dot),
                    _ => {} // skip unknown reporters
                }
            }
        }

        TestConfig {
            filter: filter_override.or_else(|| self.filter.clone()),
            report_formats,
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
            junit_path: if self.junit_path != Some("test-results".to_owned()) {
                Some(
                    self.junit_path
                        .clone()
                        .unwrap_or(junit_path_override.unwrap_or("".to_owned())),
                )
            } else {
                junit_path_override
            },
            junit_merge: junit_merge_override || self.junit_merge.unwrap_or(false),
        }
    }
}
