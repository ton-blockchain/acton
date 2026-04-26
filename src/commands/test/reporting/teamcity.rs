use super::{
    TestReport, TestReporter, TestStatus, TestSuiteStats, extract_suite_name,
    format_fuzz_failure_context,
};
use crate::commands::test::TestDescriptor;
use crate::context::AssertFailure;
use crate::formatter::FormatterContext;
use std::borrow::Cow;
use std::io::{Write, stdout};
use std::path::Path;

pub(crate) struct TeamCityReporter;

impl TeamCityReporter {
    pub(crate) const fn new() -> Self {
        Self
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

    fn formatter_for_test<'a>(&self, test: &'a TestReport) -> Option<FormatterContext<'a>> {
        let failure = test.execution.as_ref()?.failure.as_ref()?;

        Some(FormatterContext {
            contract_abi: test.abi.clone(),
            accounts: Cow::Borrowed(&failure.accounts),
            build_cache: Cow::Borrowed(&failure.build_cache),
            emulations: Cow::Borrowed(&failure.emulations),
            known_addresses: Cow::Borrowed(&failure.known_addresses),
            known_code_cells: Cow::Borrowed(&failure.known_code_cells),
            show_bodies: test.show_bodies,
            has_wallets_config: false,
            available_wallets: vec![],
            backtrace: test.backtrace,
            fork_net: None,
            network: None,
        })
    }

    fn format_test_failure(
        &self,
        test: &TestReport,
    ) -> (String, String, Option<String>, Option<String>) {
        let mut message = "Test failed".to_string();
        let mut details = String::new();
        let mut expected: Option<String> = None;
        let mut actual: Option<String> = None;
        let formatter = self.formatter_for_test(test);

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
                        if let Some(formatter) = &formatter {
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
                    _ if bin_failure.is_ord() => {
                        message = "Comparison failed".to_string();
                        if let Some(formatter) = &formatter {
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
                AssertFailure::Decimal(failure) => {
                    message = "Decimal equality failed".to_string();
                    expected = Some(failure.right.clone());
                    actual = Some(failure.left.clone());
                }
                AssertFailure::Fail(_) => {
                    message = "Test assertion failed".to_string();
                }
                AssertFailure::Assume(_) => {
                    message = "Test assumption failed".to_string();
                }
                AssertFailure::GetMethod(failure) => {
                    message = FormatterContext::strip_ansi_text(
                        &FormatterContext::format_get_method_assert_failure_title(failure),
                    );
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

        if let Some(exec) = &test.execution
            && let Some(fuzz) = &exec.fuzz
        {
            let fuzz_details = format_fuzz_failure_context(fuzz);
            if !fuzz_details.is_empty() {
                if !details.is_empty() {
                    details.push('\n');
                }
                details.push_str(&fuzz_details);
            }
        }

        (message, details, expected, actual)
    }

    fn flow_id(&self, file_path: &Path) -> String {
        self.escape_name(&file_path.display().to_string())
    }

    fn emit_message(&self, message: String) -> anyhow::Result<()> {
        println!("{message}");
        stdout().flush()?;
        Ok(())
    }
}

impl Default for TeamCityReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl TestReporter for TeamCityReporter {
    fn on_testing_started(&mut self) -> anyhow::Result<()> {
        self.emit_message("##teamcity[testingStarted]".to_owned())
    }

    fn on_testing_finished(&mut self, _stats: &TestSuiteStats) -> anyhow::Result<()> {
        self.emit_message("##teamcity[testingFinished]".to_owned())
    }

    fn on_suite_started(
        &mut self,
        file_path: &Path,
        _tests: &[TestDescriptor],
    ) -> anyhow::Result<()> {
        let suite_name = extract_suite_name(file_path);
        let escaped_name = self.escape_name(&suite_name);
        let flow_id = self.flow_id(file_path);

        self.emit_message(format!("##teamcity[flowStarted flowId='{flow_id}']"))?;
        self.emit_message(format!(
            "##teamcity[testSuiteStarted name='{escaped_name}' nodeId='suite_{escaped_name}' parentNodeId='0' nodeType='file' flowId='{flow_id}' locationHint='file://{}']",
            file_path.display(),
        ))
    }

    fn on_suite_finished(
        &mut self,
        file_path: &Path,
        _stats: &TestSuiteStats,
    ) -> anyhow::Result<()> {
        let suite_name = extract_suite_name(file_path);
        let escaped_name = self.escape_name(&suite_name);
        let flow_id = self.flow_id(file_path);

        self.emit_message(format!(
            "##teamcity[testSuiteFinished name='{escaped_name}' nodeId='suite_{escaped_name}' flowId='{flow_id}']"
        ))?;
        self.emit_message(format!("##teamcity[flowFinished flowId='{flow_id}']"))
    }

    fn on_test_started(&mut self, test: &TestReport) -> anyhow::Result<()> {
        let test_name = self.escape_name(&test.name);
        let suite_name = self.escape_name(&extract_suite_name(Path::new(&test.file_path)));
        let location = self.escape_name(&format!("{}:{}", test.file_path.display(), test.name));
        let flow_id = self.flow_id(&test.file_path);

        self.emit_message(format!(
            "##teamcity[testStarted name='{test_name}' nodeId='test_{test_name}' parentNodeId='suite_{suite_name}' flowId='{flow_id}' locationHint='tolk_qn://{location}']"
        ))
    }

    fn on_test_finished(&mut self, test: &TestReport) -> anyhow::Result<()> {
        let test_name = self.escape_name(&test.name);
        let suite_name = self.escape_name(&extract_suite_name(Path::new(&test.file_path)));
        let duration_ms = test.duration.as_millis();
        let flow_id = self.flow_id(&test.file_path);

        match test.status {
            TestStatus::Failed => {
                let (message, details, expected, actual) = self.format_test_failure(test);

                if let (Some(exp), Some(act)) = (expected, actual) {
                    self.emit_message(format!(
                        "##teamcity[testFailed type='comparisonFailure' name='{}' nodeId='test_{}' duration='{}' flowId='{}' message='{}' details='{}' expected='{}' actual='{}']",
                        test_name,
                        test_name,
                        duration_ms,
                        flow_id,
                        self.escape_name(&message),
                        self.escape_name(&details),
                        self.escape_name(&exp),
                        self.escape_name(&act),
                    ))?;
                } else {
                    self.emit_message(format!(
                        "##teamcity[testFailed name='{}' nodeId='test_{}' duration='{}' flowId='{}' message='{}' details='{}']",
                        test_name,
                        test_name,
                        duration_ms,
                        flow_id,
                        self.escape_name(&message),
                        self.escape_name(&details),
                    ))?;
                }
            }
            TestStatus::Skipped | TestStatus::Todo => {
                if let Some(details) = test.details.as_deref() {
                    self.emit_message(format!(
                        "##teamcity[testIgnored name='{test_name}' nodeId='test_{test_name}' duration='{duration_ms}' flowId='{flow_id}' message='{}']",
                        self.escape_name(details),
                    ))?;
                } else {
                    self.emit_message(format!(
                        "##teamcity[testIgnored name='{test_name}' nodeId='test_{test_name}' duration='{duration_ms}' flowId='{flow_id}']"
                    ))?;
                }
            }
            TestStatus::Passed => {}
        }

        self.emit_message(format!(
            "##teamcity[testFinished name='{test_name}' nodeId='test_{test_name}' duration='{duration_ms}' parentNodeId='suite_{suite_name}' flowId='{flow_id}']"
        ))
    }
}
