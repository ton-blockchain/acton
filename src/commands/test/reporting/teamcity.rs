use super::{TestReport, TestReporter, TestStatus, TestSuiteStats, extract_suite_name};
use crate::commands::test::TestDescriptor;
use crate::context::AssertFailure;
use crate::formatter::FormatterContext;
use std::path::Path;

pub(crate) struct TeamCityReporter {
    formatter: Option<FormatterContext<'static>>,
}

impl TeamCityReporter {
    pub(crate) const fn new() -> Self {
        Self { formatter: None }
    }

    /// See <https://www.jetbrains.com/help/teamcity/service-messages.html#Escaped+Values>
    fn escape_name(&self, name: &str) -> String {
        name.replace('|', "||")
            .replace('\n', "|n")
            .replace('\r', "|r")
            .replace('[', "|[")
            .replace(']', "|]")
            .replace('\'', "|'")
    }

    fn format_test_failure(
        &self,
        test: &TestReport,
    ) -> (String, String, Option<String>, Option<String>) {
        let mut message = "Test failed".to_string();
        let mut details = String::new();
        let mut expected: Option<String> = None;
        let mut actual: Option<String> = None;

        if let Some(exec) = &test.execution
            && let Some(ref assert_failure) = exec.assert_failure
        {
            if let Some(location) = assert_failure.location() {
                details = location.format_full();
            }

            match assert_failure {
                AssertFailure::Bin(bin_failure) => match bin_failure.operator.as_str() {
                    "==" => {
                        message = "Values are not equal".to_string();
                        if let Some(formatter) = &self.formatter {
                            expected = Some(formatter.format_tuple_value(
                                &bin_failure.right,
                                &bin_failure.right_type,
                                0,
                            ));
                            actual = Some(formatter.format_tuple_value(
                                &bin_failure.left,
                                &bin_failure.left_type,
                                0,
                            ));
                        }
                    }
                    "!=" => {
                        message = "Values are equal but expected to be different".to_string();
                    }
                    _ => {
                        message = "Assertion failed".to_string();
                    }
                },
                AssertFailure::Fail(_) => {
                    message = "Test assertion failed".to_string();
                }
                AssertFailure::TransactionNotFound(_) => {
                    message = "Transaction not found".to_string();
                }
                AssertFailure::TransactionIsFound(_) => {
                    message = "Unexpected transaction found".to_string();
                }
                AssertFailure::WalletNotFound(failure) => {
                    message = format!("Wallet '{}' not found", failure.wallet_name);
                }
            }

            if let Some(failure_message) = assert_failure.message()
                && !failure_message.is_empty()
            {
                message = failure_message;
            }
        }

        if let Some(ref test_message) = test.message {
            message = test_message.clone();
        }

        (message, details, expected, actual)
    }
}

impl Default for TeamCityReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl TestReporter for TeamCityReporter {
    fn on_testing_started(&mut self) -> anyhow::Result<()> {
        println!("##teamcity[testingStarted]");
        Ok(())
    }

    fn on_testing_finished(&mut self, _stats: &TestSuiteStats) -> anyhow::Result<()> {
        println!("##teamcity[testingFinished]");
        Ok(())
    }

    fn on_suite_started(
        &mut self,
        file_path: &Path,
        _tests: &[TestDescriptor],
    ) -> anyhow::Result<()> {
        let suite_name = extract_suite_name(file_path);
        let escaped_name = self.escape_name(&suite_name);

        println!(
            "##teamcity[testSuiteStarted name='{escaped_name}' nodeId='suite_{escaped_name}' parentNodeId='0' nodeType='file' locationHint='file://{}']",
            file_path.display()
        );
        Ok(())
    }

    fn on_suite_finished(
        &mut self,
        file_path: &Path,
        _stats: &TestSuiteStats,
    ) -> anyhow::Result<()> {
        let suite_name = extract_suite_name(file_path);
        let escaped_name = self.escape_name(&suite_name);

        println!(
            "##teamcity[testSuiteFinished name='{escaped_name}' nodeId='suite_{escaped_name}']"
        );
        Ok(())
    }

    fn on_test_started(&mut self, test: &TestReport) -> anyhow::Result<()> {
        let test_name = self.escape_name(&test.name);
        let suite_name = self.escape_name(&extract_suite_name(Path::new(&test.file_path)));
        let location = self.escape_name(&format!("{}:{}", test.file_path.display(), test.name));

        println!(
            "##teamcity[testStarted name='{test_name}' nodeId='test_{test_name}' parentNodeId='suite_{suite_name}' locationHint='tolk_qn://{location}']"
        );
        Ok(())
    }

    fn on_test_finished(&mut self, test: &TestReport) -> anyhow::Result<()> {
        let test_name = self.escape_name(&test.name);
        let suite_name = self.escape_name(&extract_suite_name(Path::new(&test.file_path)));
        let duration_ms = test.duration.as_millis();

        match test.status {
            TestStatus::Failed => {
                let (message, details, expected, actual) = self.format_test_failure(test);

                if let (Some(exp), Some(act)) = (expected, actual) {
                    println!(
                        "##teamcity[testFailed type='comparisonFailure' name='{}' nodeId='test_{}' duration='{}' message='{}' details='{}' expected='{}' actual='{}']",
                        test_name,
                        test_name,
                        duration_ms,
                        self.escape_name(&message),
                        self.escape_name(&details),
                        self.escape_name(&exp),
                        self.escape_name(&act),
                    );
                } else {
                    println!(
                        "##teamcity[testFailed name='{}' nodeId='test_{}' duration='{}' message='{}' details='{}']",
                        test_name,
                        test_name,
                        duration_ms,
                        self.escape_name(&message),
                        self.escape_name(&details),
                    );
                }
            }
            TestStatus::Skipped | TestStatus::Todo => {
                println!(
                    "##teamcity[testIgnored name='{test_name}' nodeId='test_{test_name}' duration='{duration_ms}']"
                );
            }
            TestStatus::Passed => {}
        }

        println!(
            "##teamcity[testFinished name='{test_name}' nodeId='test_{test_name}' duration='{duration_ms}' parentNodeId='suite_{suite_name}']"
        );

        Ok(())
    }
}
