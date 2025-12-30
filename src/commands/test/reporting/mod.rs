use crate::commands::test::TestDescriptor;
use crate::context::{AssertFailure, BuildCache, Emulations, KnownAddresses};
use abi::ContractAbi;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;
use tolkc::source_map::SourceMap;
use ton_executor::get::GetMethodResult;
use tycho_types::models::ShardAccount;

pub mod console;
pub mod dot;
pub mod junit;
pub mod teamcity;

#[derive(Debug, Clone)]
pub struct TestExecutionContext {
    pub get_result: GetMethodResult,
    pub gas_used: u64,
    pub stdout: String,
    pub stderr: String,
    pub assert_failure: Option<AssertFailure>,
    pub accounts: HashMap<String, ShardAccount>,
    pub expected_exit_code: i32,
    pub build_cache: BuildCache,
    pub emulations: Emulations,
    pub known_addresses: KnownAddresses,
    pub known_code_cells: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct TestReport {
    pub name: String,
    pub suite_name: String,
    pub file_path: String,
    pub duration: Duration,
    pub gas_limit: Option<u64>,
    pub status: TestStatus,
    pub message: Option<String>,
    pub details: Option<String>,
    pub abi: ContractAbi,
    pub source_map: SourceMap,
    pub backtrace: Option<String>,
    pub execution: Option<TestExecutionContext>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TestStatus {
    Passed,
    Failed,
    Skipped,
    Ignored,
    Todo,
}

#[derive(Debug, Clone, Default)]
pub struct TestSuiteStats {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub ignored: usize,
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
            TestStatus::Ignored => self.ignored += 1,
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
        _file_path: &str,
        _tests: &[TestDescriptor],
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_suite_finished(
        &mut self,
        _file_path: &str,
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
        file_path: &str,
        tests: &[TestDescriptor],
    ) -> anyhow::Result<()> {
        for reporter in &mut self.reporters {
            reporter.on_suite_started(file_path, tests)?;
        }
        Ok(())
    }

    pub fn on_suite_finished(
        &mut self,
        file_path: &str,
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

pub(crate) fn extract_suite_name(file_path: &str) -> String {
    Path::new(file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(file_path)
        .to_string()
}

pub(crate) fn escape_xml(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
