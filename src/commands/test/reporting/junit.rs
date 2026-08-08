use super::{
    TestReport, TestReporter, TestStatus, TestSuiteStats, extract_suite_name,
    format_fuzz_failure_context,
};
use crate::commands::test::TestDescriptor;
use crate::formatter::FormatterContext;
use acton_config::config::project_root as configured_project_root;
use quick_junit::{NonSuccessKind, Report, TestCase, TestCaseStatus, TestSuite};
use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub(crate) struct JUnitConfig {
    pub output_dir: PathBuf,
    pub merge_suites: bool,
    pub include_system_out: bool,
    pub include_system_err: bool,
}

impl Default for JUnitConfig {
    fn default() -> Self {
        Self {
            output_dir: configured_project_root().join("test-results"),
            merge_suites: false,
            include_system_out: true,
            include_system_err: true,
        }
    }
}

#[derive(Debug)]
struct JUnitTestSuite {
    name: Arc<str>,
    file_path: PathBuf,
    tests: Vec<TestReport>,
    stats: TestSuiteStats,
    timestamp: SystemTime,
}

pub(crate) struct JUnitReporter {
    config: JUnitConfig,
    suites: BTreeMap<PathBuf, JUnitTestSuite>,
}

impl JUnitReporter {
    pub(crate) const fn new(config: JUnitConfig) -> Self {
        Self {
            config,
            suites: BTreeMap::new(),
        }
    }

    fn convert_test_to_testcase(&self, test: &TestReport) -> TestCase {
        let status = match test.status {
            TestStatus::Passed => TestCaseStatus::success(),
            TestStatus::Failed => {
                let mut status = TestCaseStatus::non_success(NonSuccessKind::Failure);
                if let Some(ref message) = test.message {
                    status.set_message(message);
                }
                status.set_type("AssertionError");
                let mut description_lines = Vec::new();
                if let Some(ref location) = test.location {
                    description_lines.push(format!("at {}", location.format_full().as_str()));
                }
                if let Some(execution) = &test.execution
                    && let Some(fuzz) = &execution.fuzz
                {
                    description_lines.push(format_fuzz_failure_context(fuzz));
                }
                if let Some(detailed_message) = test
                    .detailed_message
                    .as_deref()
                    .filter(|message| !message.trim().is_empty())
                {
                    description_lines.push(FormatterContext::strip_ansi_text(detailed_message));
                }
                if !description_lines.is_empty() {
                    status.set_description(description_lines.join("\n"));
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
                    .map_or_else(|| "TODO".to_string(), |d| format!("TODO: {d}"));
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

        let filename = self.suite_output_filename(suite);

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

    fn suite_output_filename(&self, suite: &JUnitTestSuite) -> String {
        let sanitized_name: String = suite
            .name
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                    ch
                } else {
                    '_'
                }
            })
            .collect();

        let basename = format!("TEST-{sanitized_name}");
        let same_name_count = self
            .suites
            .values()
            .filter(|item| item.name == suite.name)
            .count();

        if same_name_count <= 1 {
            return format!("{basename}.xml");
        }

        let mut hasher = DefaultHasher::new();
        suite.file_path.hash(&mut hasher);
        let suffix = hasher.finish();
        format!("{basename}-{suffix:016x}.xml")
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
        let suite_id = file_path.to_owned();

        let suite = JUnitTestSuite {
            name: suite_name,
            file_path: suite_id.clone(),
            tests: Vec::new(),
            stats: TestSuiteStats::default(),
            timestamp: SystemTime::now(),
        };

        self.suites.insert(suite_id, suite);
        Ok(())
    }

    fn on_suite_finished(
        &mut self,
        file_path: &Path,
        stats: &TestSuiteStats,
    ) -> anyhow::Result<()> {
        if let Some(suite) = self.suites.get_mut(file_path) {
            suite.stats = stats.clone();
        }

        if !self.config.merge_suites
            && let Some(suite) = self.suites.get(file_path)
        {
            self.write_suite_file(suite)?;
        }

        Ok(())
    }

    fn on_test_finished(&mut self, test: &TestReport) -> anyhow::Result<()> {
        if let Some(suite) = self.suites.get_mut(&test.file_path) {
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
