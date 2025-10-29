use crate::context::{
    AnyExecutor, AssertFailure, BuildCache, Context, KnownAddresses,
    TransactionGenericAssertFailure,
};
use crate::dap::DapMessage;
use crate::debug_context::DebugContext;
use crate::formatter::FormatterContext;
use crate::{asserts_exts, exts, io_exts};
use abi::{ContractAbi, contract_abi};
use anyhow::anyhow;
use crossbeam_channel::{Receiver, Sender, unbounded};
use dap::prelude::Request;
use emulator::blockchain::Blockchain;
use emulator::emulator::Emulator;
use emulator::exit_codes;
use emulator::get_executor::{GetExecutor, GetMethodParams, GetMethodResult};
use emulator::step_get_executor::StepGetExecutor;
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use owo_colors::OwoColorize;
use regex::Regex;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use std::{fs, process};
use teamcity::TeamcityReporter;
use tolkc::source_map::{DebugLocation, SourceLocation, SourceMap};
use tonlib_core::TonAddress;
use tonlib_core::cell::{ArcCell, Cell, CellBuilder};
use tonlib_core::tlb_types::tlb::TLB;
use tree_sitter::Node;
use tycho_types::models::ShardAccount;
use vmlogs::parser::VmLine;

mod annotations;
mod teamcity;

const CRC16: crc::Crc<u16> = crc::Crc::<u16>::new(&crc::CRC_16_XMODEM);

pub fn test_cmd(
    path: &String,
    filter: Option<&str>,
    teamcity: bool,
    debug: bool,
    backtrace: Option<String>,
) -> Result<(), anyhow::Error> {
    let metadata = fs::metadata(path)?;
    let test_files = if metadata.is_file() {
        if !path.ends_with("_test.tolk") {
            return Err(anyhow!("File must end with _test.tolk"));
        }
        vec![path.clone()]
    } else if metadata.is_dir() {
        find_test_files_recursively(path)?
    } else {
        return Err(anyhow!("Path '{}' is neither a file nor a directory", path));
    };

    let cwd = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());

    if teamcity {
        TeamcityReporter::on_testing_started();
    }

    if !teamcity {
        println!(
            "\n{} {}\n",
            " TEST ".bold().on_blue(),
            cwd.display().dimmed()
        );
    }

    let mut total_passed = 0;
    let mut total_failed = 0;
    let mut total_skipped = 0;
    let mut total_todo = 0;

    for (index, file) in test_files.iter().enumerate() {
        let result = run_tests_for_file(&file, filter, teamcity, debug, backtrace.clone());
        match result {
            Ok(stats) => {
                total_passed += stats.passed;
                total_failed += stats.failed;
                total_skipped += stats.skipped;
                total_todo += stats.todo;

                if index > 0 && test_files.len() != index - 1 {
                    println!()
                }
            }
            Err(err) => {
                println!("{} {}", "Error:".red(), err);
                total_failed += 1;
            }
        }
    }

    let mut parts = Vec::new();

    if total_passed > 0 {
        parts.push(format!(
            "{} {} {}",
            "✓".green().bold(),
            total_passed.to_string().green().bold(),
            "passed".green().bold()
        ));
    }

    if total_failed > 0 {
        parts.push(format!(
            "{} {} {}",
            "✗".red().bold(),
            total_failed.to_string().red().bold(),
            "failed".red().bold()
        ));
    }

    if total_skipped > 0 {
        parts.push(format!(
            "{} {} {}",
            "○".yellow().bold(),
            total_skipped.to_string().yellow().bold(),
            "skipped".yellow().bold()
        ));
    }

    if total_todo > 0 {
        parts.push(format!(
            "{} {} {}",
            "□".purple().bold(),
            total_todo.to_string().purple().bold(),
            "todo".purple().bold()
        ));
    }

    let file_str = if test_files.len() == 1 {
        "file"
    } else {
        "files"
    };

    if !parts.is_empty() {
        let summary = parts.join(", ");
        println!(
            "\n {} {} {} {}",
            summary,
            "in".dimmed(),
            test_files.len().to_string().green(),
            file_str.green().dimmed()
        );
    }

    if total_failed > 0 {
        println!("\n{}", "Some tests failed.".red());
    }

    if teamcity {
        TeamcityReporter::on_testing_finished();
    }

    if total_failed > 0 {
        process::exit(1)
    }
    Ok(())
}

fn find_test_files_recursively(dir_path: &str) -> Result<Vec<String>, anyhow::Error> {
    let mut test_files = Vec::new();

    fn visit_dir(dir: &Path, test_files: &mut Vec<String>) -> Result<(), anyhow::Error> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                if path.file_name() == Some("node_modules".as_ref()) {
                    return Ok(());
                }

                visit_dir(&path, test_files)?;
            } else if let Some(file_name) = path.file_name() {
                if file_name.to_string_lossy().ends_with("_test.tolk") {
                    test_files.push(path.to_string_lossy().to_string());
                }
            }
        }
        Ok(())
    }

    visit_dir(Path::new(dir_path), &mut test_files)?;
    test_files.sort();
    Ok(test_files)
}

fn has_entry_function(root_node: &Node, content: &str) -> bool {
    let mut cursor = root_node.walk();
    for child in root_node.children(&mut cursor) {
        if child.kind() == "function_declaration" {
            if let Some(name_node) = child.child_by_field_name("name") {
                let name = name_node.utf8_text(content.as_bytes()).unwrap_or("");
                if name == "main" || name == "onInternalMessage" {
                    return true;
                }
            }
        }
    }
    false
}

#[derive(Debug)]
struct TestStats {
    passed: usize,
    failed: usize,
    skipped: usize,
    todo: usize,
}

fn run_tests_for_file(
    file: &str,
    filter: Option<&str>,
    teamcity: bool,
    debug: bool,
    backtrace: Option<String>,
) -> Result<TestStats, anyhow::Error> {
    let content = match fs::read_to_string(file) {
        Ok(content) => content,
        Err(err) => {
            return Err(anyhow!("Error reading file '{}': {}", file, err));
        }
    };

    let tests = find_all_test(file.to_string(), &content);

    let abi = contract_abi(content.as_str(), file);

    let executable_code = inject_locations_into_expect_calls(&content, file);
    let tmp_test_filename = file.to_owned() + "_test.tolk";

    fs::write(&tmp_test_filename, executable_code)?;

    let need_debug_info = debug || backtrace == Some("full".to_string());
    let compilation_result = tolkc::compile_fast(Path::new(&tmp_test_filename), need_debug_info);
    let result = match compilation_result {
        tolkc::CompilerResult::Success(result) => {
            let _ = fs::remove_file(&tmp_test_filename);

            let code_cell = ArcCell::from_boc_b64(&*result.code_boc64)?;
            let data_cell = ArcCell::default();

            let stats = run_all_tests(
                file,
                tests,
                &code_cell,
                &data_cell,
                &abi,
                &result.source_map.unwrap_or(Default::default()),
                filter,
                teamcity,
                debug,
                backtrace,
            );
            Ok(stats)
        }
        tolkc::CompilerResult::Error(error) => {
            let _ = fs::remove_file(&tmp_test_filename);
            let normalized_filepath = error.message.replace(&tmp_test_filename, file);
            let trimmed_message = normalized_filepath.trim();
            Err(anyhow!(trimmed_message.to_string()))
        }
    };

    result
}

fn run_all_tests(
    file_path: &str,
    tests: Vec<TestDescriptor>,
    code_cell: &Arc<Cell>,
    data_cell: &Arc<Cell>,
    abi: &ContractAbi,
    source_map: &SourceMap,
    filter: Option<&str>,
    teamcity: bool,
    debug: bool,
    backtrace: Option<String>,
) -> TestStats {
    let filtered_tests = if let Some(pattern) = filter {
        let regex = match Regex::new(pattern) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Invalid regex pattern '{}': {}", pattern, e);
                return TestStats {
                    passed: 0,
                    failed: 0,
                    skipped: 0,
                    todo: 0,
                };
            }
        };
        tests
            .into_iter()
            .filter(|test| regex.is_match(&test.name))
            .collect::<Vec<_>>()
    } else {
        tests
    };

    if !filtered_tests.is_empty() {
        if teamcity {
            TeamcityReporter::on_test_suite_started(file_path);
        }

        let cwd = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
        let relative_path = Path::new(file_path)
            .strip_prefix(&cwd)
            .unwrap_or_else(|_| Path::new(file_path));
        println!(
            " {} {} {}",
            ">".dimmed(),
            relative_path.display().to_string(),
            format!("({} tests)", filtered_tests.len()).dimmed()
        );
    }

    let dest_address = contract_address(&code_cell);

    let mut passed = 0;
    let mut failed = 0;
    let mut skipped = 0;
    let mut todo = 0;

    let mut build_cache = BuildCache::new();
    let mut known_addresses = KnownAddresses::new();

    let (req_receiver, dap_sender) = if debug {
        crate::dap::start_dap_server()
    } else {
        let (_, req_receiver) = unbounded::<Request>();
        let (dap_message_sender, _) = unbounded::<DapMessage>();
        (req_receiver, dap_message_sender)
    };

    for test in filtered_tests.iter() {
        if teamcity {
            TeamcityReporter::on_test_started(&test.name, file_path);
        }

        if test.annotations.contains(&"todo".to_string()) {
            if teamcity {
                TeamcityReporter::on_test_ignored(&test.name, 0);
            }
            let description = test.todo_description.as_deref().unwrap_or("TODO");
            println!(
                "  {} {} {}{}{}",
                "□".purple().bold(),
                test.name,
                "[".dimmed(),
                description.dimmed(),
                "]".dimmed()
            );
            todo += 1;
            continue;
        }

        if test.annotations.contains(&"skip".to_string()) {
            if teamcity {
                TeamcityReporter::on_test_ignored(&test.name, 0);
            }
            println!("  {} {} {}", "○".dimmed(), test.name, "skipped".dimmed());
            skipped += 1;
            continue;
        }

        let start_time = Instant::now();
        let result = execute_test(
            test,
            &code_cell,
            &data_cell,
            &dest_address,
            &mut build_cache,
            &mut known_addresses,
            abi,
            source_map,
            debug,
            req_receiver.clone(),
            dap_sender.clone(),
        );
        let duration = start_time.elapsed();
        let TestResult {
            captured_stdout,
            captured_stderr,
            assert_failure,
            expected_exit_code: dyn_expected_exit_code,
            accounts,
            ..
        } = result;

        let (exit_code, gas_used) = match &result.get_result {
            GetMethodResult::Success(result) => {
                let gas_used = result.gas_used.parse::<u64>().unwrap_or(0);
                (result.vm_exit_code, gas_used)
            }
            GetMethodResult::Error(_) => (999, 0),
        };

        let duration_ms = duration.as_millis();
        let (time_value, time_unit) = if duration_ms > 0 {
            (duration_ms.to_string(), "ms")
        } else {
            (duration.as_micros().to_string(), "μs")
        };

        let expected_exit_code = dyn_expected_exit_code
            .or_else(|| test.expected_exit_code)
            .unwrap_or(0);
        let mut test_passed = exit_code == expected_exit_code;

        let gas_limit_exceeded = if let Some(limit) = test.gas_limit {
            if gas_used > limit {
                test_passed = false;
                true
            } else {
                false
            }
        } else {
            false
        };

        if test_passed {
            println!(
                "  {} {} {}{}",
                "✓".green(),
                test.name,
                time_value.green(),
                time_unit.green().dimmed()
            );
            passed += 1;
        } else {
            println!(
                "  {} {} {}{}",
                "✗".red(),
                test.name,
                time_value.red(),
                time_unit.red().dimmed()
            );
            failed += 1;

            let formatter = FormatterContext {
                contract_abi: abi.clone(),
                accounts,
                build_cache: build_cache.clone(),
                known_addresses: known_addresses.clone(),
            };

            match &result.get_result {
                GetMethodResult::Success(result) => {
                    let exit_code = result.vm_exit_code as i64;

                    let exit_code_info = find_exception_info(&result.vm_log, source_map);

                    if gas_limit_exceeded {
                        println!(
                            "    {} Gas limit exceeded: used {}, limit {}",
                            "└─".dimmed(),
                            gas_used.to_string().red(),
                            test.gas_limit.unwrap().to_string().green()
                        );
                    } else if let Some(ref assert_failure) = assert_failure {
                        if let Some(message) = &assert_failure.message() {
                            if !message.is_empty() {
                                let highlighted_message = highlight_actual_expected(message);
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
                                println!("        {}", line);
                            }
                        }

                        if let AssertFailure::Bin(assert_failure) = &assert_failure
                            && assert_failure.operator == "!="
                        {
                            println!(
                                "       {}",
                                "Values are equal but expected to be different:"
                            );
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
                                &assert_failure.left,
                                &assert_failure.left_type,
                                8,
                            );

                            println!("        Actual:   {}", left.red());
                            println!("        Expected: {}", right.green());
                        }

                        if let AssertFailure::TransactionNotFound(assert_failure) = &assert_failure
                        {
                            let params = format_search_transaction_parameters(assert_failure);

                            let diff_output = format!(
                                "{}\nCannot find transaction from {} to {}\nwith:\n{}",
                                formatter.format(&assert_failure.txs),
                                formatter.format_address(
                                    &assert_failure.txs,
                                    &assert_failure.params.from
                                ),
                                formatter.format_address(
                                    &assert_failure.txs,
                                    &Some(assert_failure.params.to.clone())
                                ),
                                params.join("\n"),
                            );

                            for line in diff_output.lines() {
                                println!("        {}", line);
                            }
                        }

                        if let AssertFailure::TransactionIsFound(assert_failure) = &assert_failure {
                            let params = format_search_transaction_parameters(assert_failure);

                            let diff_output = format!(
                                "{}\nUnexpected transaction from {} to {}\n{}{}",
                                formatter.format(&assert_failure.txs),
                                formatter.format_address(
                                    &assert_failure.txs,
                                    &assert_failure.params.from
                                ),
                                formatter.format_address(
                                    &assert_failure.txs,
                                    &Some(assert_failure.params.to.clone())
                                ),
                                if params.len() != 0 { "with:\n" } else { "" },
                                params.join("\n"),
                            );

                            for line in diff_output.lines() {
                                println!("        {}", line);
                            }
                        }

                        if let Some(location) = &assert_failure.location() {
                            if !location.is_empty() {
                                println!("      {} at {}", "└─".dimmed(), location.dimmed());
                            }
                        }
                    } else {
                        if expected_exit_code != 0 {
                            println!(
                                "    {} Expected exit_code={}, got={}",
                                "└─".dimmed(),
                                expected_exit_code.to_string().green(),
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
                                            normalize_path(&loc.file),
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
                                                loc.context.event_function.as_ref().map(
                                                    |func_name| {
                                                        let location = format!(
                                                            "{}:{}:{}",
                                                            normalize_path(&loc.loc.file),
                                                            loc.loc.line + 1,
                                                            loc.loc.column + 2
                                                        );
                                                        format!(
                                                            "{:<width$} at {}",
                                                            func_name.green(),
                                                            location.dimmed(),
                                                            width = max_function_name_len
                                                        )
                                                    },
                                                )
                                            });

                                        for line in backtrace_lines {
                                            println!("      {}     {}", "│".dimmed(), line);
                                        }
                                    }
                                } else if backtrace.is_none() {
                                    println!(
                                        "      {} Re-run with {} to get more information",
                                        "├─".dimmed(),
                                        "--backtrace full".yellow()
                                    );
                                }
                                if !info.description.is_empty() {
                                    println!(
                                        "      {} {}",
                                        "├─".dimmed(),
                                        info.description.dimmed()
                                    );
                                }
                            }

                            if let Some(info) = exit_codes::get_exit_code_info(exit_code) {
                                if exit_code_info.is_none() {
                                    // Don't show duplicate info
                                    println!(
                                        "      {} {}",
                                        "├─".dimmed(),
                                        info.description.dimmed()
                                    );
                                }
                                println!("      {} Phase: {}", "└─".dimmed(), info.phase.dimmed());
                            } else if exit_code == 678 {
                                println!(
                                    "      {} {}",
                                    "└─".dimmed(),
                                    "Cannot run method of not deployed contract"
                                );
                            } else if exit_code == 679 {
                                println!(
                                    "      {} {}",
                                    "└─".dimmed(),
                                    "Cannot run method of contract without code"
                                );
                            }
                        }
                    }
                }
                GetMethodResult::Error(error) => {
                    println!("    {} {}", "└─".dimmed(), error.error.yellow());
                }
            }

            if teamcity {
                TeamcityReporter::on_test_failed(
                    &test.name,
                    duration_ms,
                    assert_failure.as_ref(),
                    &formatter,
                );
            }
        }

        if !captured_stdout.trim().is_empty() {
            println!("    {} Test output:", "└─".dimmed());
            for line in captured_stdout.trim().lines() {
                println!("       {}", line);
            }
        }

        if !captured_stderr.trim().is_empty() {
            println!("    {} Test stderr:", "└─".dimmed());
            for line in captured_stderr.trim().lines() {
                println!("       {}", line.bright_red());
            }
        }

        if teamcity {
            TeamcityReporter::on_test_finished(&test.name, file_path, duration_ms);
        }
    }

    if !filtered_tests.is_empty() && teamcity {
        TeamcityReporter::on_test_suite_finished(file_path);
    }

    TestStats {
        passed,
        failed,
        skipped,
        todo,
    }
}

fn normalize_path(file: &String) -> String {
    let normalized = file.replace("_test.tolk_test.tolk", "_test.tolk");

    if let Ok(cwd) = std::env::current_dir() {
        let file_path = Path::new(&normalized);

        if let Ok(relative) = file_path.strip_prefix(&cwd) {
            let relative_str = relative.to_string_lossy();
            if relative_str.len() < normalized.len()
                || normalized.starts_with(cwd.to_string_lossy().as_ref())
            {
                return relative_str.to_string();
            }
        }
    }

    normalized
}

fn format_search_transaction_parameters(
    assert_failure: &TransactionGenericAssertFailure,
) -> Vec<String> {
    let mut params = vec![];
    if let Some(bounced) = assert_failure.params.bounced {
        params.push(format!(
            "  bounced={}",
            if bounced {
                "true".green().to_string()
            } else {
                "false".red().to_string()
            }
        ))
    }
    if let Some(deploy) = assert_failure.params.deploy {
        params.push(format!(
            "  deploy={}",
            if deploy {
                "true".green().to_string()
            } else {
                "false".red().to_string()
            }
        ))
    }
    if let Some(exit_code) = assert_failure.params.exit_code {
        params.push(format!(
            "  exit_code={}",
            if exit_code == 0 {
                "0".green().to_string()
            } else {
                exit_code.to_string().red().to_string()
            }
        ))
    }
    params
}

struct ExceptionInfo {
    description: String,
    loc: Option<SourceLocation>,
    backtrace: Vec<DebugLocation>,
}

fn find_exception_info(vm_logs: &String, source_map: &SourceMap) -> Option<ExceptionInfo> {
    let lines = vmlogs::parser::parse_lines(vm_logs.as_str());

    let exception = lines.iter().rfind(|line| match line {
        Ok(VmLine::VmException { .. }) => true,
        _ => false,
    });
    let description = match exception {
        Some(Ok(VmLine::VmException { message, .. })) => message.to_string(),
        _ => "".to_string(),
    };

    let location = lines.iter().rfind(|line| match line {
        Ok(VmLine::VmLoc { .. }) => true,
        _ => false,
    });

    let (hash, offset) = match location {
        Some(Ok(VmLine::VmLoc { hash, offset })) => (hash.to_string(), offset.parse().unwrap_or(0)),
        _ => ("".to_string(), 0),
    };

    let loc = find_source_loc(source_map, &hash, offset);

    let backtrace = find_backtrace(source_map, lines);

    Some(ExceptionInfo {
        description,
        loc,
        backtrace,
    })
}

fn find_backtrace(
    source_map: &SourceMap,
    lines: Vec<Result<VmLine, String>>,
) -> Vec<DebugLocation> {
    let execution_path = lines
        .iter()
        .filter_map(|line| match line {
            Ok(VmLine::VmLoc { hash, offset }) => Some((hash, offset.parse().unwrap_or(0))),
            _ => None,
        })
        .flat_map(|(hash, offset)| {
            let Some(marks) = source_map.debug_marks.get(*hash) else {
                return vec![];
            };

            let debug_pairs = marks
                .iter()
                .filter(|(mark_offset, _)| return *mark_offset == offset)
                .collect::<Vec<_>>();

            let exact_locs = source_map
                .high_level
                .locations
                .iter()
                .filter(|loc| !loc.loc.file.is_empty() && !loc.loc.file.starts_with("@stdlib/"))
                .filter(|loc| {
                    debug_pairs
                        .iter()
                        .find(|(_, debug_id)| (*debug_id) as i64 == loc.idx)
                        .is_some()
                })
                .collect::<Vec<_>>();

            exact_locs
        })
        .collect::<Vec<_>>();

    let mut stack = vec![];

    for step in &execution_path {
        if step.context.event == Some("EnterFunction".to_string())
            || step.context.event == Some("EnterInlinedFunction".to_string())
        {
            if step.context.event_function.is_none() {
                continue;
            }

            stack.push(step);
        }
        if step.context.event == Some("AfterFunctionCall".to_string())
            || step.context.event == Some("LeaveInlinedFunction".to_string())
        {
            let event_function = &step.context.event_function;

            let Some(last) = stack.last() else {
                continue;
            };

            if last.context.event_function == *event_function {
                stack.pop();
            }
        }
    }
    stack.iter().map(|loc| (**loc).clone()).collect::<Vec<_>>()
}

fn find_source_loc(source_map: &SourceMap, hash: &String, offset: i32) -> Option<SourceLocation> {
    if source_map.high_level.locations.len() != 0 {
        let Some(marks) = source_map.debug_marks.get(hash) else {
            return None;
        };

        let mut debug_pairs = marks
            .iter()
            .filter(|(mark_offset, _)| return *mark_offset == offset)
            .collect::<Vec<_>>();

        if debug_pairs.is_empty() {
            // We can't always find the exact location, so try to find an approximate location
            debug_pairs = marks
                .iter()
                .rfind(|(mark_offset, _)| return offset > *mark_offset)
                .iter()
                .map(|pair| *pair)
                .collect::<Vec<_>>();
        }

        let exact_locs = source_map
            .high_level
            .locations
            .iter()
            .filter(|loc| !loc.loc.file.is_empty() && !loc.loc.file.starts_with("@stdlib/"))
            .filter(|loc| {
                debug_pairs
                    .iter()
                    .find(|(_, debug_id)| (*debug_id) as i64 == loc.idx)
                    .is_some()
            })
            .collect::<Vec<_>>();

        exact_locs.last().and_then(|l| Some(l.loc.clone()))
    } else {
        None
    }
}

struct TestResult {
    get_result: GetMethodResult,
    captured_stdout: String,
    captured_stderr: String,
    assert_failure: Option<AssertFailure>,
    expected_exit_code: Option<i32>,
    accounts: HashMap<String, ShardAccount>,
}

fn execute_test(
    test: &TestDescriptor,
    code_cell: &Arc<Cell>,
    data_cell: &Arc<Cell>,
    dest_address: &TonAddress,
    build_cache: &mut BuildCache,
    known_addresses: &mut KnownAddresses,
    abi: &ContractAbi,
    source_map: &SourceMap,
    debug: bool,
    req_receiver: Receiver<Request>,
    dap_sender: Sender<DapMessage>,
) -> TestResult {
    let params = GetMethodParams {
        code: code_cell.to_boc_b64(false).unwrap().to_string(),
        data: data_cell.to_boc_b64(false).unwrap().to_string(),
        verbosity: 5,
        libs: "".to_string(),
        address: dest_address.to_string(),
        unixtime: 0,
        balance: "10".to_string(),
        rand_seed: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        gas_limit: "0".to_string(),
        method_id: test.id,
        debug_enabled: true,
        extra_currencies: HashMap::new(),
        prev_blocks_info: None,
    };
    let mut get_executor = GetExecutor::new(params.clone());

    let mut emulator = Emulator::new();
    let mut blockchain = Blockchain::new();

    let mut ctx = Context {
        stdout_buffer: "".to_string(),
        stderr_buffer: "".to_string(),
        capture_test_output: true,
        assert_failure: &mut None,
        blockchain: &mut blockchain,
        emulator: &mut emulator,
        build_cache,
        known_addresses,
        abi: (*abi).clone(),
        expected_exit_code: &mut Some(BigInt::from(0)),
        dbg_ctx: &mut DebugContext::empty(),
        debug,
    };

    let (result, captured_stdout, captured_stderr, assert_failure, expected_exit_code) = if debug {
        let mut get_executor = StepGetExecutor::new(Default::default(), params.clone());

        exts::register_extensions(&mut get_executor, &mut ctx);
        io_exts::register_extensions(&mut get_executor, &mut ctx);
        asserts_exts::register_extensions(&mut get_executor, &mut ctx);

        let mut dbg_ctx = DebugContext::new(
            AnyExecutor::Get(get_executor.clone()),
            source_map,
            &req_receiver,
            dap_sender,
        );

        ctx.dbg_ctx = &mut dbg_ctx;

        get_executor.run_get_method(test.id, Default::default());

        ctx.dbg_ctx.process_incoming_requests(true).unwrap();

        let get_result = get_executor.finish_get_method();

        (
            get_result,
            ctx.stdout_buffer,
            ctx.stderr_buffer,
            (*ctx.assert_failure).clone(),
            ctx.expected_exit_code
                .clone()
                .map(|value| value.to_i32())
                .unwrap_or(None),
        )
    } else {
        exts::register_extensions(&mut get_executor, &mut ctx);
        io_exts::register_extensions(&mut get_executor, &mut ctx);
        asserts_exts::register_extensions(&mut get_executor, &mut ctx);

        let get_result = get_executor.run_get_method(Default::default(), params);

        (
            get_result,
            ctx.stdout_buffer,
            ctx.stderr_buffer,
            (*ctx.assert_failure).clone(),
            ctx.expected_exit_code
                .clone()
                .map(|value| value.to_i32())
                .unwrap_or(None),
        )
    };

    TestResult {
        get_result: result,
        captured_stdout,
        captured_stderr,
        assert_failure,
        expected_exit_code,
        accounts: blockchain.get_accounts().clone(),
    }
}

fn contract_address(code: &Arc<Cell>) -> TonAddress {
    let state_init = CellBuilder::new()
        .store_bit(false)
        .unwrap()
        .store_bit(false)
        .unwrap()
        .store_ref_cell_optional(Some(&code))
        .unwrap()
        .store_ref_cell_optional(Some(&ArcCell::default()))
        .unwrap()
        .store_bit(false)
        .unwrap()
        .build()
        .unwrap();

    let dest_address = TonAddress::new(0, state_init.cell_hash());
    dest_address
}

#[derive(Debug)]
struct TestDescriptor {
    pub file: String,
    pub id: i32,
    pub name: String,
    pub annotations: Vec<String>,
    pub expected_exit_code: Option<i32>,
    pub gas_limit: Option<u64>,
    pub todo_description: Option<String>,
}

fn find_all_test(file: String, content: &String) -> Vec<TestDescriptor> {
    let Ok(tree) = tolk_parser::parser::parse(&content) else {
        return vec![];
    };
    let root_node = tree.root_node();
    let mut cursor = root_node.walk();

    root_node
        .children(&mut cursor)
        .flat_map(|child| {
            if child.kind() == "get_method_declaration" {
                let name_node = child.child_by_field_name("name");
                let raw_name = name_node
                    .unwrap()
                    .utf8_text(content.as_bytes())
                    .unwrap()
                    .to_string();
                let name = raw_name
                    .strip_prefix("`")
                    .unwrap_or(&raw_name)
                    .strip_suffix("`")
                    .unwrap_or(&raw_name);

                // get fun `test-foo`() or get fun test_foo()
                if name.starts_with("test-") || name.starts_with("test_") {
                    let id = (CRC16.checksum(name.as_bytes()) & 0xff_ff) as i32 | 0x1_00_00;
                    let test_annotations = annotations::find_test_annotations(content, child);

                    return vec![TestDescriptor {
                        file: file.clone(),
                        id,
                        name: name.to_string(),
                        annotations: test_annotations.annotations,
                        expected_exit_code: test_annotations.expected_exit_code,
                        gas_limit: test_annotations.gas_limit,
                        todo_description: test_annotations.todo_description,
                    }];
                }
            };

            vec![]
        })
        .collect()
}

fn inject_locations_into_expect_calls(content: &str, file_path: &str) -> String {
    let Ok(tree) = tolk_parser::parser::parse(&content) else {
        return "".to_string();
    };
    let root_node = tree.root_node();

    let mut replacements = Vec::new();
    find_expect_calls(&root_node, content, file_path, &mut replacements);

    let mut result = content.to_string();

    if !has_entry_function(&root_node, &result) {
        result += "\n\nfun main() {}"
    }

    for (start, end, replacement) in replacements.into_iter().rev() {
        result.replace_range(start..end, &replacement);
    }

    result
}

fn find_expect_calls(
    node: &Node,
    content: &str,
    file_path: &str,
    replacements: &mut Vec<(usize, usize, String)>,
) {
    for i in 0..node.child_count() {
        let child = node.child(i).unwrap();
        find_expect_calls(&child, content, file_path, replacements);
    }

    if node.kind() != "function_call" {
        // fast path
        return;
    }

    let Some(callee_node) = node.child_by_field_name("callee") else {
        return;
    };

    if callee_node.kind() == "identifier"
        && callee_node.utf8_text(content.as_bytes()).unwrap_or("") == "expect"
    {
        let Some(args_node) = node.child_by_field_name("arguments") else {
            return;
        };

        let mut arg_count = 0;
        let mut cursor = args_node.walk();
        for child in args_node.children(&mut cursor) {
            if child.kind() == "call_argument" {
                arg_count += 1;
            }
        }

        // Don't add location if it already passed by the user
        if arg_count == 1 {
            let column = callee_node.start_position().column + 1;
            let start = args_node.end_byte() - 1;
            let end = args_node.end_byte() - 1;

            let lines: Vec<&str> = content[..start].lines().collect();
            let line_number = lines.len();

            let location = format!(", \"{file_path}:{line_number}:{column}\"",);
            replacements.push((start, end, location));
        }
    }
}

fn highlight_actual_expected(message: &str) -> String {
    let result = message
        .replace("<actual>", &"actual".red().to_string())
        .replace("<expected>", &"expected".green().to_string());

    result.to_string()
}
