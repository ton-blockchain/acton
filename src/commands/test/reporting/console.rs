use super::{TestExecutionContext, TestReport, TestReporter, TestStatus, TestSuiteStats};
use crate::commands::test::TestDescriptor;
use crate::context::AssertFailure;
use crate::formatter::FormatterContext;
use crate::{exit_codes, retrace};
use owo_colors::OwoColorize;
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use ton_executor::get::{GetMethodResult, GetMethodResultSuccess};
use ton_source_map::SourceLocation;

const CANNOT_RUN_GET_METHOD_OD_UNDEPLOYED_CONTRACT: i32 = 678;
const CANNOT_RUN_GET_METHOD_OF_CONTRACT_WITHOUT_CODE: i32 = 679;

#[derive(Debug, Clone)]
pub(crate) struct ConsoleConfig {
    pub show_output: bool,
}

impl Default for ConsoleConfig {
    fn default() -> Self {
        Self { show_output: true }
    }
}

pub(crate) struct ConsoleReporter {
    config: ConsoleConfig,
    count_suites: usize,
}

impl ConsoleReporter {
    pub(crate) const fn new(config: ConsoleConfig) -> Self {
        Self {
            config,
            count_suites: 0,
        }
    }

    fn beatify_test_name(&self, name: &str) -> String {
        name.trim_start_matches("test ")
            .trim_start_matches("test-")
            .trim_start_matches("test_")
            .to_string()
    }
}

impl TestReporter for ConsoleReporter {
    fn on_testing_started(&mut self) -> anyhow::Result<()> {
        let cwd = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
        println!(
            "\n{} {}\n",
            " TEST ".bold().on_blue(),
            cwd.display().dimmed()
        );
        Ok(())
    }

    fn on_testing_finished(&mut self, stats: &TestSuiteStats) -> anyhow::Result<()> {
        let mut parts = Vec::new();

        // Show 0 passed only if no tests at all
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

        if !parts.is_empty() {
            let summary = parts.join(", ");
            let suites_count = self.count_suites;
            let file_str = if suites_count == 1 { "file" } else { "files" };

            println!(
                "\n {} {} {} {}",
                summary,
                "in".dimmed(),
                suites_count.to_string().green(),
                file_str.green().dimmed()
            );
        }

        if stats.failed > 0 {
            println!("\n{}", "Some tests failed.".red());
        }

        Ok(())
    }

    fn on_suite_started(
        &mut self,
        file_path: &Path,
        tests: &[TestDescriptor],
    ) -> anyhow::Result<()> {
        self.count_suites += 1;

        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let relative = pathdiff::diff_paths(file_path, cwd);
        let relative_path = relative.unwrap_or_else(|| file_path.to_owned());

        println!(
            " {} {} {}",
            ">".dimmed(),
            relative_path.display(),
            format!(
                "({} test{})",
                tests.len(),
                if tests.len() == 1 { "" } else { "s" }
            )
            .dimmed()
        );

        Ok(())
    }

    fn on_suite_finished(
        &mut self,
        _file_path: &Path,
        _stats: &TestSuiteStats,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_test_finished(&mut self, test: &TestReport) -> anyhow::Result<()> {
        let beautified_name = self.beatify_test_name(&test.name);
        let duration_ms = test.duration.as_millis();
        let (time_value, time_unit) = if duration_ms > 0 {
            (duration_ms.to_string(), "ms")
        } else {
            (test.duration.as_micros().to_string(), "μs")
        };

        if test.status == TestStatus::Passed {
            println!(
                "  {} {} {}{}",
                "✓".green(),
                beautified_name,
                time_value.green(),
                time_unit.green().dimmed()
            );
        }

        if test.status == TestStatus::Skipped {
            println!(
                "  {} {} {}",
                "○".dimmed(),
                beautified_name,
                "skipped".dimmed()
            );
        }

        if test.status == TestStatus::Todo {
            let description = test.details.as_deref().unwrap_or("TODO");
            println!(
                "  {} {} {}{}{}",
                "□".purple().bold(),
                beautified_name,
                "[".dimmed(),
                description.dimmed(),
                "]".dimmed()
            );
        }

        if test.status == TestStatus::Failed {
            println!(
                "  {} {} {}{}",
                "✗".red(),
                beautified_name,
                time_value.red(),
                time_unit.red().dimmed()
            );

            let Some(exec) = &test.execution else {
                anyhow::bail!("Test execution context is missing for failed test")
            };

            let formatter = FormatterContext {
                contract_abi: test.abi.clone(),
                accounts: Cow::Borrowed(&exec.accounts),
                build_cache: Cow::Borrowed(&exec.build_cache),
                emulations: Cow::Borrowed(&exec.emulations),
                known_addresses: Cow::Borrowed(&exec.known_addresses),
                known_code_cells: Cow::Borrowed(&exec.known_code_cells),
                backtrace: test.backtrace,
                fork_net: None,
                network: None,
                api_key: None,
            };

            match &exec.get_result {
                GetMethodResult::Success(result) => {
                    process_test_fail(test, exec, formatter, result);
                }
                GetMethodResult::Error(error) => {
                    println!("    {} {}", "└─".dimmed(), error.error.yellow());
                }
            }
        }

        if self.config.show_output
            && let Some(exec) = &test.execution
        {
            if !exec.stdout.trim().is_empty() {
                println!("    {} Test output:", "└─".dimmed());
                for line in exec.stdout.trim().lines() {
                    println!("       {line}");
                }
            }

            if !exec.stderr.trim().is_empty() {
                println!("    {} Test stderr:", "└─".dimmed());
                for line in exec.stderr.trim().lines() {
                    println!("       {}", line.bright_red());
                }
            }
        }

        Ok(())
    }
}

fn process_test_fail(
    test: &TestReport,
    exec: &TestExecutionContext,
    fmt: FormatterContext<'_>,
    result: &GetMethodResultSuccess,
) {
    if test.gas_limit.is_some_and(|limit| exec.gas_used > limit) {
        println!(
            "    {} Gas limit exceeded: used {}, limit {}",
            "└─".dimmed(),
            exec.gas_used.to_string().bright_red(),
            test.gas_limit.unwrap_or(0).to_string().green()
        );
        // since the gas limit is exceeded, other possible faults are of no concern
        return;
    }

    if let Some(assert_failure) = &exec.assert_failure {
        process_assert_failure(assert_failure, test, &fmt);
        // since assertions set the exit code to 567, we don't want to process exit codes
        return;
    }

    if exec.expected_exit_code != 0 {
        println!(
            "    {} Expected exit_code={}, got={}",
            "└─".dimmed(),
            exec.expected_exit_code.to_string().green(),
            result.vm_exit_code.to_string().bright_red()
        );
    }

    if exec.expected_exit_code == 0 {
        process_nonzero_exit_code(test, result, result.vm_exit_code);
    }
}

fn process_assert_failure(failure: &AssertFailure, test: &TestReport, fmt: &FormatterContext<'_>) {
    if let Some(message) = &failure.message() {
        if message.is_empty() {
            println!("    {}", "└─".dimmed());
        } else {
            let highlighted_message = FormatterContext::highlight_actual_expected(message);
            println!(
                "    {} {} {}",
                "└─".dimmed(),
                "Error:".bright_red(),
                highlighted_message
            );
        }
    } else {
        println!("    {}", "└─".dimmed());
    }

    if let AssertFailure::Bin(failure) = &failure
        && failure.operator == "=="
    {
        let diff_output = fmt.format_tuple_diff(
            &failure.left,
            &failure.right,
            &failure.left_type,
            &failure.right_type,
        );

        for line in diff_output.lines() {
            println!("        {line}");
        }
    }

    if let AssertFailure::Bin(failure) = &failure
        && failure.operator == "!="
    {
        let value = fmt.format_tuple_value(&failure.left, &failure.left_type, 8);
        println!("       Values are equal but expected to be different:");
        println!("         {}", value.dimmed());
    }

    if let AssertFailure::Bin(failure) = &failure
        && failure.is_ord()
    {
        let left = fmt.format_tuple_value(&failure.left, &failure.left_type, 8);
        let right = fmt.format_tuple_value(&failure.right, &failure.right_type, 8);

        println!("        Actual:   {}", left.red());
        println!("        Expected: {}", right.green());
    }

    if let AssertFailure::TransactionNotFound(failure) = &failure {
        let params = fmt.format_search_transaction_parameters(failure, test.abi.clone());
        let tx_tree = fmt.format(&failure.txs);

        let diff_output = format!(
            "{tx_tree}\nCannot find transaction from {} to {}\nwith:\n{}",
            fmt.format_address(&failure.txs, &failure.params.from),
            fmt.format_address(&failure.txs, &failure.params.to),
            params.join("\n"),
        );

        for line in diff_output.lines() {
            println!("        {line}");
        }
    }

    if let AssertFailure::TransactionIsFound(failure) = &failure {
        let params = fmt.format_search_transaction_parameters(failure, test.abi.clone());
        let tx_tree = fmt.format(&failure.txs);

        let from_to = if failure.params.from.is_none() && failure.params.to.is_none() {
            ""
        } else {
            &format!(
                " from {} to {}",
                fmt.format_address(&failure.txs, &failure.params.from),
                fmt.format_address(&failure.txs, &failure.params.to),
            )
        };

        let diff_output = format!(
            "{tx_tree}\nUnexpected transaction{from_to}\n{}{}",
            if params.is_empty() { "" } else { "with:\n" },
            params.join("\n"),
        );

        for line in diff_output.lines() {
            println!("        {line}");
        }
    }

    if let Some(location) = &failure.location() {
        println!("      {} at {}", "└─".dimmed(), location.format().dimmed());
    }
}

fn process_nonzero_exit_code(test: &TestReport, result: &GetMethodResultSuccess, exit_code: i32) {
    println!(
        "    {} exit_code={}",
        "└─".dimmed(),
        exit_code.to_string().yellow()
    );

    let exit_code_info = retrace::find_exception_info(&result.vm_log, &test.source_map);

    if let Some(info) = &exit_code_info {
        if let Some(loc) = &info.loc {
            println!(
                "      {} at {}",
                "├─".dimmed(),
                format!(
                    "{}:{}:{}",
                    SourceLocation::normalize_path(&loc.file),
                    loc.line + 1,
                    loc.column + 2
                )
                .dimmed(),
            );

            let backtrace_lines = FormatterContext::format_backtrace(&info.backtrace);
            for line in backtrace_lines {
                println!("      {}     {}", "│".dimmed(), line);
            }
        } else if test.backtrace.is_none() {
            println!(
                "      {} Re-run with {} to get more information",
                "├─".dimmed(),
                "--backtrace full".yellow()
            );
        }

        if !info.description.is_empty() {
            println!("      {} {}", "├─".dimmed(), info.description.dimmed());
        }
    }

    if let Some(info) = exit_codes::find(exit_code) {
        if exit_code_info.is_none() {
            // Don't show duplicate info
            println!("      {} {}", "├─".dimmed(), info.description.dimmed());
        }
        println!("      {} Phase: {}", "└─".dimmed(), info.phase.dimmed());
    }

    // Special throw exit codes
    if exit_code == CANNOT_RUN_GET_METHOD_OD_UNDEPLOYED_CONTRACT {
        println!(
            "      {} Cannot run method of not deployed contract, make sure you're deployed contract first or passed {}",
            "└─".dimmed(),
            "--fork-net".yellow(),
        );
    } else if exit_code == CANNOT_RUN_GET_METHOD_OF_CONTRACT_WITHOUT_CODE {
        println!(
            "      {} Cannot run method of contract without code",
            "└─".dimmed()
        );
    }
}
