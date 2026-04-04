use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config::Network;

/// Backtrace verbosity for failed tests
#[derive(
    clap::ValueEnum, Debug, Copy, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum BacktraceMode {
    /// Emit the full execution backtrace
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

/// Output formats supported by `acton test`
#[derive(
    clap::ValueEnum, Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema,
)]
#[clap(rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ReportFormat {
    /// Human-readable console output
    #[default]
    Console,
    /// TeamCity service messages
    TeamCity,
    /// JUnit XML report
    JUnit,
    /// Compact dot-progress output
    Dot,
}

/// Coverage output formats supported by `acton test`
#[derive(
    clap::ValueEnum, Debug, Copy, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum CoverageFormat {
    /// LCOV coverage report
    #[default]
    Lcov,
    /// Plain-text coverage summary
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

/// Mutation levels supported by mutation testing filters
#[derive(
    clap::ValueEnum, Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Hash,
)]
#[clap(rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum MutationLevel {
    /// Security-sensitive control-flow, persistence, and upgrade mutations
    Critical,
    /// High-signal behavioral mutations such as arithmetic and comparisons
    Major,
    /// Broader low-priority mutations such as bitwise variants
    Minor,
}

impl MutationLevel {
    pub const fn as_str(self) -> &'static str {
        match self {
            MutationLevel::Critical => "critical",
            MutationLevel::Major => "major",
            MutationLevel::Minor => "minor",
        }
    }
}

impl std::fmt::Display for MutationLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Diff scopes supported by mutation testing filters
#[derive(
    clap::ValueEnum, Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Hash,
)]
#[clap(rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum MutationDiffMode {
    /// Mutate only lines changed in the current worktree compared to HEAD
    Worktree,
    /// Mutate only lines changed compared to a specific ref or commit
    Ref,
    /// Mutate only lines changed on the current branch since its merge-base
    Branch,
}

impl MutationDiffMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            MutationDiffMode::Worktree => "worktree",
            MutationDiffMode::Ref => "ref",
            MutationDiffMode::Branch => "branch",
        }
    }
}

impl std::fmt::Display for MutationDiffMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Default)]
pub struct TestConfig {
    pub report_formats: Vec<ReportFormat>,
    pub show_bodies: bool,
    pub debug: bool,
    pub debug_port: u16,
    pub backtrace: Option<BacktraceMode>,
    pub coverage: bool,
    pub coverage_minimum_percent: Option<f64>,
    pub coverage_include_wrappers: bool,
    pub coverage_include_tests: bool,
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
    pub fail_on_diff: bool,
    pub fork_net: Option<Network>,
    pub api_key: Option<String>,
    pub fork_block_number: Option<u64>,
    pub save_test_trace: Option<String>,
    pub mutate: bool,
    pub mutate_overrides: Option<String>,
    pub mutate_contract: Option<String>,
    pub mutation_session_id: Option<String>,
    pub mutation_levels: Vec<MutationLevel>,
    pub mutation_minimum_percent: Option<f64>,
    pub mutation_ids: Vec<usize>,
    pub mutation_diff: Option<MutationDiffMode>,
    pub mutation_diff_ref: Option<String>,
    pub disable_rules: Vec<String>,
    pub fuzz_runs: Option<usize>,
    pub fuzz_max_test_rejects: Option<usize>,
    pub fuzz_seed: Option<u64>,
    pub fail_fast: bool,
    pub ui: bool,
    pub ui_port: u16,
}
