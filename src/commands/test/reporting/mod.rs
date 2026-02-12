use crate::commands::test::TestDescriptor;
use crate::commands::test::trace::TransactionInfo;
use crate::context::{AssertFailure, BuildCache, EmulationsState, KnownAddresses};
use acton_config::test::BacktraceMode;
use rustc_hash::FxHashMap;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use ton_abi::ContractAbi;
use ton_executor::get::GetMethodResult;
use ton_source_map::{SourceLocation, SourceMap};
use tycho_types::models::ShardAccount;

pub(super) mod console;
pub(super) mod dot;
pub(super) mod junit;
pub(super) mod teamcity;
pub(super) mod ui;

#[derive(Debug, Clone)]
pub struct TestExecutionContext {
    pub get_result: GetMethodResult,
    pub gas_used: u64,
    pub stdout: String,
    pub stderr: String,
    pub assert_failure: Option<AssertFailure>,
    pub accounts: FxHashMap<String, ShardAccount>,
    pub expected_exit_code: i32,
    pub build_cache: BuildCache,
    pub emulations: EmulationsState,
    pub known_addresses: KnownAddresses,
    pub known_code_cells: FxHashMap<String, String>,
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
    pub source_map: Arc<SourceMap>,
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

pub(super) fn escape_xml(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
