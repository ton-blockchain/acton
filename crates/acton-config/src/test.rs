use crate::config::Network;

#[derive(clap::ValueEnum, Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum BacktraceMode {
    #[default]
    Full,
}

impl std::fmt::Display for BacktraceMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BacktraceMode::Full => write!(f, "full"),
        }
    }
}

#[derive(clap::ValueEnum, Debug, Clone, PartialEq, Default)]
#[clap(rename_all = "lowercase")]
pub enum ReportFormat {
    #[default]
    Console,
    TeamCity,
    JUnit,
    Dot,
}

#[derive(clap::ValueEnum, Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum CoverageFormat {
    #[default]
    Lcov,
    Text,
}

impl std::fmt::Display for CoverageFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoverageFormat::Lcov => write!(f, "lcov"),
            CoverageFormat::Text => write!(f, "text"),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TestConfig {
    pub report_formats: Vec<ReportFormat>,
    pub debug: bool,
    pub debug_port: u16,
    pub backtrace: Option<BacktraceMode>,
    pub coverage: bool,
    pub filter: Option<String>,
    pub coverage_format: Option<CoverageFormat>,
    pub coverage_file: Option<String>,
    pub exclude_patterns: Vec<String>,
    pub include_patterns: Vec<String>,
    pub clear_cache: bool,
    pub junit_path: Option<String>,
    pub junit_merge: bool,
    pub snapshot: Option<String>,
    pub baseline_snapshot: Option<String>,
    pub fork_net: Option<Network>,
    pub api_key: Option<String>,
    pub fork_block_number: Option<u64>,
    pub save_test_trace: Option<String>,
    pub mutate: bool,
    pub mutate_overrides: Option<String>,
    pub mutate_contract: Option<String>,
    pub disable_rules: Vec<String>,
    pub fail_fast: bool,
    pub ui: bool,
    pub ui_port: u16,
}
