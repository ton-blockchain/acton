use super::{TestReport, TestReporter, TestStatus, TestSuiteStats};
use crate::commands::test::TestDescriptor;
use crate::context::AssertFailure;
use crate::formatter::FormatterContext;
use crate::retrace;
use emulator::exit_codes;
use emulator::get_executor::GetMethodResult;
use owo_colors::OwoColorize;
use std::path::Path;
use tolkc::source_map::SourceLocation;

#[derive(Debug, Clone)]
pub struct ConsoleConfig {
    pub show_output: bool,
}

impl Default for ConsoleConfig {
    fn default() -> Self {
        Self { show_output: true }
    }
}

pub struct ConsoleReporter {
    config: ConsoleConfig,
    count_suites: usize,
}

impl ConsoleReporter {
    pub fn new(config: ConsoleConfig) -> Self {
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
        _file_path: &str,
        tests: &[TestDescriptor],
    ) -> anyhow::Result<()> {
        self.count_suites += 1;

        let cwd = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
        let relative_path = Path::new(_file_path)
            .strip_prefix(&cwd)
            .unwrap_or_else(|_| Path::new(_file_path));

        println!(
            " {} {} {}",
            ">".dimmed(),
            relative_path.display(),
            format!(
                "({} test{})",
                tests.len(),
                if tests.len() != 1 { "s" } else { "" }
            )
            .dimmed()
        );

        Ok(())
    }

    fn on_suite_finished(
        &mut self,
        _file_path: &str,
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
        } else if test.status == TestStatus::Failed {
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

            let gas_limit_exceeded = if let Some(limit) = test.gas_limit {
                exec.gas_used > limit
            } else {
                false
            };

            let formatter = FormatterContext {
                contract_abi: test.abi.clone(),
                accounts: exec.accounts.clone(),
                build_cache: exec.build_cache.clone(),
                emulations: exec.emulations.clone(),
                known_addresses: exec.known_addresses.clone(),
                known_code_cells: exec.known_code_cells.clone(),
                backtrace: test.backtrace.clone(),
            };

            match &exec.get_result {
                GetMethodResult::Success(result) => {
                    let exit_code = result.vm_exit_code as i64;

                    let exit_code_info =
                        retrace::find_exception_info(&result.vm_log, &test.source_map);

                    if gas_limit_exceeded {
                        println!(
                            "    {} Gas limit exceeded: used {}, limit {}",
                            "└─".dimmed(),
                            exec.gas_used.to_string().red(),
                            test.gas_limit.unwrap_or(0).to_string().green()
                        );
                    } else if let Some(ref assert_failure) = exec.assert_failure {
                        if let Some(message) = &assert_failure.message() {
                            if !message.is_empty() {
                                let highlighted_message =
                                    FormatterContext::highlight_actual_expected(message);
                                println!(
                                    "    {} {} {}",
                                    "└─".dimmed(),
                                    "Error:".bright_red(),
                                    highlighted_message
                                );
                            } else {
                                println!("    {}", "└─".dimmed());
                            }
                        } else {
                            println!("    {}", "└─".dimmed());
                        }

                        if let AssertFailure::Bin(assert_failure) = &assert_failure
                            && assert_failure.operator == "=="
                        {
                            let diff_output = formatter.format_tuple_diff(
                                &assert_failure.left,
                                &assert_failure.right,
                                &assert_failure.left_type,
                                &assert_failure.right_type,
                            );

                            for line in diff_output.lines() {
                                println!("        {line}");
                            }
                        }

                        if let AssertFailure::Bin(assert_failure) = &assert_failure
                            && assert_failure.operator == "!="
                        {
                            println!("       Values are equal but expected to be different:");
                            let value = formatter.format_tuple_value(
                                &assert_failure.left,
                                &assert_failure.left_type,
                                8,
                            );
                            println!("         {}", value.dimmed());
                        }

                        if let AssertFailure::Bin(assert_failure) = &assert_failure
                            && assert_failure.is_ord()
                        {
                            let left = formatter.format_tuple_value(
                                &assert_failure.left,
                                &assert_failure.left_type,
                                8,
                            );

                            let right = formatter.format_tuple_value(
                                &assert_failure.right,
                                &assert_failure.right_type,
                                8,
                            );

                            println!("        Actual:   {}", left.red());
                            println!("        Expected: {}", right.green());
                        }

                        if let AssertFailure::TransactionNotFound(assert_failure) = &assert_failure
                        {
                            let params = formatter
                                .format_search_transaction_parameters(assert_failure, &test.abi);

                            let diff_output = format!(
                                "{}\nCannot find transaction from {} to {}\nwith:\n{}",
                                formatter.format(&assert_failure.txs),
                                formatter.format_address(
                                    &assert_failure.txs,
                                    &assert_failure.params.from
                                ),
                                formatter
                                    .format_address(&assert_failure.txs, &assert_failure.params.to),
                                params.join("\n"),
                            );

                            for line in diff_output.lines() {
                                println!("        {line}");
                            }
                        }

                        if let AssertFailure::TransactionIsFound(assert_failure) = &assert_failure {
                            let params = formatter
                                .format_search_transaction_parameters(assert_failure, &test.abi);

                            let diff_output = format!(
                                "{}\nUnexpected transaction from {} to {}\n{}{}",
                                formatter.format(&assert_failure.txs),
                                formatter.format_address(
                                    &assert_failure.txs,
                                    &assert_failure.params.from
                                ),
                                formatter.format_address(
                                    &assert_failure.txs,
                                    &assert_failure.params.to,
                                ),
                                if !params.is_empty() { "with:\n" } else { "" },
                                params.join("\n"),
                            );

                            for line in diff_output.lines() {
                                println!("        {line}");
                            }
                        }

                        if let Some(location) = &assert_failure.location()
                            && !location.is_empty()
                        {
                            println!("      {} at {}", "└─".dimmed(), location.dimmed());
                        }
                    } else if exec.expected_exit_code != 0 {
                        println!(
                            "    {} Expected exit_code={}, got={}",
                            "└─".dimmed(),
                            exec.expected_exit_code.to_string().green(),
                            exit_code.to_string().bright_red()
                        );
                    } else {
                        println!(
                            "    {} exit_code={}",
                            "└─".dimmed(),
                            exit_code.to_string().yellow()
                        );

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
                                if !info.backtrace.is_empty() {
                                    let max_function_name_len = info
                                        .backtrace
                                        .iter()
                                        .filter_map(|loc| loc.context.event_function.as_ref())
                                        .map(|name| name.len() + 2)
                                        .max()
                                        .unwrap_or(0);

                                    let backtrace_lines =
                                        info.backtrace.iter().rev().filter_map(|loc| {
                                            loc.context.event_function.as_ref().map(|func_name| {
                                                let location = format!(
                                                    "{}:{}:{}",
                                                    SourceLocation::normalize_path(&loc.loc.file),
                                                    loc.loc.line + 1,
                                                    loc.loc.column + 2
                                                );
                                                format!(
                                                    "{:<width$} at {}",
                                                    func_name.green(),
                                                    location.dimmed(),
                                                    width = max_function_name_len
                                                )
                                            })
                                        });

                                    for line in backtrace_lines {
                                        println!("      {}     {}", "│".dimmed(), line);
                                    }
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

                        if let Some(info) = exit_codes::get_exit_code_info(exit_code) {
                            if exit_code_info.is_none() {
                                // Don't show duplicate info
                                println!("      {} {}", "├─".dimmed(), info.description.dimmed());
                            }
                            println!("      {} Phase: {}", "└─".dimmed(), info.phase.dimmed());
                        } else if exit_code == 678 {
                            println!(
                                "      {} Cannot run method of not deployed contract, make sure you're deployed contract first or passed {}",
                                "└─".dimmed(),
                                "--fork-net".yellow(),
                            );
                        } else if exit_code == 679 {
                            println!(
                                "      {} Cannot run method of contract without code",
                                "└─".dimmed()
                            );
                        }
                    }
                }
                GetMethodResult::Error(error) => {
                    println!("    {} {}", "└─".dimmed(), error.error.yellow());
                }
            }
        } else {
            match test.status {
                TestStatus::Skipped => {
                    println!(
                        "  {} {} {}",
                        "○".dimmed(),
                        beautified_name,
                        "skipped".dimmed()
                    );
                }
                TestStatus::Todo => {
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
                TestStatus::Ignored => {
                    println!(
                        "  {} {} {}",
                        "○".dimmed(),
                        beautified_name,
                        "ignored".dimmed()
                    );
                }
                _ => {}
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
