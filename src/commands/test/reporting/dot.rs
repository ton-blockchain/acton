use super::{
    TestReport, TestReporter, TestStatus, TestSuiteStats, format_fuzz_failure_context,
    formatter_for_failed_test,
};
use crate::commands::test::TestDescriptor;
use crate::formatter::FormatterContext;
use acton_config::color::OwoColorize;
use std::io::{Write, stdout};
use std::path::Path;
use ton_executor::get::GetMethodResult;

pub(crate) struct DotReporter {
    show_output: bool,
    tests: Vec<TestReport>,
    count_suites: usize,
}

impl DotReporter {
    pub(crate) const fn new(show_output: bool) -> Self {
        Self {
            show_output,
            tests: Vec::new(),
            count_suites: 0,
        }
    }

    fn print_status_dot(&self, status: &TestStatus) {
        print!("{}", self.get_colored_status_char(status));
        stdout().flush().ok();
    }

    const fn get_status_char(&self, status: &TestStatus) -> char {
        match status {
            TestStatus::Passed => '·',
            TestStatus::Failed => 'x',
            TestStatus::Skipped => '○',
            TestStatus::Todo => '□',
        }
    }

    fn get_colored_status_char(&self, status: &TestStatus) -> String {
        let char = self.get_status_char(status);
        match status {
            TestStatus::Passed => char.to_string().green().to_string(),
            TestStatus::Failed => char.to_string().red().to_string(),
            TestStatus::Skipped => char.to_string().yellow().to_string(),
            TestStatus::Todo => char.to_string().purple().to_string(),
        }
    }

    fn print_test_output(&self, test: &TestReport, output_type: &str) -> anyhow::Result<()> {
        let Some(execution) = &test.execution else {
            return Ok(());
        };

        let (content, label, color_stderr) = match output_type {
            "stdout" => {
                if execution.stdout.trim().is_empty() {
                    return Ok(());
                }
                (&execution.stdout, "stdout", false)
            }
            "stderr" => {
                if execution.stderr.trim().is_empty() {
                    return Ok(());
                }
                (&execution.stderr, "stderr", true)
            }
            _ => return Ok(()),
        };

        println!();
        println!("{} | {} > {}", label, test.file_path.display(), test.name);

        let lines: Vec<&str> = content.trim().lines().collect();
        for line in lines {
            if color_stderr {
                println!("{}", line.bright_red());
            } else {
                println!("{line}");
            }
        }

        Ok(())
    }

    fn print_all_test_outputs(&self) -> anyhow::Result<()> {
        for test in &self.tests {
            if test.status == TestStatus::Failed {
                let detailed_message = self.format_detailed_failure(test);
                let has_detailed_message = detailed_message
                    .as_deref()
                    .is_some_and(|message| !message.trim().is_empty());
                let hide_header_message =
                    self.header_message_duplicates_details(test, detailed_message.as_deref());

                println!();
                if let Some(message) = &test.message
                    && !hide_header_message
                {
                    println!("{} {}: {}", "✗".red(), test.name, message);
                } else {
                    println!("{} {}", "✗".red(), test.name);
                }

                if !has_detailed_message && let Some(location) = &test.location {
                    println!("  {}", location.format().dimmed());
                }

                if let Some(execution) = &test.execution
                    && let Some(fuzz) = &execution.fuzz
                {
                    for line in format_fuzz_failure_context(fuzz).lines() {
                        println!("  {}", line.dimmed());
                    }
                }

                self.print_detailed_failure(detailed_message.as_deref());
            }

            self.print_test_output(test, "stdout")?;
            self.print_test_output(test, "stderr")?;
        }

        Ok(())
    }

    fn print_skip_and_todo_details(&self) {
        let mut has_details = false;

        for test in &self.tests {
            let (symbol, default_description) = match test.status {
                TestStatus::Skipped => ("○".yellow().to_string(), "skipped"),
                TestStatus::Todo => ("□".purple().bold().to_string(), "TODO"),
                _ => continue,
            };

            if !has_details {
                println!();
                has_details = true;
            }

            let description = test.details.as_deref().unwrap_or(default_description);
            println!(
                "{} {} {}{}{}",
                symbol,
                test.name,
                "[".dimmed(),
                description.dimmed(),
                "]".dimmed()
            );
        }
    }

    fn print_summary(&self, stats: &TestSuiteStats) {
        let mut parts = Vec::new();

        if stats.passed > 0 || (stats.failed == 0 && stats.skipped == 0 && stats.todo == 0) {
            parts.push(format!(
                "{} {} {}",
                "✓".green().bold(),
                stats.passed.to_string().green().bold(),
                "passed".green().bold()
            ));
        }

        if stats.failed > 0 {
            parts.push(format!(
                "{} {} {}",
                "✗".red().bold(),
                stats.failed.to_string().red().bold(),
                "failed".red().bold()
            ));
        }

        if stats.skipped > 0 {
            parts.push(format!(
                "{} {} {}",
                "○".yellow().bold(),
                stats.skipped.to_string().yellow().bold(),
                "skipped".yellow().bold()
            ));
        }

        if stats.todo > 0 {
            parts.push(format!(
                "{} {} {}",
                "□".purple().bold(),
                stats.todo.to_string().purple().bold(),
                "todo".purple().bold()
            ));
        }

        if parts.is_empty() {
            return;
        }

        let file_str = if self.count_suites == 1 {
            "file"
        } else {
            "files"
        };
        println!(
            "\n {} {} {} {}",
            parts.join(", "),
            "in".dimmed(),
            self.count_suites.to_string().green(),
            file_str.green().dimmed()
        );

        if stats.failed > 0 {
            println!("\n{}", "Some tests failed.".red());
        }
    }

    fn format_detailed_failure(&self, test: &TestReport) -> Option<String> {
        let exec = test.execution.as_ref()?;
        let formatter = formatter_for_failed_test(test)?;

        if test.gas_limit.is_some_and(|limit| exec.gas_used > limit) {
            return None;
        }

        if let Some(assert_failure) = &exec.assert_failure {
            return Some(formatter.format_detailed_assert_failure(assert_failure));
        }

        let failure = exec.failure.as_ref()?;
        match &failure.get_result {
            GetMethodResult::Success(result) => {
                Some(formatter.format_detailed_exit_code(test, result, result.vm_exit_code))
            }
            GetMethodResult::Error(error) => Some(
                test.detailed_message
                    .clone()
                    .unwrap_or_else(|| error.error.to_string()),
            ),
        }
    }

    fn header_message_duplicates_details(
        &self,
        test: &TestReport,
        detailed_message: Option<&str>,
    ) -> bool {
        let Some(message) = test.message.as_deref() else {
            return false;
        };
        let Some(detailed_message) = detailed_message else {
            return false;
        };
        let Some(first_line) = detailed_message.trim().lines().next() else {
            return false;
        };

        let first_line = FormatterContext::strip_ansi_text(first_line);
        let message = FormatterContext::strip_ansi_text(message);
        let highlighted_message = FormatterContext::strip_ansi_text(
            &FormatterContext::highlight_actual_expected(&message),
        );

        let first_line = first_line.trim();
        first_line == message.trim() || first_line == highlighted_message.trim()
    }

    fn print_detailed_failure(&self, detailed_message: Option<&str>) {
        let Some(detailed_message) = detailed_message else {
            return;
        };
        for line in detailed_message.trim().lines() {
            println!("  {line}");
        }
    }
}

impl TestReporter for DotReporter {
    fn on_suite_started(
        &mut self,
        _file_path: &Path,
        _tests: &[TestDescriptor],
    ) -> anyhow::Result<()> {
        self.count_suites += 1;
        Ok(())
    }

    fn on_test_started(&mut self, _test: &TestReport) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_test_finished(&mut self, test: &TestReport) -> anyhow::Result<()> {
        self.print_status_dot(&test.status);
        let mut test = test.clone();
        if !self.show_output
            && let Some(execution) = test.execution.as_mut()
        {
            execution.stdout.clone_from(&execution.debug_output);
            execution.stderr.clear();
        }
        self.tests.push(test);
        Ok(())
    }

    fn on_testing_finished(&mut self, stats: &TestSuiteStats) -> anyhow::Result<()> {
        println!();
        self.print_all_test_outputs()?;
        self.print_skip_and_todo_details();
        self.print_summary(stats);
        Ok(())
    }
}
