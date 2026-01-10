use super::{TestReport, TestReporter, TestStatus, TestSuiteStats, escape_xml, extract_suite_name};
use crate::commands::test::TestDescriptor;
use quick_junit::{NonSuccessKind, Report, TestCase, TestCaseStatus, TestSuite};
use std::collections::BTreeMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct JUnitConfig {
    pub output_dir: PathBuf,
    pub merge_suites: bool,
    pub include_system_out: bool,
    pub include_system_err: bool,
}

impl Default for JUnitConfig {
    fn default() -> Self {
        Self {
            output_dir: PathBuf::from("test-results"),
            merge_suites: false,
            include_system_out: true,
            include_system_err: true,
        }
    }
}

#[derive(Debug)]
struct JUnitTestSuite {
    name: String,
    file_path: PathBuf,
    tests: Vec<TestReport>,
    stats: TestSuiteStats,
    timestamp: SystemTime,
}

pub struct JUnitReporter {
    config: JUnitConfig,
    suites: BTreeMap<String, JUnitTestSuite>,
    current_suite: Option<String>,
}

impl JUnitReporter {
    pub fn new(config: JUnitConfig) -> Self {
        Self {
            config,
            suites: BTreeMap::new(),
            current_suite: None,
        }
    }

    fn convert_test_to_testcase(&self, test: &TestReport) -> TestCase {
        let status = match test.status {
            TestStatus::Passed => TestCaseStatus::success(),
            TestStatus::Failed => {
                let mut status = TestCaseStatus::non_success(NonSuccessKind::Failure);
                if let Some(ref message) = test.message {
                    status.set_message(escape_xml(message).as_str());
                }
                status.set_type("AssertionError");
                if let Some(ref details) = test.details {
                    status.set_description(format!("at {}", details.as_str()));
                }
                status
            }
            TestStatus::Skipped => {
                let mut status = TestCaseStatus::skipped();
                let message = test.details.as_deref().unwrap_or("Test skipped");
                status.set_message(message);
                status
            }
            TestStatus::Todo => {
                let mut status = TestCaseStatus::skipped();
                let message = test
                    .details
                    .as_ref()
                    .map(|d| format!("TODO: {d}"))
                    .unwrap_or_else(|| "TODO".to_string());
                status.set_message(message);
                status
            }
        };

        let mut test_case = TestCase::new(&test.name, status);
        test_case.set_classname(&test.suite_name);
        test_case.set_time(test.duration);

        if let Some(execution) = &test.execution {
            if self.config.include_system_out && !execution.stdout.trim().is_empty() {
                test_case.set_system_out(&execution.stdout);
            }
            if self.config.include_system_err && !execution.stderr.trim().is_empty() {
                test_case.set_system_err(&execution.stderr);
            }
        }

        test_case
    }

    fn convert_suite_to_testsuite(&self, suite: &JUnitTestSuite) -> TestSuite {
        let mut test_suite = TestSuite::new(&suite.name);
        test_suite.set_time(suite.stats.duration);

        if let Some(timestamp) = chrono::DateTime::from_timestamp(
            suite
                .timestamp
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
            0,
        ) {
            test_suite.set_timestamp(timestamp.fixed_offset());
        }

        test_suite.add_property(("file.path", suite.file_path.display().to_string().as_str()));

        for test in &suite.tests {
            let test_case = self.convert_test_to_testcase(test);
            test_suite.add_test_case(test_case);
        }

        test_suite
    }

    fn write_suite_file(&self, suite: &JUnitTestSuite) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.config.output_dir)?;

        if self.config.merge_suites {
            return Ok(());
        }

        let filename = format!(
            "TEST-{}.xml",
            suite.name.replace("/", "_").replace("\\", "_")
        );

        let file_path = self.config.output_dir.join(filename);
        let mut file = File::create(&file_path)?;

        let test_suite = self.convert_suite_to_testsuite(suite);
        let mut report = Report::new("test-results");
        report.add_test_suite(test_suite);

        report
            .serialize(&mut file)
            .map_err(|e| anyhow::anyhow!("Failed to serialize JUnit report: {e}"))?;

        Ok(())
    }
}

impl TestReporter for JUnitReporter {
    fn init(&mut self) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.config.output_dir)?;
        Ok(())
    }

    fn on_suite_started(
        &mut self,
        file_path: &Path,
        _tests: &[TestDescriptor],
    ) -> anyhow::Result<()> {
        let suite_name = extract_suite_name(file_path);
        self.current_suite = Some(suite_name.clone());

        let suite = JUnitTestSuite {
            name: suite_name.clone(),
            file_path: file_path.to_owned(),
            tests: Vec::new(),
            stats: TestSuiteStats::default(),
            timestamp: SystemTime::now(),
        };

        self.suites.insert(suite_name, suite);
        Ok(())
    }

    fn on_suite_finished(
        &mut self,
        _file_path: &Path,
        stats: &TestSuiteStats,
    ) -> anyhow::Result<()> {
        if let Some(ref suite_name) = self.current_suite {
            if let Some(suite) = self.suites.get_mut(suite_name) {
                suite.stats = stats.clone();
            }

            if !self.config.merge_suites
                && let Some(suite) = self.suites.get(suite_name)
            {
                self.write_suite_file(suite)?;
            }
        }
        self.current_suite = None;
        Ok(())
    }

    fn on_test_finished(&mut self, test: &TestReport) -> anyhow::Result<()> {
        if let Some(ref suite_name) = self.current_suite
            && let Some(suite) = self.suites.get_mut(suite_name)
        {
            suite.tests.push(test.clone());
        }
        Ok(())
    }

    fn finalize(&mut self) -> anyhow::Result<()> {
        if self.config.merge_suites && !self.suites.is_empty() {
            let file_path = self.config.output_dir.join("junit-results.xml");
            let mut file = File::create(&file_path)?;

            let mut report = Report::new("test-results");

            for suite in self.suites.values() {
                let test_suite = self.convert_suite_to_testsuite(suite);
                report.add_test_suite(test_suite);
            }

            report
                .serialize(&mut file)
                .map_err(|e| anyhow::anyhow!("Failed to serialize JUnit report: {e}"))?;
        }

        Ok(())
    }
}
