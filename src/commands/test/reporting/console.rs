use super::{TestExecutionContext, TestReport, TestReporter, TestStatus, TestSuiteStats};
use crate::commands::test::TestDescriptor;
use crate::context::AssertFailure;
use crate::formatter::FormatterContext;
use crate::retrace;
use acton_config::{color::OwoColorize, test::BacktraceMode};
use acton_debug::exit_codes;
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use ton_executor::get::{GetMethodResult, GetMethodResultSuccess};

#[derive(Debug, Clone)]
pub(crate) struct ConsoleConfig {
    pub show_output: bool,
    pub project_root: PathBuf,
}

impl Default for ConsoleConfig {
    fn default() -> Self {
        Self {
            show_output: true,
            project_root: PathBuf::from("."),
        }
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

    fn format_fuzz_suffix(&self, test: &TestReport) -> String {
        let Some(exec) = &test.execution else {
            return String::new();
        };
        let Some(fuzz) = &exec.fuzz else {
            return String::new();
        };

        let label = if fuzz.total_runs == 1 { "run" } else { "runs" };
        format!(
            " {}",
            format!("({} {label}, seed {})", fuzz.total_runs, fuzz.seed).dimmed()
        )
    }
}

impl TestReporter for ConsoleReporter {
    fn on_testing_started(&mut self) -> anyhow::Result<()> {
        println!(
            "\n{} {}\n",
            " TEST ".bold().on_blue(),
            self.config.project_root.display().dimmed()
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

        let relative = pathdiff::diff_paths(file_path, &self.config.project_root);
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
        let fuzz_suffix = self.format_fuzz_suffix(test);

        if test.status == TestStatus::Passed {
            println!(
                "  {} {} {}{}{}",
                "✓".green(),
                beautified_name,
                time_value.green(),
                time_unit.green().dimmed(),
                fuzz_suffix
            );
        }

        if test.status == TestStatus::Skipped {
            if let Some(description) = test.details.as_deref() {
                println!(
                    "  {} {} {}{}{}",
                    "○".dimmed(),
                    beautified_name,
                    "[".dimmed(),
                    description.dimmed(),
                    "]".dimmed()
                );
            } else {
                println!(
                    "  {} {} {}",
                    "○".dimmed(),
                    beautified_name,
                    "skipped".dimmed()
                );
            }
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
                "  {} {} {}{}{}",
                "✗".red(),
                beautified_name,
                time_value.red(),
                time_unit.red().dimmed(),
                fuzz_suffix
            );

            let Some(exec) = &test.execution else {
                if let Some(message) = &test.message {
                    println!("    {} {}", "└─".dimmed(), message.bright_red());
                } else {
                    println!("    {} {}", "└─".dimmed(), "Test failed".bright_red());
                }
                return Ok(());
            };

            if let Some(fuzz) = &exec.fuzz
                && let Some(case) = &fuzz.failed_case
            {
                println!(
                    "    {} Fuzz case {}/{}",
                    "├─".dimmed(),
                    case.run.to_string().yellow(),
                    fuzz.total_runs.to_string().yellow()
                );
                if !case.inputs.is_empty() {
                    let inputs = case
                        .inputs
                        .iter()
                        .map(|(name, value)| format!("{name}={value}"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    println!("    {} Inputs: {}", "├─".dimmed(), inputs);
                }
            }

            let Some(failure_context) = &exec.failure else {
                if let Some(message) = &test.message {
                    println!("    {} {}", "└─".dimmed(), message.bright_red());
                } else {
                    println!("    {} {}", "└─".dimmed(), "Test failed".bright_red());
                }
                return Ok(());
            };

            let formatter = FormatterContext {
                accounts: Cow::Borrowed(&failure_context.accounts),
                build_cache: Cow::Borrowed(&failure_context.build_cache),
                emulations: Cow::Borrowed(&failure_context.emulations),
                known_addresses: Cow::Borrowed(&failure_context.known_addresses),
                known_code_cells: Cow::Borrowed(&failure_context.known_code_cells),
                show_bodies: test.show_bodies,
                has_wallets_config: failure_context.has_wallets_config,
                available_wallets: failure_context.available_wallets.clone(),
                backtrace: test.backtrace,
                fork_net: failure_context.fork_net.clone(),
                network: failure_context.network.clone(),
            };

            match &failure_context.get_result {
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
        process_assert_failure(assert_failure, test, &fmt, result);
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
        process_nonzero_exit_code(test, result, result.vm_exit_code, &fmt);
    }
}

fn process_assert_failure(
    failure: &AssertFailure,
    test: &TestReport,
    fmt: &FormatterContext<'_>,
    result: &GetMethodResultSuccess,
) {
    if let AssertFailure::GetMethod(failure) = &failure {
        let formatted = fmt.format_get_method_assert_failure(failure);
        let mut lines = formatted.lines();
        let Some(header) = lines.next() else {
            println!("    {}", "└─".dimmed());
            return;
        };

        println!("    {} {}", "└─".dimmed(), header);

        let mut groups: Vec<(String, Vec<String>)> = Vec::new();
        for line in lines {
            if line.trim().is_empty() {
                continue;
            }

            if line.starts_with("  ")
                && let Some((_, nested)) = groups.last_mut()
            {
                nested.push(line.trim_start().to_string());
            } else if line.starts_with("  ") {
                groups.push((line.trim_start().to_string(), Vec::new()));
            } else {
                groups.push((line.to_string(), Vec::new()));
            }
        }

        for (idx, (line, nested)) in groups.iter().enumerate() {
            let is_last = idx + 1 == groups.len();
            let branch = if is_last { "└─" } else { "├─" };
            println!("      {} {}", branch.dimmed(), line);

            let nested_branch = if is_last { " " } else { "│" };
            for nested_line in nested {
                println!("      {}     {}", nested_branch.dimmed(), nested_line);
            }
        }

        return;
    }

    if let AssertFailure::WalletNotFound(failure) = &failure {
        let formatted = fmt.format_wallet_not_found_message(failure);
        let has_location = failure.location.is_some();
        for (idx, line) in formatted.lines().enumerate() {
            if idx == 0 {
                let branch = if has_location { "├─" } else { "└─" };
                println!(
                    "    {} {} {}",
                    branch.dimmed(),
                    "Error:".bright_red(),
                    FormatterContext::highlight_actual_expected(line)
                );
            } else if line.trim().is_empty() {
                if has_location {
                    println!("    {}", "│".dimmed());
                } else {
                    println!();
                }
            } else {
                let prefix = if has_location { "│" } else { " " };
                println!("    {} {}", prefix.dimmed(), line);
            }
        }

        if let Some(location) = failure.location.as_ref() {
            println!("    {} at {}", "└─".dimmed(), location.format().dimmed());
        }
        return;
    }

    if let AssertFailure::ExternalSendNotAccepted(failure) = &failure {
        println!(
            "    {} {} {}",
            "└─".dimmed(),
            "Error:".bright_red(),
            FormatterContext::highlight_actual_expected(&failure.message)
        );

        let mut details = Vec::new();
        let status = if failure.external_not_accepted {
            "external message was not accepted"
        } else {
            "external send failed before producing transactions"
        };
        details.push(format!("{} {}", "Status:".dimmed(), status.yellow()));
        details.push(format!(
            "{} {}",
            "Reason:".dimmed(),
            failure.reason.yellow()
        ));
        let backtrace = assertion_backtrace_lines(test, result);
        if let Some(exit_code) = failure.vm_exit_code {
            details.push(format!(
                "{}{}",
                "exit_code=".dimmed(),
                exit_code.to_string().yellow()
            ));
            if test.backtrace.is_none() {
                details.push(format!(
                    "Re-run with {} to get more information",
                    "--backtrace full".yellow()
                ));
            }
            if let Some(description) = fmt
                .format_compute_phase_failure_description(failure.destination.as_ref(), exit_code)
            {
                details.push(format!(
                    "{} {}",
                    "Compute phase failed:".dimmed(),
                    description.yellow()
                ));
            }
        }
        if !failure.missing_libraries.is_empty() {
            details.push(format!(
                "{} {}",
                "Missing libraries:".dimmed(),
                failure.missing_libraries.join(", ").yellow()
            ));
        }
        let has_location = failure.location.is_some();

        for (idx, detail) in details.iter().enumerate() {
            let has_next = idx + 1 < details.len() || has_location;
            let branch = if has_next { "├─" } else { "└─" };
            println!("        {} {}", branch.dimmed(), detail);
        }

        if let Some(location) = &failure.location {
            println!(
                "        {} at {}",
                "└─".dimmed(),
                location.format().dimmed()
            );
            for line in backtrace {
                println!("              {line}");
            }
        }

        return;
    }

    if let Some(message) = &failure.message() {
        if message.is_empty() {
            println!("    {}", "└─".dimmed());
        } else {
            let highlighted_message = FormatterContext::highlight_actual_expected(message);
            let mut lines = highlighted_message.lines();
            let Some(first_line) = lines.next() else {
                println!("    {}", "└─".dimmed());
                return;
            };
            let detail_lines = lines.collect::<Vec<_>>();
            println!(
                "    {} {} {}",
                "└─".dimmed(),
                "Error:".bright_red(),
                first_line
            );
            for line in detail_lines {
                println!("        {line}");
            }
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
            failure.left_ty_idx,
            failure.right_ty_idx,
            &failure.source_map,
        );

        for line in diff_output.lines() {
            println!("        {line}");
        }
    }

    if let AssertFailure::Bin(failure) = &failure
        && failure.operator == "!="
    {
        let value =
            fmt.format_tuple_value(&failure.left, failure.left_ty_idx, &failure.source_map, 8);
        println!("       Values are equal but expected to be different:");
        println!("         {}", value.dimmed());
    }

    if let AssertFailure::Bin(failure) = &failure
        && failure.is_ord()
    {
        let left =
            fmt.format_tuple_value(&failure.left, failure.left_ty_idx, &failure.source_map, 8);
        let right =
            fmt.format_tuple_value(&failure.right, failure.right_ty_idx, &failure.source_map, 8);

        println!("        Actual:   {}", left.red());
        println!("        Expected: {}", right.green());
    }

    if let AssertFailure::Decimal(failure) = &failure {
        println!("        Actual:   {}", failure.left.red());
        println!("        Expected: {}", failure.right.green());
    }

    if let AssertFailure::TransactionNotFound(failure) = &failure {
        let params = fmt.format_search_transaction_parameters(failure);
        let tx_tree = fmt.format_transaction_list(&failure.txs);

        let from_addr = failure.params.from.as_ref().and_then(|dp| match dp {
            crate::context::DisplayParam::Value(a) => Some(a.clone()),
            crate::context::DisplayParam::Function => None,
        });
        let to_addr = failure.params.to.as_ref().and_then(|dp| match dp {
            crate::context::DisplayParam::Value(a) => Some(a.clone()),
            crate::context::DisplayParam::Function => None,
        });
        let diff_output = format!(
            "{tx_tree}\nCannot find transaction from {} to {}\nwith:\n{}",
            fmt.format_address(&failure.txs, from_addr.as_ref()),
            fmt.format_address(&failure.txs, to_addr.as_ref()),
            params.join("\n"),
        );

        for line in diff_output.lines() {
            println!("        {line}");
        }
    }

    if let AssertFailure::TransactionIsFound(failure) = &failure {
        let params = fmt.format_search_transaction_parameters(failure);
        let tx_tree = fmt.format_transaction_list(&failure.txs);

        let from_addr2 = failure.params.from.as_ref().and_then(|dp| match dp {
            crate::context::DisplayParam::Value(a) => Some(a.clone()),
            crate::context::DisplayParam::Function => None,
        });
        let to_addr2 = failure.params.to.as_ref().and_then(|dp| match dp {
            crate::context::DisplayParam::Value(a) => Some(a.clone()),
            crate::context::DisplayParam::Function => None,
        });
        let from_to = if failure.params.from.is_none() && failure.params.to.is_none() {
            ""
        } else {
            &format!(
                " from {} to {}",
                fmt.format_address(&failure.txs, from_addr2.as_ref()),
                fmt.format_address(&failure.txs, to_addr2.as_ref()),
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

    if let AssertFailure::ExternalMessageNotFound(failure) = &failure {
        let params = fmt.format_external_message_search_parameters(failure);
        let tx_tree = fmt.format_transaction_list(&failure.txs);
        let diff_output = format!(
            "{tx_tree}\nCannot find external message {}\n{}{}",
            failure.message_name.purple().bold(),
            if params.is_empty() { "" } else { "with:\n" },
            params.join("\n"),
        );

        for line in diff_output.lines() {
            println!("        {line}");
        }
    }

    let backtrace = assertion_backtrace_lines(test, result);
    if let Some(location) = &failure.location() {
        let branch = if backtrace.is_empty() {
            "└─"
        } else {
            "├─"
        };
        println!(
            "      {} at {}",
            branch.dimmed(),
            location.format().dimmed()
        );
    }

    if !backtrace.is_empty() {
        println!("      {} Backtrace:", "└─".dimmed());
        for line in backtrace {
            println!("            {line}");
        }
    }
}

fn assertion_backtrace_lines(test: &TestReport, result: &GetMethodResultSuccess) -> Vec<String> {
    if test.backtrace != Some(BacktraceMode::Full) {
        return Vec::new();
    }

    retrace::find_exception_info(&result.vm_log, &test.source_map)
        .map(|info| FormatterContext::format_backtrace(&info.backtrace))
        .unwrap_or_default()
}

fn process_nonzero_exit_code(
    test: &TestReport,
    result: &GetMethodResultSuccess,
    exit_code: i32,
    fmt: &FormatterContext<'_>,
) {
    println!(
        "    {} exit_code={}",
        "└─".dimmed(),
        exit_code.to_string().yellow()
    );

    let exit_code_info = retrace::find_exception_info(&result.vm_log, &test.source_map);
    let get_method_info = fmt.find_failed_get_method_exception(test);

    let mut groups: Vec<(String, Vec<String>)> = Vec::new();

    if let Some(info) = &get_method_info {
        let mut nested = vec![format!(
            "at {}",
            FormatterContext::format_location(&info.loc).dimmed()
        )];
        nested.extend(FormatterContext::format_backtrace(&info.backtrace));
        groups.push(("Get method:".to_string(), nested));
    }

    if let Some(info) = &exit_code_info {
        if get_method_info.is_some() {
            let mut nested = FormatterContext::format_backtrace(&info.backtrace);
            if nested.is_empty() {
                nested.push(format!(
                    "at {}",
                    FormatterContext::format_location(&info.loc).dimmed()
                ));
            }
            groups.push(("Called from:".to_string(), nested));
        } else {
            groups.push((
                format!(
                    "at {}",
                    FormatterContext::format_location(&info.loc).dimmed()
                ),
                FormatterContext::format_backtrace(&info.backtrace),
            ));
        }
    } else if test.backtrace.is_none() {
        groups.push((
            format!(
                "Re-run with {} to get more information",
                "--backtrace full".yellow()
            ),
            Vec::new(),
        ));
    }

    if let Some(info) = exit_codes::find_for_phase(exit_code, exit_codes::ExitCodePhase::Compute) {
        groups.push((info.description.dimmed().to_string(), Vec::new()));
        groups.push((format!("Phase: {}", info.phase.dimmed()), Vec::new()));
    } else if let Some(info) = &exit_code_info {
        let description = if info.description.is_empty() {
            format!("uncaught exception {}", info.errno)
        } else {
            info.description.clone()
        };
        groups.push((description.dimmed().to_string(), Vec::new()));
    }

    if let Some(message) = FormatterContext::special_get_method_exit_code_message(exit_code) {
        groups.push((message, Vec::new()));
    }

    for (idx, (line, nested)) in groups.iter().enumerate() {
        let is_last = idx + 1 == groups.len();
        let branch = if is_last { "└─" } else { "├─" };
        println!("      {} {}", branch.dimmed(), line);

        let nested_branch = if is_last { " " } else { "│" };
        for nested_line in nested {
            println!("      {}     {}", nested_branch.dimmed(), nested_line);
        }
    }
}
