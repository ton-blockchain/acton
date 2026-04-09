use crate::test::{
    BacktraceMode, CoverageFormat, MutationDiffMode, MutationLevel, ReportFormat, TestConfig,
};
use anyhow::{Result, anyhow};
use path_absolutize::Absolutize;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
pub use ton_networks::{CustomNetworkUrls, Network};

static MANIFEST_PATH: OnceLock<PathBuf> = OnceLock::new();
static PROJECT_ROOT: OnceLock<PathBuf> = OnceLock::new();
static MANIFEST_PATH_SOURCE: OnceLock<ResolutionSource> = OnceLock::new();
static PROJECT_ROOT_SOURCE: OnceLock<ResolutionSource> = OnceLock::new();
pub const DEFAULT_PROJECT_MAPPINGS: &[(&str, &str)] = &[
    ("acton", ".acton"),
    ("contracts", "contracts"),
    ("tests", "tests"),
    ("wrappers", "wrappers"),
    ("gen", "gen"),
];

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ResolutionSource {
    ProjectRootFlag,
    ManifestPathFlag,
    AutoDetected,
    FallbackCwd,
}

impl ResolutionSource {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProjectRootFlag => "--project-root",
            Self::ManifestPathFlag => "--manifest-path",
            Self::AutoDetected => "auto-detected",
            Self::FallbackCwd => "fallback-cwd",
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ResolvedPathsDiagnostics {
    pub project_root: PathBuf,
    pub manifest_path: PathBuf,
    pub project_root_source: ResolutionSource,
    pub manifest_path_source: ResolutionSource,
}

#[derive(clap::ValueEnum, Debug, Copy, Clone)]
pub enum Explorer {
    Tonscan,
    Toncx,
    Dton,
    Tonviewer,
}

/// Output format for `acton check` diagnostics
#[derive(
    clap::ValueEnum, Debug, Clone, Serialize, Deserialize, JsonSchema, Hash, Eq, PartialEq, Default,
)]
#[serde(rename_all = "kebab-case")]
pub enum CheckOutputFormat {
    /// Human-readable plain output
    #[default]
    Plain,
    /// Structured JSON output
    Json,
    /// SARIF output for code scanning tools
    Sarif,
    /// GitHub workflow command output
    Github,
    /// GitLab code quality output
    Gitlab,
}

/// How a compiled dependency is linked into a contract
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Hash, Eq, PartialEq, Default)]
pub enum DependencyKind {
    /// Embed dependency code directly into the output
    #[serde(rename = "embed_code")]
    #[default]
    EmbedCode,
    /// Reference the dependency as an on-chain library
    #[serde(rename = "library_ref")]
    LibraryRef,
}

/// Dependency declaration for a contract
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Hash, Eq, PartialEq)]
#[serde(untagged)]
pub enum ContractDependency {
    /// Name of the contract to depend on in the simple form
    Simple(String),
    /// Detailed dependency configuration
    Detailed {
        /// Name of the contract to depend on
        name: String,
        #[serde(default)]
        /// Dependency type
        kind: DependencyKind,
        /// Custom name for the generated code function
        function: Option<String>,
        /// Custom output path for the generated code file
        path: Option<String>,
    },
}

/// `TonCenter` API endpoints for a custom network
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct CustomNetworkApiConfig {
    /// The URL for the `TonCenter` API v2. For localnet this defaults to
    /// `http://localhost:<litenode.port>/api/v2` with `5411` as the fallback port
    pub v2: Option<String>,
    /// The URL for the `TonCenter` API v3. For localnet this defaults to
    /// `http://localhost:<litenode.port>/api/v3` with `5411` as the fallback port
    pub v3: Option<String>,
}

/// Custom network configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct CustomNetworkConfig {
    /// Base URL used to build transaction links for this network. Acton appends
    /// `/tx/<hash>` automatically and derives links from `api.v2` when omitted
    pub explorer: Option<String>,
    /// `TonCenter` API endpoints for this network
    pub api: Option<CustomNetworkApiConfig>,
}

/// JSON schema for Acton.toml configuration file
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(title = "Acton Configuration Schema")]
pub struct ActonConfig {
    /// Package metadata for the Acton project
    pub package: PackageConfig,
    /// Definition of contracts in the project
    pub contracts: Option<ContractsConfig>,
    /// Default settings for the test runner
    pub test: Option<TestSettings>,
    /// Linter configuration for the project
    pub lint: Option<LintConfig>,
    /// Settings for the Tolk code formatter
    pub fmt: Option<FmtSettings>,
    /// Default settings for the build command
    pub build: Option<BuildSettings>,
    /// Default settings for wrapper generation
    pub wrappers: Option<WrappersConfig>,
    /// Default settings for `acton litenode` commands
    pub litenode: Option<LitenodeSettings>,
    /// Custom scripts that can be run with `acton run`
    pub scripts: Option<BTreeMap<String, String>>,
    #[serde(skip)] // we build wallets manually
    pub wallets: Option<WalletsConfig>,
    #[serde(skip)] // we build libraries manually
    pub libraries: Option<LibrariesConfig>,
    /// Path mappings for Tolk compiler imports, for example mapping `"core" = "./foo/core"`
    /// so imports can use `@core/...`
    pub mappings: Option<BTreeMap<String, String>>,
    /// Custom network configurations
    pub networks: Option<BTreeMap<String, CustomNetworkConfig>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LibrariesConfig {
    #[serde(flatten)]
    pub libraries: BTreeMap<String, LibraryConfig>,
}

impl JsonSchema for LibrariesConfig {
    fn schema_name() -> String {
        "LibrariesConfig".to_string()
    }

    fn json_schema(generator: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        <BTreeMap<String, LibraryConfig>>::json_schema(generator)
    }
}

/// A deployed library entry from `libraries.toml`
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LibraryConfig {
    /// Logical library name
    pub name: String,
    /// Library hash
    pub hash: String,
    /// Library code encoded as BOC
    pub code: String,
    /// Account address that stores the library
    pub account: String,
    /// Remaining deployment duration in seconds
    pub duration: u64,
    /// Network where the library is deployed
    pub network: Network,
    /// Initial deployment timestamp
    pub timestamp: String,
    /// Last top-up timestamp
    pub last_topup_timestamp: String,
    /// Number of bits in the serialized library cell tree
    pub bits: u64,
    /// Number of cells in the serialized library cell tree
    pub cells: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct LibrariesFile {
    pub libraries: Option<LibrariesConfig>,
}

/// Package metadata for the Acton project
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PackageConfig {
    /// The name of the project
    pub name: String,
    /// A short description of the project
    pub description: String,
    /// The current version of the project
    pub version: String,
    /// The URL of the project's repository
    pub repository: Option<String>,
    /// The project's license identifier
    pub license: Option<String>,
}

/// Coverage settings for the test runner
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "kebab-case")]
pub struct TestCoverageSettings {
    /// Enable code coverage reporting
    pub enabled: Option<bool>,
    /// Format for coverage reports
    #[schemars(with = "Option<CoverageFormat>")]
    #[schemars(default = "default_test_coverage_format")]
    pub format: Option<String>,
    /// Path to save the coverage report
    pub output_file: Option<String>,
    /// Minimum total line coverage percentage required for a non-UI coverage run
    pub minimum_percent: Option<f64>,
    /// Include files from the `@wrappers` mapping in coverage reports
    pub include_wrappers: Option<bool>,
    /// Include `.test.tolk` files in coverage reports
    pub include_tests: Option<bool>,
}

/// Fuzz settings for parameterized tests marked with `@test({ fuzz: ... })`
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "kebab-case")]
pub struct TestFuzzSettings {
    /// Number of accepted fuzz cases to execute for each fuzz test
    pub runs: Option<usize>,
    /// Maximum number of rejected inputs from `assume(...)` before the test fails
    pub max_test_rejects: Option<usize>,
    /// Seed used for reproducible fuzz input generation
    pub seed: Option<u64>,
}

/// Default settings for the test runner
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "kebab-case")]
pub struct TestSettings {
    /// Regex pattern to filter test names
    pub filter: Option<String>,
    /// List of test reporters to use
    #[schemars(with = "Option<Vec<ReportFormat>>")]
    pub reporter: Option<Vec<String>>,
    /// Enable debug mode for tests
    pub debug: Option<bool>,
    /// Port for the debug server
    #[schemars(default = "default_test_debug_port", range(max = 65535))]
    pub debug_port: Option<u16>,
    /// Enable stack traces for failed tests
    #[schemars(with = "Option<BacktraceMode>")]
    pub backtrace: Option<String>,
    /// Coverage settings for test runs
    pub coverage: Option<TestCoverageSettings>,
    /// Default fuzz settings for parameterized tests
    pub fuzz: Option<TestFuzzSettings>,
    /// Glob patterns to exclude from testing
    pub exclude: Option<Vec<String>>,
    /// Glob patterns to include in testing
    pub include: Option<Vec<String>>,
    /// Directory for `JUnit` XML reports
    pub junit_path: Option<String>,
    /// Merge all test suites into a single `JUnit` file
    pub junit_merge: Option<bool>,
    /// Network to fork for testing
    #[schemars(with = "Option<Network>")]
    pub fork_net: Option<String>,
    /// API key for the network provider when forking
    pub api_key: Option<String>,
    /// Specific block number to fork from
    pub fork_block_number: Option<u64>,
    /// Configuration for mutation testing
    pub mutation: Option<MutationConfig>,
    /// Stop test execution after the first failure
    pub fail_fast: Option<bool>,
    /// Exit with a non-zero code when profiling differs from baseline
    pub fail_on_diff: Option<bool>,
    /// Enable the test UI server
    pub ui: Option<bool>,
    /// Port for the test UI server
    #[schemars(range(max = 65535))]
    pub ui_port: Option<u16>,
    #[schemars(with = "BTreeMap<String, serde_json::Value>")]
    #[serde(flatten)]
    pub metadata: BTreeMap<String, toml::Value>,
}

const fn default_test_debug_port() -> Option<u16> {
    Some(12345)
}

fn default_test_coverage_format() -> Option<String> {
    Some("lcov".to_string())
}

/// Lint severity level
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LintLevel {
    /// Disable the rule
    Allow,
    /// Emit warnings for the rule
    Warn,
    /// Treat the rule as an error
    Deny,
}

/// Lint rule configuration, either a global level or contract-specific overrides
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum LintEntry {
    /// Global lint level for a rule
    Level(LintLevel),
    /// Contract-specific lint overrides
    Config(BTreeMap<String, LintLevel>),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LintRules {
    #[serde(flatten)]
    pub entries: BTreeMap<String, LintEntry>,
}

impl JsonSchema for LintRules {
    fn schema_name() -> String {
        "LintRules".to_string()
    }

    fn json_schema(generator: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        <BTreeMap<String, LintEntry>>::json_schema(generator)
    }
}

const fn default_max_warnings() -> usize {
    usize::MAX
}

/// Linter configuration for the project
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub struct LintConfig {
    /// Glob patterns for files to exclude from lint diagnostics
    pub exclude: Option<Vec<String>>,
    #[serde(default = "default_max_warnings")]
    /// Maximum allowed warning count before `acton check` exits with a non-zero code
    pub max_warnings: usize,
    /// Output format for `acton check` diagnostics
    pub output_format: Option<CheckOutputFormat>,
    /// Lint rules and contract-specific overrides
    pub rules: Option<LintRules>,
    #[schemars(with = "BTreeMap<String, serde_json::Value>")]
    #[serde(flatten)]
    pub metadata: BTreeMap<String, toml::Value>,
}

impl Default for LintConfig {
    fn default() -> Self {
        Self {
            exclude: None,
            max_warnings: default_max_warnings(),
            output_format: None,
            rules: None,
            metadata: BTreeMap::new(),
        }
    }
}

/// Settings for the Tolk code formatter
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "kebab-case")]
pub struct FmtSettings {
    /// Maximum line width for formatting
    #[schemars(default = "default_fmt_width")]
    pub width: Option<usize>,
    /// Glob patterns to ignore from formatting
    pub ignore: Option<Vec<String>>,
    /// Insert an empty line between import groups (`@stdlib`, `@acton`, `@<other>`, `../`, `./`)
    #[schemars(default = "default_fmt_separate_import_groups")]
    pub separate_import_groups: Option<bool>,
}

const fn default_fmt_width() -> Option<usize> {
    Some(100)
}

const fn default_fmt_separate_import_groups() -> Option<bool> {
    Some(false)
}

/// Default settings for the build command
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "kebab-case")]
pub struct BuildSettings {
    /// Directory where JSON build artifacts are saved
    pub out_dir: Option<String>,
    /// Directory where generated dependency files are saved
    pub gen_dir: Option<String>,
    /// Directory where per-contract compiled Fift files are saved
    pub output_fift: Option<String>,
}

/// Default settings for wrapper generation
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct WrappersConfig {
    /// Default settings for Tolk wrapper generation
    pub tolk: Option<TolkWrapperSettings>,
    /// Default settings for TypeScript wrapper generation
    pub typescript: Option<TypescriptWrapperSettings>,
}

/// Default settings for Tolk wrapper generation
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "kebab-case")]
pub struct TolkWrapperSettings {
    /// Directory where `acton wrapper` writes generated Tolk wrappers by default
    pub output_dir: Option<String>,
    /// Generate a Tolk test stub by default for `acton wrapper`
    pub generate_test: Option<bool>,
    /// Directory where generated Tolk test stubs are written by default
    pub test_output_dir: Option<String>,
}

/// Default settings for TypeScript wrapper generation
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "kebab-case")]
pub struct TypescriptWrapperSettings {
    /// Directory where `acton wrapper --ts` writes generated TypeScript wrappers by default
    pub output_dir: Option<String>,
}

/// Default settings for `acton litenode` commands
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "kebab-case")]
pub struct LitenodeSettings {
    /// Litenode port used by `acton litenode` commands
    #[schemars(default = "default_litenode_port", range(max = 65535))]
    pub port: Option<u16>,
    /// Network to fork from used by `acton litenode start`
    #[schemars(with = "Option<Network>")]
    pub fork_net: Option<String>,
    /// Block sequence number used by `acton litenode start` when forking from historical state
    pub fork_block_number: Option<u64>,
    /// Wallet names from `[wallets]` that are automatically funded and deployed on
    /// `acton litenode start`
    pub accounts: Option<Vec<String>>,
    /// Maximum number of API requests per second served by `LiteNode` `/api` endpoints
    pub rate_limit: Option<u32>,
}

const fn default_litenode_port() -> Option<u16> {
    Some(3000)
}

/// Configuration for mutation testing
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "kebab-case")]
pub struct MutationConfig {
    /// List of mutation rules to disable
    pub disable_rules: Option<Vec<String>>,
    /// Path to a JSON file with custom query-based mutation rules
    pub rules_file: Option<String>,
    /// List of mutation levels to run
    pub mutation_levels: Option<Vec<MutationLevel>>,
    /// Minimum mutation score percentage required for the run to succeed
    pub minimum_percent: Option<f64>,
    /// Diff scope used to limit mutation testing to changed lines
    pub diff: Option<MutationDiffMode>,
    /// Base ref used by diff-based mutation testing modes
    pub diff_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContractsConfig {
    #[serde(flatten)]
    pub contracts: BTreeMap<String, ContractConfig>,
}

impl JsonSchema for ContractsConfig {
    fn schema_name() -> String {
        "ContractsConfig".to_string()
    }

    fn json_schema(generator: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        <BTreeMap<String, ContractConfig>>::json_schema(generator)
    }
}

/// Wallet seed sources
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WalletKeys {
    /// Environment variable that contains the wallet mnemonic
    #[serde(rename = "mnemonic-env")]
    pub mnemonic_env: Option<String>,
    /// File path that stores the wallet mnemonic
    #[serde(rename = "mnemonic-file")]
    pub mnemonic_file: Option<String>,
    /// Wallet mnemonic stored directly in the config
    pub mnemonic: Option<String>,
    /// Keyring entry that stores the wallet mnemonic
    #[serde(rename = "mnemonic-keyring")]
    pub mnemonic_keyring: Option<String>,
}

/// Expected wallet addresses for different networks
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WalletExpectedAddresses {
    /// Expected mainnet address
    #[serde(rename = "address-mainnet")]
    pub address_mainnet: Option<String>,
    /// Expected testnet address
    #[serde(rename = "address-testnet")]
    pub address_testnet: Option<String>,
}

/// Wallet configuration entry
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WalletConfig {
    /// Wallet contract type
    pub kind: String,
    /// Workchain for the wallet address
    pub workchain: Option<i32>,
    /// Mnemonic and key storage configuration
    pub keys: WalletKeys,
    #[serde(default)]
    /// Expected wallet addresses for supported networks
    pub expected: Option<WalletExpectedAddresses>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WalletsConfig {
    #[serde(flatten)]
    pub wallets: BTreeMap<String, WalletConfig>,
}

impl JsonSchema for WalletsConfig {
    fn schema_name() -> String {
        "WalletsConfig".to_string()
    }

    fn json_schema(generator: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        <BTreeMap<String, WalletConfig>>::json_schema(generator)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct WalletsFile {
    pub wallets: Option<WalletsConfig>,
}

/// Definition of a contract in the project
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct ContractConfig {
    /// Human-readable display name of the contract
    #[serde(rename = "display-name")]
    pub name: String,
    /// Path to the contract source (`.tolk`) or precompiled (`.boc`) file
    pub src: String,
    /// Dependencies of this contract
    pub depends: Option<Vec<ContractDependency>>,
    /// Path where the compiled `.boc` should be saved
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
                separate_import_groups: None,
            }),
            build: None,
            wrappers: None,
            litenode: None,
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
        let config_path = manifest_path();
        if !config_path.exists() {
            return Err(anyhow!(
                "Acton.toml not found. Run 'acton init' to initialize Acton in the project."
            ));
        }

        let content = fs::read_to_string(config_path)?;
        let mut config: ActonConfig = toml::from_str(&content)?;

        // Merge wallets from different sources
        // Order of importance (later overrides earlier):
        // 1. Global ~/.config/acton/wallets/global.wallets.toml
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
        let local_wallets_path = project_root().join("wallets.toml");
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
        let local_libraries_path = project_root().join("libraries.toml");
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
    pub fn tolk_wrapper_output_dir(&self) -> Option<&str> {
        self.wrappers.as_ref()?.tolk.as_ref()?.output_dir.as_deref()
    }

    #[must_use]
    pub fn tolk_wrapper_generate_test(&self) -> bool {
        self.wrappers
            .as_ref()
            .and_then(|wrappers| wrappers.tolk.as_ref())
            .and_then(|tolk| tolk.generate_test)
            .unwrap_or(false)
    }

    #[must_use]
    pub fn tolk_wrapper_test_output_dir(&self) -> Option<&str> {
        self.wrappers
            .as_ref()?
            .tolk
            .as_ref()?
            .test_output_dir
            .as_deref()
    }

    #[must_use]
    pub fn typescript_wrapper_output_dir(&self) -> Option<&str> {
        self.wrappers
            .as_ref()?
            .typescript
            .as_ref()?
            .output_dir
            .as_deref()
    }

    #[must_use]
    pub fn custom_networks(&self) -> HashMap<String, CustomNetworkUrls> {
        let mut result = HashMap::new();

        let localnet_port = self
            .litenode
            .as_ref()
            .and_then(|cfg| cfg.port)
            .unwrap_or(5411);
        let default_localnet_v2 = format!("http://localhost:{localnet_port}/api/v2");
        let default_localnet_v3 = format!("http://localhost:{localnet_port}/api/v3");

        let localnet_config = self
            .networks
            .as_ref()
            .and_then(|networks| networks.get("localnet"));
        let localnet_v2 = localnet_config
            .and_then(|config| config.api.as_ref())
            .and_then(|api| api.v2.as_deref())
            .unwrap_or(default_localnet_v2.as_str());
        let localnet_v3 = localnet_config
            .and_then(|config| config.api.as_ref())
            .and_then(|api| api.v3.as_deref())
            .unwrap_or(default_localnet_v3.as_str());

        result.insert(
            "localnet".to_string(),
            CustomNetworkUrls {
                v2_url: Arc::from(localnet_v2.trim_end_matches('/')),
                v3_url: Some(Arc::from(localnet_v3.trim_end_matches('/'))),
                explorer_url: localnet_config
                    .and_then(|config| config.explorer.as_ref())
                    .map(|s| Arc::from(s.trim_end_matches('/'))),
            },
        );

        if let Some(networks) = &self.networks {
            for (name, config) in networks {
                if name == "localnet" {
                    continue;
                }

                let Some(v2_url) = config
                    .api
                    .as_ref()
                    .and_then(|api| api.v2.as_ref())
                    .map(String::as_str)
                else {
                    continue;
                };

                result.insert(
                    name.clone(),
                    CustomNetworkUrls {
                        v2_url: Arc::from(v2_url.trim_end_matches('/')),
                        v3_url: config
                            .api
                            .as_ref()
                            .and_then(|api| api.v3.as_ref())
                            .map(|s| Arc::from(s.trim_end_matches('/'))),
                        explorer_url: config
                            .explorer
                            .as_ref()
                            .map(|s| Arc::from(s.trim_end_matches('/'))),
                    },
                );
            }
        }
        result
    }

    #[must_use]
    pub fn mappings(&self) -> Option<BTreeMap<String, String>> {
        normalize_mappings(&self.mappings, project_root())
    }

    pub fn ensure_default_mappings(&mut self) -> bool {
        let mappings = self.mappings.get_or_insert_with(default_project_mappings);
        let mut changed = false;

        for (prefix, target) in DEFAULT_PROJECT_MAPPINGS {
            if !mappings.contains_key(*prefix) {
                mappings.insert((*prefix).to_string(), (*target).to_string());
                changed = true;
            }
        }

        changed
    }
}

#[must_use]
pub fn default_project_mappings() -> BTreeMap<String, String> {
    DEFAULT_PROJECT_MAPPINGS
        .iter()
        .map(|(prefix, target)| ((*prefix).to_string(), (*target).to_string()))
        .collect()
}

#[must_use]
pub fn manifest_path() -> &'static Path {
    MANIFEST_PATH
        .get_or_init(|| {
            let (root, manifest) = default_project_root_and_manifest_path();
            let _ = PROJECT_ROOT.set(root);
            let _ = PROJECT_ROOT_SOURCE.set(ResolutionSource::FallbackCwd);
            let _ = MANIFEST_PATH_SOURCE.set(ResolutionSource::FallbackCwd);
            manifest
        })
        .as_path()
}

#[must_use]
pub fn project_root() -> &'static Path {
    PROJECT_ROOT
        .get_or_init(|| {
            let (root, manifest) = default_project_root_and_manifest_path();
            let _ = MANIFEST_PATH.set(manifest);
            let _ = MANIFEST_PATH_SOURCE.set(ResolutionSource::FallbackCwd);
            let _ = PROJECT_ROOT_SOURCE.set(ResolutionSource::FallbackCwd);
            root
        })
        .as_path()
}

pub fn init_manifest_path(path: impl AsRef<Path>) -> Result<()> {
    init_manifest_path_with_source(path, ResolutionSource::FallbackCwd)
}

pub fn init_manifest_path_with_source(
    path: impl AsRef<Path>,
    source: ResolutionSource,
) -> Result<()> {
    let path = path.as_ref();
    let mut resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        path.absolutize()?.to_path_buf()
    };

    if resolved.is_dir() {
        resolved = resolved.join("Acton.toml");
    }

    match MANIFEST_PATH.set(resolved.clone()) {
        Ok(()) => Ok(()),
        Err(existing) if existing == resolved => Ok(()),
        Err(existing) => Err(anyhow!(
            "Manifest path already initialized to {}",
            existing.display()
        )),
    }?;

    match MANIFEST_PATH_SOURCE.set(source) {
        Ok(()) => Ok(()),
        Err(existing) if existing == source => Ok(()),
        Err(existing) => Err(anyhow!(
            "Manifest path source already initialized to {}",
            existing.as_str()
        )),
    }
}

pub fn init_project_root(path: impl AsRef<Path>) -> Result<()> {
    init_project_root_with_source(path, ResolutionSource::FallbackCwd)
}

pub fn init_project_root_with_source(
    path: impl AsRef<Path>,
    source: ResolutionSource,
) -> Result<()> {
    let path = path.as_ref();
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        path.absolutize()?.to_path_buf()
    };

    match PROJECT_ROOT.set(resolved.clone()) {
        Ok(()) => Ok(()),
        Err(existing) if existing == resolved => Ok(()),
        Err(existing) => Err(anyhow!(
            "Project root already initialized to {}",
            existing.display()
        )),
    }?;

    match PROJECT_ROOT_SOURCE.set(source) {
        Ok(()) => Ok(()),
        Err(existing) if existing == source => Ok(()),
        Err(existing) => Err(anyhow!(
            "Project root source already initialized to {}",
            existing.as_str()
        )),
    }
}

#[must_use]
pub fn manifest_path_resolution_source() -> ResolutionSource {
    *MANIFEST_PATH_SOURCE.get_or_init(|| ResolutionSource::FallbackCwd)
}

#[must_use]
pub fn project_root_resolution_source() -> ResolutionSource {
    *PROJECT_ROOT_SOURCE.get_or_init(|| ResolutionSource::FallbackCwd)
}

#[must_use]
pub fn resolved_paths_diagnostics() -> ResolvedPathsDiagnostics {
    ResolvedPathsDiagnostics {
        project_root: project_root().to_path_buf(),
        manifest_path: manifest_path().to_path_buf(),
        project_root_source: project_root_resolution_source(),
        manifest_path_source: manifest_path_resolution_source(),
    }
}

fn default_project_root_and_manifest_path() -> (PathBuf, PathBuf) {
    let project_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let manifest_path = project_root.join("Acton.toml");
    (project_root, manifest_path)
}

#[must_use]
pub fn normalize_mappings(
    mappings: &Option<BTreeMap<String, String>>,
    base_dir: &Path,
) -> Option<BTreeMap<String, String>> {
    let mappings = mappings.as_ref()?;

    Some(
        mappings
            .iter()
            .map(|(key, value)| {
                let normalized_key = if key.starts_with('@') {
                    key.clone()
                } else {
                    format!("@{key}")
                };
                let value_path = Path::new(value);
                let normalized_path = if value_path.is_absolute() {
                    value_path
                        .absolutize()
                        .map_or_else(|_| value_path.to_path_buf(), |path| path.to_path_buf())
                } else {
                    value_path
                        .absolutize_from(base_dir)
                        .map_or_else(|_| base_dir.join(value_path), |path| path.to_path_buf())
                };

                (
                    normalized_key,
                    normalized_path.to_string_lossy().to_string(),
                )
            })
            .collect(),
    )
}

#[must_use]
pub fn global_wallets_path() -> Option<PathBuf> {
    #[cfg(windows)]
    let home = std::env::var("USERPROFILE").ok()?;
    #[cfg(not(windows))]
    let home = std::env::var("HOME").ok()?;

    Some(
        PathBuf::from(home)
            .join(".config")
            .join("acton")
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
            .join(".config")
            .join("acton")
            .join("libraries")
            .join("global.libraries.toml"),
    )
}

impl TestSettings {
    fn coverage_enabled(&self) -> Option<bool> {
        self.coverage.as_ref().and_then(|coverage| coverage.enabled)
    }

    fn coverage_format_value(&self) -> Option<&str> {
        self.coverage
            .as_ref()
            .and_then(|coverage| coverage.format.as_deref())
    }

    fn coverage_file_value(&self) -> Option<String> {
        self.coverage
            .as_ref()
            .and_then(|coverage| coverage.output_file.clone())
    }

    fn coverage_minimum_percent_value(&self) -> Option<f64> {
        self.coverage
            .as_ref()
            .and_then(|coverage| coverage.minimum_percent)
    }

    fn coverage_include_wrappers_value(&self) -> Option<bool> {
        self.coverage
            .as_ref()
            .and_then(|coverage| coverage.include_wrappers)
    }

    fn coverage_include_tests_value(&self) -> Option<bool> {
        self.coverage
            .as_ref()
            .and_then(|coverage| coverage.include_tests)
    }

    fn mutation_minimum_percent_value(&self) -> Option<f64> {
        self.mutation
            .as_ref()
            .and_then(|mutation| mutation.minimum_percent)
    }

    fn fuzz_runs_value(&self) -> Option<usize> {
        self.fuzz.as_ref().and_then(|fuzz| fuzz.runs)
    }

    fn fuzz_max_test_rejects_value(&self) -> Option<usize> {
        self.fuzz.as_ref().and_then(|fuzz| fuzz.max_test_rejects)
    }

    fn fuzz_seed_value(&self) -> Option<u64> {
        self.fuzz.as_ref().and_then(|fuzz| fuzz.seed)
    }

    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn to_test_config(
        &self,
        filter_override: Option<String>,
        report_formats: Vec<ReportFormat>,
        show_bodies_override: bool,
        debug_override: Option<bool>,
        debug_port_override: Option<u16>,
        backtrace_override: Option<BacktraceMode>,
        coverage_override: Option<bool>,
        coverage_format_override: Option<CoverageFormat>,
        coverage_file_override: Option<String>,
        coverage_minimum_percent_override: Option<f64>,
        coverage_include_wrappers_override: Option<bool>,
        coverage_include_tests_override: Option<bool>,
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
        mutation_diff_override: Option<MutationDiffMode>,
        mutation_diff_ref_override: Option<String>,
        mutation_levels_override: Vec<MutationLevel>,
        mutation_minimum_percent_override: Option<f64>,
        disable_rules_override: Vec<String>,
        fuzz_seed_override: Option<u64>,
        fail_on_diff_override: Option<bool>,
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
            show_bodies: show_bodies_override,
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
            coverage: coverage_override.unwrap_or_else(|| self.coverage_enabled().unwrap_or(false)),
            coverage_format: coverage_format_override.or_else(|| {
                self.coverage_format_value()
                    .and_then(|f| match f.to_lowercase().as_str() {
                        "lcov" => Some(CoverageFormat::Lcov),
                        "text" => Some(CoverageFormat::Text),
                        _ => None,
                    })
            }),
            coverage_file: coverage_file_override.or_else(|| self.coverage_file_value()),
            coverage_minimum_percent: coverage_minimum_percent_override
                .or_else(|| self.coverage_minimum_percent_value()),
            coverage_include_wrappers: coverage_include_wrappers_override
                .unwrap_or_else(|| self.coverage_include_wrappers_value().unwrap_or(false)),
            coverage_include_tests: coverage_include_tests_override
                .unwrap_or_else(|| self.coverage_include_tests_value().unwrap_or(false)),
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
                        "localnet" => Some(Network::Localnet),
                        _ => None,
                    })
            }),
            api_key: api_key_override.or_else(|| self.api_key.clone()),
            fork_block_number: fork_block_number_override.or(self.fork_block_number),
            save_test_trace: save_test_trace_override,
            mutate: mutate_override,
            mutate_overrides: mutate_overrides_override,
            mutate_contract: mutate_contract_override,
            mutation_rules_file: self
                .mutation
                .as_ref()
                .and_then(|mutation| mutation.rules_file.clone()),
            mutation_session_id: None,
            mutation_workers: None,
            mutation_levels: if mutation_levels_override.is_empty() {
                self.mutation
                    .as_ref()
                    .and_then(|m| m.mutation_levels.clone())
                    .unwrap_or_default()
            } else {
                mutation_levels_override
            },
            mutation_minimum_percent: mutation_minimum_percent_override
                .or_else(|| self.mutation_minimum_percent_value()),
            mutation_ids: Vec::new(),
            mutation_diff: mutation_diff_override
                .or_else(|| self.mutation.as_ref().and_then(|mutation| mutation.diff)),
            mutation_diff_ref: mutation_diff_ref_override.or_else(|| {
                self.mutation
                    .as_ref()
                    .and_then(|mutation| mutation.diff_ref.clone())
            }),
            disable_rules: if disable_rules_override.is_empty() {
                self.mutation
                    .as_ref()
                    .and_then(|m| m.disable_rules.clone())
                    .unwrap_or_default()
            } else {
                disable_rules_override
            },
            fuzz_runs: self.fuzz_runs_value(),
            fuzz_max_test_rejects: self.fuzz_max_test_rejects_value(),
            fuzz_seed: fuzz_seed_override.or_else(|| self.fuzz_seed_value()),
            fail_on_diff: fail_on_diff_override
                .unwrap_or_else(|| self.fail_on_diff.unwrap_or(false)),
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
display-name = "Counter Contract"
src = "counter.tolk"
depends = []

[contracts.wallet-v5]
display-name = "Wallet V5"
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
    fn test_mutation_levels_parsing() {
        let toml_content = r#"
[package]
name = "test-project"
description = "Test project"
version = "0.1.0"

[test.mutation]
mutation-levels = ["critical", "major"]
"#;

        let config: ActonConfig = toml::from_str(toml_content).unwrap();
        let mutation = config
            .test
            .as_ref()
            .and_then(|test| test.mutation.as_ref())
            .expect("mutation config should be present");

        assert_eq!(
            mutation.mutation_levels,
            Some(vec![MutationLevel::Critical, MutationLevel::Major])
        );
    }

    #[test]
    fn test_mutation_minimum_percent_parsing() {
        let toml_content = r#"
[package]
name = "test-project"
description = "Test project"
version = "0.1.0"

[test.mutation]
minimum-percent = 85
"#;

        let config: ActonConfig = toml::from_str(toml_content).unwrap();
        let mutation = config
            .test
            .as_ref()
            .and_then(|test| test.mutation.as_ref())
            .expect("mutation config should be present");

        assert_eq!(mutation.minimum_percent, Some(85.0));
    }

    #[test]
    fn test_mutation_diff_parsing() {
        let toml_content = r#"
[package]
name = "test-project"
description = "Test project"
version = "0.1.0"

[test.mutation]
diff = "branch"
diff-ref = "origin/main"
"#;

        let config: ActonConfig = toml::from_str(toml_content).unwrap();
        let mutation = config
            .test
            .as_ref()
            .and_then(|test| test.mutation.as_ref())
            .expect("mutation config should be present");

        assert_eq!(mutation.diff, Some(MutationDiffMode::Branch));
        assert_eq!(mutation.diff_ref.as_deref(), Some("origin/main"));
    }

    #[test]
    fn test_mutation_rules_file_parsing() {
        let toml_content = r#"
[package]
name = "test-project"
description = "Test project"
version = "0.1.0"

[contracts.counter]
display-name = "Counter Contract"
src = "counter.tolk"
depends = []

[test.mutation]
rules-file = "mutation-rules.json"
"#;

        let config: ActonConfig = toml::from_str(toml_content).unwrap();
        let mutation = config
            .test
            .as_ref()
            .and_then(|test| test.mutation.as_ref())
            .expect("mutation settings should be present");

        assert_eq!(mutation.rules_file.as_deref(), Some("mutation-rules.json"));
    }

    #[test]
    fn test_contract_config_serializes_display_name_key() {
        let config = ActonConfig {
            package: PackageConfig {
                name: "test-project".to_string(),
                description: "Test project".to_string(),
                version: "0.1.0".to_string(),
                repository: None,
                license: None,
            },
            contracts: Some(ContractsConfig {
                contracts: BTreeMap::from([(
                    "counter".to_string(),
                    ContractConfig {
                        name: "Counter Contract".to_string(),
                        src: "counter.tolk".to_string(),
                        depends: Some(vec![]),
                        output: None,
                    },
                )]),
            }),
            test: None,
            lint: None,
            fmt: None,
            build: None,
            wrappers: None,
            litenode: None,
            scripts: None,
            wallets: None,
            libraries: None,
            mappings: None,
            networks: None,
        };

        let toml_content = toml::to_string(&config).unwrap();

        assert!(toml_content.contains("display-name = \"Counter Contract\""));
        assert!(!toml_content.contains("\nname = \"Counter Contract\""));
    }

    #[test]
    fn test_networks_api_config_parsing() {
        let toml_content = r#"
[package]
name = "test-project"
description = "Test project"
version = "0.1.0"

[networks.localnet]
api = { v2 = "http://localhost:3010/api/v2/", v3 = "http://localhost:3010/api/v3/" }
explorer = "http://localhost:3010/explorer/"

[networks.my-custom]
api = { v2 = "https://example.com/api/v2/" }
"#;

        let config: ActonConfig = toml::from_str(toml_content).unwrap();
        let networks = config.custom_networks();

        let localnet = networks
            .get("localnet")
            .expect("localnet config should be present");
        assert_eq!(localnet.v2_url.as_ref(), "http://localhost:3010/api/v2");
        assert_eq!(
            localnet.v3_url.as_deref(),
            Some("http://localhost:3010/api/v3")
        );
        assert_eq!(
            localnet.explorer_url.as_deref(),
            Some("http://localhost:3010/explorer")
        );

        let custom = networks
            .get("my-custom")
            .expect("custom network config should be present");
        assert_eq!(custom.v2_url.as_ref(), "https://example.com/api/v2");
        assert_eq!(custom.v3_url, None);
        assert_eq!(custom.explorer_url, None);
    }

    #[test]
    fn test_localnet_api_defaults_to_litenode_port() {
        let toml_content = r#"
[package]
name = "test-project"
description = "Test project"
version = "0.1.0"

[litenode]
port = 3015

[networks.localnet]
explorer = "http://localhost:3015/explorer"
"#;

        let config: ActonConfig = toml::from_str(toml_content).unwrap();
        let networks = config.custom_networks();
        let localnet = networks
            .get("localnet")
            .expect("localnet config should always be present");

        assert_eq!(localnet.v2_url.as_ref(), "http://localhost:3015/api/v2");
        assert_eq!(
            localnet.v3_url.as_deref(),
            Some("http://localhost:3015/api/v3")
        );
        assert_eq!(
            localnet.explorer_url.as_deref(),
            Some("http://localhost:3015/explorer")
        );
    }

    #[test]
    fn test_localnet_api_defaults_to_5411_without_litenode_port() {
        let toml_content = r#"
[package]
name = "test-project"
description = "Test project"
version = "0.1.0"
"#;

        let config: ActonConfig = toml::from_str(toml_content).unwrap();
        let networks = config.custom_networks();
        let localnet = networks
            .get("localnet")
            .expect("localnet config should always be present");

        assert_eq!(localnet.v2_url.as_ref(), "http://localhost:5411/api/v2");
        assert_eq!(
            localnet.v3_url.as_deref(),
            Some("http://localhost:5411/api/v3")
        );
        assert_eq!(localnet.explorer_url, None);
    }

    #[test]
    fn test_networks_legacy_v2_url_is_rejected() {
        let toml_content = r#"
[package]
name = "test-project"
description = "Test project"
version = "0.1.0"

[networks.localnet]
v2-url = "http://localhost:3010/api/v2"
"#;

        let err = toml::from_str::<ActonConfig>(toml_content).expect_err("legacy key must fail");
        assert!(
            err.to_string().contains("unknown field `v2-url`"),
            "unexpected parse error: {err}"
        );
    }

    #[test]
    fn test_lint_config_parsing() {
        let toml_content = r#"
[package]
name = "test-project"
description = "Test project"
version = "0.1.0"

[lint]
exclude = ["contracts/skip.tolk"]
max-warnings = 3
output-format = "sarif"

[lint.rules]
unused-variable = "deny"
mutable-variable-can-be-immutable = "warn"

[lint.rules.counter]
unused-variable = "allow"
"#;

        let config: ActonConfig = toml::from_str(toml_content).unwrap();
        let lint_settings = config.lint.as_ref().unwrap();
        assert_eq!(
            lint_settings.exclude.as_ref().unwrap(),
            &vec!["contracts/skip.tolk".to_string()]
        );
        assert_eq!(lint_settings.max_warnings, 3);
        assert_eq!(lint_settings.output_format, Some(CheckOutputFormat::Sarif));

        let lint = lint_settings.rules.as_ref().unwrap();

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
    fn test_lint_config_max_warnings_default_is_unlimited() {
        let toml_content = r#"
[package]
name = "test-project"
description = "Test project"
version = "0.1.0"

[lint.rules]
unused-variable = "warn"
"#;

        let config: ActonConfig = toml::from_str(toml_content).unwrap();
        let lint_settings = config.lint.as_ref().unwrap();
        assert_eq!(lint_settings.max_warnings, usize::MAX);
    }

    #[test]
    fn test_lint_config_parses_gitlab_output_format() {
        let toml_content = r#"
[package]
name = "test-project"
description = "Test project"
version = "0.1.0"

[lint]
output-format = "gitlab"
"#;

        let config: ActonConfig = toml::from_str(toml_content).unwrap();
        let lint_settings = config.lint.as_ref().unwrap();
        assert_eq!(lint_settings.output_format, Some(CheckOutputFormat::Gitlab));
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
exclude = ["**/integration/**"]
include = ["**/unit/**"]
junit-path = "custom-reports"
junit-merge = true

[test.coverage]
enabled = true
format = "lcov"
output-file = "coverage.txt"
minimum-percent = 85
include-wrappers = true
include-tests = true

[test.fuzz]
runs = 512
max-test-rejects = 4096
seed = 42
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
        let coverage = test_settings
            .coverage
            .as_ref()
            .expect("coverage settings should be parsed");
        assert_eq!(coverage.enabled, Some(true));
        assert_eq!(coverage.format, Some("lcov".to_string()));
        assert_eq!(coverage.output_file, Some("coverage.txt".to_string()));
        assert_eq!(coverage.minimum_percent, Some(85.0));
        assert_eq!(coverage.include_wrappers, Some(true));
        assert_eq!(coverage.include_tests, Some(true));
        let fuzz = test_settings
            .fuzz
            .as_ref()
            .expect("fuzz settings should be parsed");
        assert_eq!(fuzz.runs, Some(512));
        assert_eq!(fuzz.max_test_rejects, Some(4096));
        assert_eq!(fuzz.seed, Some(42));
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

    #[test]
    fn test_normalize_mappings_adds_prefix_and_resolves_paths_from_base_dir() {
        let base_dir = std::env::temp_dir().join("acton-config-mappings-base");
        let _ = fs::create_dir_all(&base_dir);
        let shared_dir = base_dir
            .parent()
            .expect("base path must have parent")
            .join("shared");

        let mappings = Some(BTreeMap::from([
            ("contracts".to_string(), "./contracts".to_string()),
            ("@tests".to_string(), "tests".to_string()),
            ("shared".to_string(), "../shared".to_string()),
        ]));

        let normalized = normalize_mappings(&mappings, &base_dir).expect("must normalize");

        assert_eq!(
            normalized.get("@contracts"),
            Some(&base_dir.join("contracts").to_string_lossy().to_string())
        );
        assert_eq!(
            normalized.get("@tests"),
            Some(&base_dir.join("tests").to_string_lossy().to_string())
        );
        assert_eq!(
            normalized.get("@shared"),
            Some(&shared_dir.to_string_lossy().to_string())
        );
    }

    #[test]
    fn test_build_settings_parsing() {
        let toml_content = r#"
[package]
name = "test-project"
description = "Test project"
version = "0.1.0"

[build]
out-dir = "artifacts/build"
gen-dir = "artifacts/gen"
output-fift = "build/fift"
"#;

        let config: ActonConfig = toml::from_str(toml_content).unwrap();
        let build = config.build.as_ref().unwrap();
        assert_eq!(build.out_dir.as_deref(), Some("artifacts/build"));
        assert_eq!(build.gen_dir.as_deref(), Some("artifacts/gen"));
        assert_eq!(build.output_fift.as_deref(), Some("build/fift"));
    }

    #[test]
    fn test_wrappers_typescript_settings_parsing() {
        let toml_content = r#"
[package]
name = "test-project"
description = "Test project"
version = "0.1.0"

[wrappers.tolk]
output-dir = "tests/generated-wrappers"
generate-test = true
test-output-dir = "tests/generated-tests"

[wrappers.typescript]
output-dir = "./wrappers"
"#;

        let config: ActonConfig = toml::from_str(toml_content).unwrap();
        assert_eq!(
            config.tolk_wrapper_output_dir(),
            Some("tests/generated-wrappers")
        );
        assert!(config.tolk_wrapper_generate_test());
        assert_eq!(
            config.tolk_wrapper_test_output_dir(),
            Some("tests/generated-tests")
        );
        assert_eq!(config.typescript_wrapper_output_dir(), Some("./wrappers"));
    }

    #[test]
    fn test_litenode_settings_parsing() {
        let toml_content = r#"
[package]
name = "test-project"
description = "Test project"
version = "0.1.0"

[litenode]
port = 3015
fork-net = "testnet"
fork-block-number = 1234567
accounts = ["deployer", "user"]
rate-limit = 3
"#;

        let config: ActonConfig = toml::from_str(toml_content).unwrap();
        let litenode = config.litenode.as_ref().unwrap();
        assert_eq!(litenode.port, Some(3015));
        assert_eq!(litenode.fork_net.as_deref(), Some("testnet"));
        assert_eq!(litenode.fork_block_number, Some(1234567));
        assert_eq!(
            litenode.accounts,
            Some(vec!["deployer".to_string(), "user".to_string()])
        );
        assert_eq!(litenode.rate_limit, Some(3));
    }
}
