use crate::test::{BacktraceMode, CoverageFormat, ReportFormat, TestConfig};
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
pub use ton_networks::{CustomNetworkUrls, Network};

#[derive(clap::ValueEnum, Debug, Copy, Clone)]
pub enum Explorer {
    Tonscan,
    Toncx,
    Dton,
    Tonviewer,
}

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
#[serde(rename_all = "kebab-case")]
pub struct CustomNetworkConfig {
    pub v2_url: String,
    pub v3_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActonConfig {
    pub package: PackageConfig,
    pub contracts: Option<ContractsConfig>,
    pub test: Option<TestSettings>,
    pub lint: Option<LintConfig>,
    pub fmt: Option<FmtSettings>,
    pub scripts: Option<BTreeMap<String, String>>,
    #[serde(skip)] // we build wallets manually
    pub wallets: Option<WalletsConfig>,
    #[serde(skip)] // we build libraries manually
    pub libraries: Option<LibrariesConfig>,
    pub mappings: Option<BTreeMap<String, String>>,
    pub networks: Option<BTreeMap<String, CustomNetworkConfig>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LibrariesConfig {
    #[serde(flatten)]
    pub libraries: BTreeMap<String, LibraryConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryConfig {
    pub name: String,
    pub hash: String,
    pub code: String,
    pub account: String,
    pub duration: u64,
    pub network: Network,
    pub timestamp: String,
    pub bits: u64,
    pub cells: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LibrariesFile {
    pub libraries: Option<LibrariesConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageConfig {
    pub name: String,
    pub description: String,
    pub version: String,
    pub repository: Option<String>,
    pub license: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct TestSettings {
    pub filter: Option<String>,
    pub reporter: Option<Vec<String>>,
    pub debug: Option<bool>,
    pub debug_port: Option<u16>,
    pub backtrace: Option<String>,
    pub coverage: Option<bool>,
    pub coverage_format: Option<String>,
    pub coverage_file: Option<String>,
    pub exclude: Option<Vec<String>>,
    pub include: Option<Vec<String>>,
    pub junit_path: Option<String>,
    pub junit_merge: Option<bool>,
    pub fork_net: Option<String>,
    pub api_key: Option<String>,
    pub fork_block_number: Option<u64>,
    pub mutation: Option<MutationConfig>,
    pub fail_fast: Option<bool>,
    pub ui: Option<bool>,
    pub ui_port: Option<u16>,
    #[serde(flatten)]
    pub metadata: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LintLevel {
    Allow,
    Warn,
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LintEntry {
    Level(LintLevel),
    Config(BTreeMap<String, LintLevel>),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LintConfig {
    #[serde(flatten)]
    pub entries: BTreeMap<String, LintEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct FmtSettings {
    pub width: Option<usize>,
    pub ignore: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct MutationConfig {
    pub disable_rules: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContractsConfig {
    #[serde(flatten)]
    pub contracts: BTreeMap<String, ContractConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletKeys {
    #[serde(rename = "mnemonic-env")]
    pub mnemonic_env: Option<String>,
    #[serde(rename = "mnemonic-file")]
    pub mnemonic_file: Option<String>,
    pub mnemonic: Option<String>,
    #[serde(rename = "mnemonic-keyring")]
    pub mnemonic_keyring: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletExpectedAddresses {
    #[serde(rename = "address-mainnet")]
    pub address_mainnet: Option<String>,
    #[serde(rename = "address-testnet")]
    pub address_testnet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletConfig {
    pub kind: String,
    pub workchain: Option<i32>,
    pub keys: WalletKeys,
    #[serde(default)]
    pub expected: Option<WalletExpectedAddresses>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WalletsConfig {
    #[serde(flatten)]
    pub wallets: BTreeMap<String, WalletConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WalletsFile {
    pub wallets: Option<WalletsConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
                repository: None,
                license: Some("MIT".to_string()),
            },
            test: None,
            lint: None,
            contracts: None,
            fmt: Some(FmtSettings {
                width: Some(100),
                ignore: Some(vec![]),
            }),
            wallets: None,
            libraries: None,
            scripts: None,
            mappings: None,
            networks: None,
        }
    }
}

impl std::fmt::Display for ContractDependency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContractDependency::Simple(name) | ContractDependency::Detailed { name, .. } => {
                write!(f, "{name}")
            }
        }
    }
}

impl ContractDependency {
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            ContractDependency::Simple(name) | ContractDependency::Detailed { name, .. } => name,
        }
    }

    #[must_use]
    pub fn kind(&self) -> DependencyKind {
        match self {
            ContractDependency::Simple(_) => DependencyKind::EmbedCode,
            ContractDependency::Detailed { kind, .. } => kind.clone(),
        }
    }

    #[must_use]
    pub fn compiled_code_function(&self) -> Option<&str> {
        match self {
            ContractDependency::Simple(_) => None,
            ContractDependency::Detailed { function, .. } => function.as_deref(),
        }
    }

    #[must_use]
    pub fn compiled_code_out_path(&self) -> Option<&str> {
        match self {
            ContractDependency::Simple(_) => None,
            ContractDependency::Detailed { path, .. } => path.as_deref(),
        }
    }
}

impl ContractConfig {
    #[must_use]
    pub fn dependency_names(&self) -> Vec<&str> {
        self.depends
            .as_ref()
            .map(|deps| deps.iter().map(ContractDependency::name).collect())
            .unwrap_or_default()
    }

    #[must_use]
    pub fn get_dependency(&self, name: &str) -> Option<&ContractDependency> {
        self.depends.as_ref()?.iter().find(|dep| dep.name() == name)
    }
}

impl ActonConfig {
    pub fn load() -> Result<Self> {
        let config_path = Path::new("Acton.toml");
        if !config_path.exists() {
            return Err(anyhow!(
                "Acton.toml not found. Run 'acton init' to initialize Acton in the project."
            ));
        }

        let content = fs::read_to_string(config_path)?;
        let mut config: ActonConfig = toml::from_str(&content)?;

        // Merge wallets from different sources
        // Order of importance (later overrides earlier):
        // 1. Global ~/.acton/wallets/global.wallets.toml
        // 2. Local wallets.toml

        let mut merged_wallets = BTreeMap::new();

        // 1. Load global wallets
        if let Some(global_path) = global_wallets_path()
            && global_path.exists()
        {
            let global_content = fs::read_to_string(&global_path)?;
            let global_wallets: WalletsFile = toml::from_str(&global_content)?;
            if let Some(wallets) = global_wallets.wallets {
                for (name, wallet) in wallets.wallets {
                    merged_wallets.insert(name, wallet);
                }
            }
        }

        // 2. Load local wallets.toml
        let local_wallets_path = Path::new("wallets.toml");
        if local_wallets_path.exists() {
            let local_content = fs::read_to_string(local_wallets_path)?;
            let local_wallets: WalletsFile = toml::from_str(&local_content)?;
            if let Some(wallets) = local_wallets.wallets {
                for (name, wallet) in wallets.wallets {
                    merged_wallets.insert(name, wallet);
                }
            }
        }

        config.wallets = Some(WalletsConfig {
            wallets: merged_wallets,
        });

        // Merge libraries from different sources
        let mut merged_libraries = BTreeMap::new();

        // 1. Load global libraries
        if let Some(global_path) = global_libraries_path()
            && global_path.exists()
        {
            let global_content = fs::read_to_string(&global_path)?;
            let global_libraries: LibrariesFile = toml::from_str(&global_content)?;
            if let Some(libraries) = global_libraries.libraries {
                for (name, library) in libraries.libraries {
                    merged_libraries.insert(name, library);
                }
            }
        }

        // 2. Load local libraries.toml
        let local_libraries_path = Path::new("libraries.toml");
        if local_libraries_path.exists() {
            let local_content = fs::read_to_string(local_libraries_path)?;
            let local_libraries: LibrariesFile = toml::from_str(&local_content)?;
            if let Some(libraries) = local_libraries.libraries {
                for (name, library) in libraries.libraries {
                    merged_libraries.insert(name, library);
                }
            }
        }

        config.libraries = Some(LibrariesConfig {
            libraries: merged_libraries,
        });

        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let content = toml::to_string_pretty(self)?;
        fs::write("Acton.toml", content)?;
        Ok(())
    }

    #[must_use]
    pub fn contracts(&self) -> Option<&BTreeMap<String, ContractConfig>> {
        self.contracts.as_ref().map(|c| &c.contracts)
    }

    #[must_use]
    pub fn get_contract(&self, name: &str) -> Option<&ContractConfig> {
        self.contracts.as_ref()?.contracts.get(name)
    }

    #[must_use]
    pub fn wallets(&self) -> Option<&BTreeMap<String, WalletConfig>> {
        self.wallets.as_ref().map(|w| &w.wallets)
    }

    #[must_use]
    pub fn get_wallet(&self, name: &str) -> Option<&WalletConfig> {
        self.wallets.as_ref()?.wallets.get(name)
    }

    #[must_use]
    pub fn libraries(&self) -> Option<&BTreeMap<String, LibraryConfig>> {
        self.libraries.as_ref().map(|l| &l.libraries)
    }

    #[must_use]
    pub fn get_library(&self, name: &str) -> Option<&LibraryConfig> {
        self.libraries.as_ref()?.libraries.get(name)
    }

    #[must_use]
    pub fn custom_networks(&self) -> std::collections::HashMap<String, CustomNetworkUrls> {
        let mut result = std::collections::HashMap::new();
        if let Some(networks) = &self.networks {
            for (name, config) in networks {
                result.insert(
                    name.clone(),
                    CustomNetworkUrls {
                        v2_url: Arc::from(config.v2_url.as_str()),
                        v3_url: config.v3_url.as_ref().map(|s| Arc::from(s.as_str())),
                    },
                );
            }
        }
        result
    }
}

#[must_use]
pub fn global_wallets_path() -> Option<PathBuf> {
    #[cfg(windows)]
    let home = std::env::var("USERPROFILE").ok()?;
    #[cfg(not(windows))]
    let home = std::env::var("HOME").ok()?;

    Some(
        PathBuf::from(home)
            .join(".acton")
            .join("wallets")
            .join("global.wallets.toml"),
    )
}

#[must_use]
pub fn global_libraries_path() -> Option<PathBuf> {
    #[cfg(windows)]
    let home = std::env::var("USERPROFILE").ok()?;
    #[cfg(not(windows))]
    let home = std::env::var("HOME").ok()?;

    Some(
        PathBuf::from(home)
            .join(".acton")
            .join("libraries")
            .join("global.libraries.toml"),
    )
}

impl TestSettings {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn to_test_config(
        &self,
        filter_override: Option<String>,
        report_formats: Vec<ReportFormat>,
        debug_override: Option<bool>,
        debug_port_override: Option<u16>,
        backtrace_override: Option<BacktraceMode>,
        coverage_override: Option<bool>,
        coverage_format_override: Option<CoverageFormat>,
        coverage_file_override: Option<String>,
        exclude_override: Option<Vec<String>>,
        include_override: Option<Vec<String>>,
        clear_cache_override: Option<bool>,
        junit_path_override: Option<String>,
        junit_merge_override: bool,
        snapshot_override: Option<String>,
        baseline_gas_override: Option<String>,
        fork_net_override: Option<Network>,
        api_key_override: Option<String>,
        fork_block_number_override: Option<u64>,
        save_test_trace_override: Option<String>,
        mutate_override: bool,
        mutate_overrides_override: Option<String>,
        mutate_contract_override: Option<String>,
        disable_rules_override: Vec<String>,
        fail_fast_override: Option<bool>,
        ui_override: bool,
        ui_port_override: Option<u16>,
    ) -> TestConfig {
        let mut final_report_formats = Vec::new();

        if report_formats.is_empty() {
            // process config reporters only if no cli reporters provided
            if let Some(reporters) = &self.reporter {
                for reporter in reporters {
                    match reporter.to_lowercase().as_str() {
                        "console" => final_report_formats.push(ReportFormat::Console),
                        "teamcity" => final_report_formats.push(ReportFormat::TeamCity),
                        "junit" => final_report_formats.push(ReportFormat::JUnit),
                        "dot" => final_report_formats.push(ReportFormat::Dot),
                        _ => {} // skip unknown reporters
                    }
                }
            }
        } else {
            final_report_formats = report_formats;
        }

        TestConfig {
            filter: filter_override.or_else(|| self.filter.clone()),
            report_formats: final_report_formats,
            debug: debug_override.unwrap_or_else(|| self.debug.unwrap_or(false)),
            debug_port: debug_port_override.unwrap_or_else(|| self.debug_port.unwrap_or(12345)),
            backtrace: backtrace_override.or_else(|| {
                self.backtrace
                    .as_ref()
                    .and_then(|b| match b.to_lowercase().as_str() {
                        "full" => Some(BacktraceMode::Full),
                        _ => None,
                    })
            }),
            coverage: coverage_override.unwrap_or_else(|| self.coverage.unwrap_or(false)),
            coverage_format: coverage_format_override.or_else(|| {
                self.coverage_format
                    .as_ref()
                    .and_then(|f| match f.to_lowercase().as_str() {
                        "lcov" => Some(CoverageFormat::Lcov),
                        "text" => Some(CoverageFormat::Text),
                        _ => None,
                    })
            }),
            coverage_file: coverage_file_override.or_else(|| self.coverage_file.clone()),
            exclude_patterns: exclude_override
                .unwrap_or_else(|| self.exclude.clone().unwrap_or_default()),
            include_patterns: include_override
                .unwrap_or_else(|| self.include.clone().unwrap_or_default()),
            clear_cache: clear_cache_override.unwrap_or(false),
            junit_path: if self.junit_path == Some("test-results".to_owned()) {
                junit_path_override
            } else {
                Some(
                    self.junit_path
                        .clone()
                        .unwrap_or_else(|| junit_path_override.unwrap_or_default()),
                )
            },
            junit_merge: junit_merge_override || self.junit_merge.unwrap_or(false),
            snapshot: snapshot_override,
            baseline_snapshot: baseline_gas_override,
            fork_net: fork_net_override.or_else(|| {
                self.fork_net
                    .as_ref()
                    .and_then(|n| match n.to_lowercase().as_str() {
                        "mainnet" => Some(Network::Mainnet),
                        "testnet" => Some(Network::Testnet),
                        _ => None,
                    })
            }),
            api_key: api_key_override.or_else(|| self.api_key.clone()),
            fork_block_number: fork_block_number_override.or(self.fork_block_number),
            save_test_trace: save_test_trace_override,
            mutate: mutate_override,
            mutate_overrides: mutate_overrides_override,
            mutate_contract: mutate_contract_override,
            disable_rules: if disable_rules_override.is_empty() {
                self.mutation
                    .as_ref()
                    .and_then(|m| m.disable_rules.clone())
                    .unwrap_or_default()
            } else {
                disable_rules_override
            },
            fail_fast: fail_fast_override.unwrap_or_else(|| self.fail_fast.unwrap_or(false)),
            ui: ui_override || self.ui.unwrap_or(false),
            ui_port: ui_port_override.unwrap_or_else(|| self.ui_port.unwrap_or(12344)),
        }
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
    fn test_lint_config_parsing() {
        let toml_content = r#"
[package]
name = "test-project"
description = "Test project"
version = "0.1.0"

[lint]
unused-variable = "deny"
mutable-variable-can-be-immutable = "warn"

[lint.counter]
unused-variable = "allow"
"#;

        let config: ActonConfig = toml::from_str(toml_content).unwrap();
        let lint = config.lint.as_ref().unwrap();

        match lint.entries.get("unused-variable").unwrap() {
            LintEntry::Level(level) => assert_eq!(*level, LintLevel::Deny),
            _ => panic!("Expected level"),
        }

        match lint
            .entries
            .get("mutable-variable-can-be-immutable")
            .unwrap()
        {
            LintEntry::Level(level) => assert_eq!(*level, LintLevel::Warn),
            _ => panic!("Expected level"),
        }

        match lint.entries.get("counter").unwrap() {
            LintEntry::Config(config) => {
                assert_eq!(*config.get("unused-variable").unwrap(), LintLevel::Allow);
            }
            _ => panic!("Expected config"),
        }
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

    #[test]
    fn test_wallet_config_parsing() -> Result<()> {
        let toml_content = r#"
[wallets.deployer]
kind = "v4R2"
workchain = 0
keys = { mnemonic-env = "DEPLOYER_MNEMONIC" }

[wallets.deployer.expected]
address-mainnet = "EQC2jeGorIAFh2LXwsDjHfRK-GSo9UzchdIEMh24A7T7AHot"
address-testnet = "EQD_testnet_address_here"

[wallets.user]
kind = "v5R1"
workchain = -1
keys = { mnemonic-file = "user-keys.txt" }

[wallets.direct]
kind = "v4R2"
keys = { mnemonic = "word1 word2 word3" }
"#;

        let wallets_file: WalletsFile = toml::from_str(toml_content)?;
        let wallets = wallets_file.wallets.unwrap().wallets;
        assert_eq!(wallets.len(), 3);

        let deployer = wallets.get("deployer").unwrap();
        assert_eq!(deployer.kind, "v4R2");
        assert_eq!(deployer.workchain, Some(0));
        assert_eq!(
            deployer.keys.mnemonic_env,
            Some("DEPLOYER_MNEMONIC".to_string())
        );
        assert_eq!(deployer.keys.mnemonic_file, None);

        // Check expected addresses
        let expected = deployer.expected.as_ref().unwrap();
        assert_eq!(
            expected.address_mainnet,
            Some("EQC2jeGorIAFh2LXwsDjHfRK-GSo9UzchdIEMh24A7T7AHot".to_string())
        );
        assert_eq!(
            expected.address_testnet,
            Some("EQD_testnet_address_here".to_string())
        );

        let user = wallets.get("user").unwrap();
        assert_eq!(user.kind, "v5R1");
        assert_eq!(user.workchain, Some(-1));
        assert_eq!(user.keys.mnemonic_file, Some("user-keys.txt".to_string()));
        assert_eq!(user.keys.mnemonic_env, None);
        assert!(user.expected.is_none());

        let direct = wallets.get("direct").unwrap();
        assert_eq!(direct.kind, "v4R2");
        assert_eq!(direct.keys.mnemonic, Some("word1 word2 word3".to_string()));

        Ok(())
    }

    #[test]
    fn test_mappings_parsing() {
        let toml_content = r#"
[package]
name = "test-project"
description = "Test project"
version = "0.1.0"

[mappings]
core = "./core"
utils = "/usr/local/lib/tolk/utils"
"#;

        let config: ActonConfig = toml::from_str(toml_content).unwrap();
        let mappings = config.mappings.as_ref().unwrap();
        assert_eq!(mappings.get("core").unwrap(), "./core");
        assert_eq!(mappings.get("utils").unwrap(), "/usr/local/lib/tolk/utils");
    }
}
