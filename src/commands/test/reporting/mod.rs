use crate::commands::test::TestDescriptor;
use crate::commands::test::trace::TransactionInfo;
use crate::context::{AssertFailure, BuildCache, EmulationsState, KnownAddresses};
use crate::formatter::FormatterContext;
use acton_config::config::Network;
use acton_config::test::BacktraceMode;
use rustc_hash::FxHashMap;
use serde::Serialize;
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tolk_compiler::TolkSourceMap;
use tolk_compiler::abi::ContractABI as CompilerContractABI;
use ton_abi::ContractAbi;
use ton_executor::get::GetMethodResult;
use ton_source_map::SourceLocation;
use tycho_types::cell::HashBytes;
use tycho_types::models::{ShardAccount, StdAddr};

pub(super) mod console;
pub(super) mod dot;
pub(super) mod junit;
pub(super) mod teamcity;
pub(super) mod ui;

#[derive(Debug, Clone)]
pub struct TestExecutionContext {
    pub gas_used: u64,
    pub stdout: String,
    pub stderr: String,
    pub vm_log: Option<Arc<str>>,
    pub assert_failure: Option<AssertFailure>,
    pub expected_exit_code: i32,
    pub fuzz: Option<FuzzExecutionContext>,
    pub failure: Option<TestFailureExecutionContext>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FuzzCaseContext {
    pub run: usize,
    pub inputs: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FuzzExecutionContext {
    pub total_runs: usize,
    pub seed: u64,
    pub failed_case: Option<FuzzCaseContext>,
}

#[must_use]
pub(crate) fn format_fuzz_failure_context(fuzz: &FuzzExecutionContext) -> String {
    let mut lines = vec![
        format!("Fuzz seed: {}", fuzz.seed),
        format!("Fuzz runs: {}", fuzz.total_runs),
    ];

    if let Some(case) = &fuzz.failed_case {
        lines.push(format!("Fuzz case: {}/{}", case.run, fuzz.total_runs));
        if !case.inputs.is_empty() {
            let inputs = case
                .inputs
                .iter()
                .map(|(name, value)| format!("{name}={value}"))
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!("Inputs: {inputs}"));
        }
    }

    lines.join("\n")
}

#[derive(Debug, Clone)]
pub struct TestFailureExecutionContext {
    pub get_result: GetMethodResult,
    pub accounts: FxHashMap<StdAddr, ShardAccount>,
    pub build_cache: BuildCache,
    pub emulations: EmulationsState,
    pub known_addresses: KnownAddresses,
    pub known_code_cells: FxHashMap<HashBytes, String>,
    pub has_wallets_config: bool,
    pub available_wallets: Vec<String>,
    pub fork_net: Option<Network>,
    pub network: Option<Network>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FailedTransactionContext {
    pub from_address: Option<String>,
    pub to_address: Option<String>,
    pub params: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TestReport {
    pub name: Arc<str>,
    pub suite_name: Arc<str>,
    pub file_path: PathBuf,
    pub row: usize,
    pub column: usize,
    pub duration: Duration,
    #[serde(skip)]
    pub gas_limit: Option<u64>,
    pub status: TestStatus,
    pub message: Option<String>,
    pub detailed_message: Option<String>,
    pub failed_transactions: Option<Vec<TransactionInfo>>,
    pub failed_transaction_context: Option<FailedTransactionContext>,
    pub details: Option<String>,
    pub location: Option<SourceLocation>,
    #[serde(skip)]
    pub abi: Arc<ContractAbi>,
    #[serde(skip)]
    pub compiler_abi: Option<Arc<CompilerContractABI>>,
    #[serde(skip)]
    pub source_map: Arc<TolkSourceMap>,
    #[serde(skip)]
    pub show_bodies: bool,
    #[serde(skip)]
    pub backtrace: Option<BacktraceMode>,
    #[serde(skip)]
    pub execution: Option<TestExecutionContext>,
    pub trace_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum TestStatus {
    Passed,
    Failed,
    Skipped,
    Todo,
}

#[derive(Debug, Clone, Default)]
pub struct TestSuiteStats {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub todo: usize,
    pub duration: Duration,
}

impl TestSuiteStats {
    pub fn add_test(&mut self, status: &TestStatus, duration: Duration) {
        self.total += 1;
        self.duration += duration;

        match status {
            TestStatus::Passed => self.passed += 1,
            TestStatus::Failed => self.failed += 1,
            TestStatus::Skipped => self.skipped += 1,
            TestStatus::Todo => self.todo += 1,
        }
    }
}

pub trait TestReporter: Send + Sync {
    fn init(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_testing_started(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_testing_finished(&mut self, _stats: &TestSuiteStats) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_suite_started(
        &mut self,
        _file_path: &Path,
        _tests: &[TestDescriptor],
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_suite_finished(
        &mut self,
        _file_path: &Path,
        _stats: &TestSuiteStats,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_test_started(&mut self, _test: &TestReport) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_test_finished(&mut self, _test: &TestReport) -> anyhow::Result<()> {
        Ok(())
    }

    fn finalize(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Default)]
pub struct ReporterManager {
    reporters: Vec<Box<dyn TestReporter>>,
}

impl std::fmt::Debug for ReporterManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReporterManager")
            .field("reporters_count", &self.reporters.len())
            .finish()
    }
}

impl ReporterManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            reporters: Vec::new(),
        }
    }

    pub fn add_reporter(&mut self, reporter: Box<dyn TestReporter>) {
        self.reporters.push(reporter);
    }

    pub fn init(&mut self) -> anyhow::Result<()> {
        for reporter in &mut self.reporters {
            reporter.init()?;
        }
        Ok(())
    }

    pub fn on_testing_started(&mut self) -> anyhow::Result<()> {
        for reporter in &mut self.reporters {
            reporter.on_testing_started()?;
        }
        Ok(())
    }

    pub fn on_testing_finished(&mut self, stats: &TestSuiteStats) -> anyhow::Result<()> {
        for reporter in &mut self.reporters {
            reporter.on_testing_finished(stats)?;
        }
        Ok(())
    }

    pub fn on_suite_started(
        &mut self,
        file_path: &Path,
        tests: &[TestDescriptor],
    ) -> anyhow::Result<()> {
        for reporter in &mut self.reporters {
            reporter.on_suite_started(file_path, tests)?;
        }
        Ok(())
    }

    pub fn on_suite_finished(
        &mut self,
        file_path: &Path,
        stats: &TestSuiteStats,
    ) -> anyhow::Result<()> {
        for reporter in &mut self.reporters {
            reporter.on_suite_finished(file_path, stats)?;
        }
        Ok(())
    }

    pub fn on_test_started(&mut self, test: &TestReport) -> anyhow::Result<()> {
        for reporter in &mut self.reporters {
            reporter.on_test_started(test)?;
        }
        Ok(())
    }

    pub fn on_test_finished(&mut self, test: &TestReport) -> anyhow::Result<()> {
        for reporter in &mut self.reporters {
            reporter.on_test_finished(test)?;
        }
        Ok(())
    }

    pub fn finalize(&mut self) -> anyhow::Result<()> {
        for reporter in &mut self.reporters {
            reporter.finalize()?;
        }
        Ok(())
    }
}

pub(super) fn extract_suite_name(file_path: &Path) -> Arc<str> {
    file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_else(|| file_path.to_str().unwrap_or(""))
        .into()
}

pub(super) fn formatter_for_failed_test<'a>(test: &'a TestReport) -> Option<FormatterContext<'a>> {
    let failure = test.execution.as_ref()?.failure.as_ref()?;

    Some(FormatterContext {
        contract_abi: test.abi.clone(),
        accounts: Cow::Borrowed(&failure.accounts),
        build_cache: Cow::Borrowed(&failure.build_cache),
        emulations: Cow::Borrowed(&failure.emulations),
        known_addresses: Cow::Borrowed(&failure.known_addresses),
        known_code_cells: Cow::Borrowed(&failure.known_code_cells),
        show_bodies: test.show_bodies,
        has_wallets_config: failure.has_wallets_config,
        available_wallets: failure.available_wallets.clone(),
        backtrace: test.backtrace,
        fork_net: failure.fork_net.clone(),
        network: failure.network.clone(),
    })
}
