use crate::commands::build::build_cmd;
use crate::commands::test::coverage::{
    Coverage, collect_coverage, generate_lcov_file, merge_coverages, print_coverage_summary,
};
use crate::commands::test::instrumentation::inject_locations_into_expect_calls;
use crate::config::ActonConfig;
use crate::context::{AnyExecutor, AssertFailure, BuildCache, Context, Emulations, KnownAddresses};
use crate::dap::DapMessage;
use crate::debug_context::DebugContext;
use crate::file_build_cache::FileBuildCache;
use crate::formatter::FormatterContext;
use crate::{asserts_exts, exts, io_exts, retrace};
use abi::{ContractAbi, contract_abi};
use anyhow::anyhow;
use crossbeam_channel::{Receiver, Sender, unbounded};
use dap::prelude::Request;
use emulator::blockchain::Blockchain;
use emulator::emulator::Emulator;
use emulator::executor::ExecutorVerbosity;
use emulator::exit_codes;
use emulator::get_executor::{GetExecutor, GetMethodParams, GetMethodResult};
use emulator::step_get_executor::StepGetExecutor;
use globset::{Glob, GlobSet, GlobSetBuilder};
use log::{debug, error};
use num_traits::ToPrimitive;
use owo_colors::OwoColorize;
use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use std::{fs, process};
use teamcity::TeamcityReporter;
use tolkc::source_map::{SourceLocation, SourceMap};
use tonlib_core::TonAddress;
use tonlib_core::cell::{ArcCell, Cell, CellBuilder};
use tonlib_core::tlb_types::tlb::TLB;
use tycho_types::models::ShardAccount;
use walkdir::WalkDir;

mod annotations;
mod coverage;
mod instrumentation;
mod teamcity;

const CRC16: crc::Crc<u16> = crc::Crc::<u16>::new(&crc::CRC_16_XMODEM);

#[derive(Debug, Clone)]
pub struct TestConfig {
    pub teamcity: bool,
    pub debug: bool,
    pub debug_port: u16,
    pub backtrace: Option<String>,
    pub coverage: bool,
    pub filter: Option<String>,
    pub coverage_format: Option<String>,
    pub exclude_patterns: Vec<String>,
    pub include_patterns: Vec<String>,
    pub clear_cache: bool,
}

#[derive(Debug)]
pub struct TestResult {
    pub get_result: GetMethodResult,
    pub captured_stdout: String,
    pub captured_stderr: String,
    pub assert_failure: Option<AssertFailure>,
    pub expected_exit_code: Option<i32>,
    pub accounts: HashMap<String, ShardAccount>,
}

#[derive(Debug)]
pub struct TestRunner<'a> {
    config: TestConfig,
    build_cache: BuildCache,
    file_build_cache: &'a mut FileBuildCache,
    known_addresses: KnownAddresses,
    known_code_cells: HashMap<String, String>,
    emulations: Emulations,
    req_receiver: Receiver<Request>,
    dap_sender: Sender<DapMessage>,
}

impl<'a> TestRunner<'a> {
    pub fn new(config: TestConfig, cache: &'a mut FileBuildCache) -> TestRunner<'a> {
        let (req_receiver, dap_sender) = if config.debug {
            crate::dap::start_dap_server(config.debug_port)
        } else {
            let (_, req_receiver) = unbounded::<Request>();
            let (dap_message_sender, _) = unbounded::<DapMessage>();
            (req_receiver, dap_message_sender)
        };

        Self {
            config,
            build_cache: BuildCache::new(),
            file_build_cache: cache,
            known_addresses: KnownAddresses::new(),
            known_code_cells: HashMap::new(),
            emulations: Emulations::new(),
            req_receiver,
            dap_sender,
        }
    }

    fn minimal_log_verbosity(&self) -> ExecutorVerbosity {
        if self.config.debug || self.config.backtrace == Some("full".to_owned()) {
            // for these modes we need all logs for work
            return ExecutorVerbosity::FullLocationStackVerbose;
        }

        if self.config.coverage {
            // for coverage, we need at least locations to map to actual source code
            return ExecutorVerbosity::FullLocation;
        }

        ExecutorVerbosity::Short
    }

    fn execute_test(
        &mut self,
        test: &TestDescriptor,
        code_cell: &Arc<Cell>,
        dest_address: &TonAddress,
        abi: &ContractAbi,
        source_map: &SourceMap,
    ) -> anyhow::Result<TestResult> {
        let verbosity = self.minimal_log_verbosity();
        let params = GetMethodParams {
            code: code_cell
                .to_boc_b64(false)
                .expect("Failed to encode code cell to BoC")
                .to_string(),
            data: ArcCell::default()
                .to_boc_b64(false)
                .expect("Failed to encode data cell to BoC")
                .to_string(),
            verbosity,
            libs: "".to_string(),
            address: dest_address.to_string(),
            unixtime: 0,
            balance: "10".to_string(),
            rand_seed: "0000000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
            gas_limit: "0".to_string(),
            method_id: test.id,
            debug_enabled: true,
            extra_currencies: HashMap::new(),
            prev_blocks_info: None,
        };

        let mut emulator = Emulator::new(verbosity);
        let mut blockchain = Blockchain::new();
        let mut libraries = vec![];

        let mut ctx = Context {
            config: &ActonConfig::load()?,
            stdout_buffer: "".to_string(),
            stderr_buffer: "".to_string(),
            capture_test_output: true,
            assert_failure: &mut None,
            blockchain: &mut blockchain,
            emulator: &mut emulator,
            build_cache: &mut self.build_cache,
            file_build_cache: &mut self.file_build_cache,
            known_addresses: &mut self.known_addresses,
            known_code_cells: &mut self.known_code_cells,
            emulations: &mut self.emulations,
            abi: (*abi).clone(),
            expected_exit_code: &mut None,
            dbg_ctx: &mut DebugContext::empty(),
            debug: self.config.debug,
            backtrace: self.config.backtrace.clone(),
            need_debug_info: self.config.debug
                || self.config.backtrace == Some("full".to_string())
                || self.config.coverage,
            libraries: &mut libraries,
            default_log_level: verbosity,
        };

        let (result, captured_stdout, captured_stderr, assert_failure, expected_exit_code) =
            if self.config.debug {
                let mut get_executor = StepGetExecutor::new(Default::default(), params.clone());

                exts::register_extensions(&mut get_executor, &mut ctx);
                io_exts::register_extensions(&mut get_executor, &mut ctx);
                asserts_exts::register_extensions(&mut get_executor, &mut ctx);

                let mut dbg_ctx = DebugContext::new(
                    AnyExecutor::Get(get_executor.clone()),
                    source_map,
                    &self.req_receiver,
                    self.dap_sender.clone(),
                    Some(test.name.clone()),
                );

                ctx.dbg_ctx = &mut dbg_ctx;

                get_executor.run_get_method(test.id, Default::default());

                ctx.dbg_ctx.process_incoming_requests(true)?;

                let get_result = get_executor.finish_get_method(&params.code);

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
                let mut get_executor = GetExecutor::new(params.clone());

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
                        .and_then(|value| value.to_i32()),
                )
            };

        Ok(TestResult {
            get_result: result,
            captured_stdout,
            captured_stderr,
            assert_failure,
            expected_exit_code,
            accounts: blockchain.get_accounts().clone(),
        })
    }
}

pub fn test_cmd(path: Option<String>, config: &TestConfig) -> anyhow::Result<()> {
    // First we need to build all contracts and generate all dependency files with code
    build_cmd(None, config.clear_cache, None)?;
    println!("     {} tests", "Running".green().bold());

    // If path is omitted, default to current directory
    let path = path.unwrap_or_else(|| ".".to_string());

    let metadata = fs::metadata(&path)?;
    let test_files = if metadata.is_file() {
        if !path.ends_with("_test.tolk") {
            anyhow::bail!("Test file must end with _test.tolk");
        }
        vec![path.clone()]
    } else if metadata.is_dir() {
        find_test_files_recursively(&path, &config.exclude_patterns, &config.include_patterns)?
            .into_iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect()
    } else {
        anyhow::bail!("Path '{}' is neither a file nor a directory", path);
    };

    let cwd = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());

    if config.teamcity {
        TeamcityReporter::on_testing_started();
    }

    if !config.teamcity {
        println!("\n{} {}", " TEST ".bold().on_blue(), cwd.display().dimmed());
    }

    println!();

    let mut file_cache = FileBuildCache::new(None)?;

    let mut total_passed = 0;
    let mut total_failed = 0;
    let mut total_skipped = 0;
    let mut total_todo = 0;
    let mut coverages = vec![];

    for (index, file) in test_files.iter().enumerate() {
        let result = run_tests_for_file(&file, &config, &mut file_cache);
        match result {
            Ok(stats) => {
                total_passed += stats.passed;
                total_failed += stats.failed;
                total_skipped += stats.skipped;
                total_todo += stats.todo;

                if let Some(coverage) = stats.coverage {
                    coverages.push(coverage);
                }

                if index + 1 < test_files.len() {
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

    if !coverages.is_empty() {
        let merged_coverage = merge_coverages(&coverages);
        print_coverage_summary(&merged_coverage);

        if let Some(format_type) = &config.coverage_format {
            println!();
            match format_type.as_str() {
                "lcov" => {
                    let lcov_path = "lcov.info";
                    if let Err(err) = generate_lcov_file(&merged_coverage, lcov_path) {
                        eprintln!(
                            "Warning: Failed to generate LCOV file '{}': {}",
                            lcov_path, err
                        );
                    } else {
                        println!("LCOV file saved in {}", lcov_path);
                    }
                }
                _ => {
                    eprintln!(
                        "Warning: Unknown coverage format '{}'. Supported formats: lcov",
                        format_type
                    );
                }
            }
        }
    }

    if config.teamcity {
        TeamcityReporter::on_testing_finished();
    }

    if total_failed > 0 {
        process::exit(1)
    }
    Ok(())
}

pub fn find_test_files_recursively(
    dir_path: &str,
    exclude_patterns: &[String],
    include_patterns: &[String],
) -> anyhow::Result<Vec<PathBuf>> {
    let mut exclude_builder = GlobSetBuilder::new();
    for p in exclude_patterns {
        exclude_builder.add(Glob::new(p)?);
    }
    for p in ["**/node_modules/**", "**/.git/**", "**/target/**"] {
        exclude_builder.add(Glob::new(p)?);
    }
    let excludes: GlobSet = exclude_builder.build()?;

    let includes: Option<GlobSet> = if !include_patterns.is_empty() {
        let mut include_builder = GlobSetBuilder::new();
        for p in include_patterns {
            include_builder.add(Glob::new(p)?);
        }
        Some(include_builder.build()?)
    } else {
        None
    };

    let root = Path::new(dir_path);

    let it = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            if !entry.file_type().is_dir() {
                return true;
            }
            let p = entry.path();
            let rel = p.strip_prefix(root).unwrap_or(p);
            !excludes.is_match(rel)
        });

    let mut out: Vec<PathBuf> = Vec::new();

    for entry in it {
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                log::warn!("walk dir error: {err}");
                continue;
            }
        };

        if entry.file_type().is_file() {
            let path = entry.path();

            let rel = path.strip_prefix(root).unwrap_or(path);

            if let Some(name) = rel.file_name().and_then(|s| s.to_str()) {
                if name.ends_with("_test.tolk_test.tolk") {
                    // skip temp test file
                    continue;
                }
                if !name.ends_with("_test.tolk") {
                    continue;
                }
            } else {
                continue;
            }

            if excludes.is_match(rel) {
                continue;
            }

            if let Some(includes) = &includes {
                if !includes.is_match(rel) {
                    continue;
                }
            }

            out.push(path.to_path_buf());
        }
    }

    out.sort_unstable();
    Ok(out)
}

#[derive(Debug)]
struct TestStats {
    passed: usize,
    failed: usize,
    skipped: usize,
    todo: usize,
    coverage: Option<Coverage>,
}

fn compile_test_file(
    file_cache: &mut FileBuildCache,
    file: &str,
    need_debug_info: bool,
) -> anyhow::Result<tolkc::CompilerResult> {
    let cache_entry = file_cache.get(file, need_debug_info, 0, "1.2".to_string());
    if let Some(cache_entry) = cache_entry {
        return Ok(tolkc::CompilerResult::Success(
            tolkc::compiler::CompilerResultSuccess {
                fift_code: cache_entry.fift_code,
                code_boc64: cache_entry.code_boc64,
                code_hash_hex: cache_entry.code_hash_hex,
                source_map: cache_entry.source_map,
            },
        ));
    }
    let compilation_result = tolkc::compile(Path::new(&file), need_debug_info);
    match &compilation_result {
        tolkc::CompilerResult::Success(result) => {
            let cache_result = file_cache.put(file, result, need_debug_info, 0, "1.2".to_string());
            match cache_result {
                Ok(_) => {}
                Err(err) => {
                    error!("Cannot cache result of compilation {file}: {err}",)
                }
            }
        }
        tolkc::CompilerResult::Error(_) => {}
    }
    Ok(compilation_result)
}

fn run_tests_for_file(
    file: &str,
    config: &TestConfig,
    file_cache: &mut FileBuildCache,
) -> Result<TestStats, anyhow::Error> {
    let content = match fs::read_to_string(file) {
        Ok(content) => content,
        Err(err) => {
            return Err(anyhow!("Error reading file '{}': {}", file, err));
        }
    };

    let tests = find_all_test(&content);

    let abi = contract_abi(content.as_str(), file);

    let executable_code = inject_locations_into_expect_calls(&content, file);
    let tmp_test_filename = file.to_owned() + "_test.tolk";

    fs::write(&tmp_test_filename, executable_code)?;

    let need_debug_info =
        config.debug || config.backtrace == Some("full".to_string()) || config.coverage;
    let now = Instant::now();
    let compilation_result = compile_test_file(file_cache, &tmp_test_filename, need_debug_info)?;
    debug!("Test file '{file}' compilation time: {:?}", now.elapsed());

    let result = match compilation_result {
        tolkc::CompilerResult::Success(result) => {
            let _ = fs::remove_file(&tmp_test_filename);

            let code_cell = ArcCell::from_boc_b64(&*result.code_boc64)?;

            let mut runner = TestRunner::new(config.clone(), file_cache);
            let stats = run_file_tests(
                &mut runner,
                file,
                tests,
                &code_cell,
                &abi,
                &result.source_map.unwrap_or(Default::default()),
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

fn run_file_tests(
    runner: &mut TestRunner,
    file_path: &str,
    tests: Vec<TestDescriptor>,
    code_cell: &Arc<Cell>,
    abi: &ContractAbi,
    source_map: &SourceMap,
) -> TestStats {
    let filtered_tests = if let Some(pattern) = &runner.config.filter {
        let regex = match Regex::new(pattern) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Invalid regex pattern '{}': {}", pattern, e);
                return TestStats {
                    passed: 0,
                    failed: 0,
                    skipped: 0,
                    todo: 0,
                    coverage: None,
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
        if runner.config.teamcity {
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

    for test in filtered_tests.iter() {
        if runner.config.teamcity {
            TeamcityReporter::on_test_started(&beatify_test_name(&test.name), file_path);
        }

        if test.annotations.contains(&"todo".to_string()) {
            if runner.config.teamcity {
                TeamcityReporter::on_test_ignored(&beatify_test_name(&test.name), 0);
            }
            let description = test.todo_description.as_deref().unwrap_or("TODO");
            println!(
                "  {} {} {}{}{}",
                "□".purple().bold(),
                beatify_test_name(&test.name),
                "[".dimmed(),
                description.dimmed(),
                "]".dimmed()
            );
            todo += 1;
            continue;
        }

        if test.annotations.contains(&"skip".to_string()) {
            if runner.config.teamcity {
                TeamcityReporter::on_test_ignored(&beatify_test_name(&test.name), 0);
            }
            println!(
                "  {} {} {}",
                "○".dimmed(),
                beatify_test_name(&test.name),
                "skipped".dimmed()
            );
            skipped += 1;
            continue;
        }

        let start_time = Instant::now();
        let result = match runner.execute_test(test, &code_cell, &dest_address, abi, source_map) {
            Ok(result) => result,
            Err(err) => {
                println!(
                    "{}: Cannot execute test '{}': {}",
                    "Error".red(),
                    test.name,
                    err
                );
                failed += 1;
                continue;
            }
        };
        let duration = start_time.elapsed();
        let TestResult {
            captured_stdout,
            captured_stderr,
            assert_failure,
            expected_exit_code: dyn_expected_exit_code,
            accounts,
            get_result,
            ..
        } = result;

        let (exit_code, gas_used) = match &get_result {
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

        if exit_code == 0 && assert_failure.is_some() {
            test_passed = false
        }

        if test_passed {
            println!(
                "  {} {} {}{}",
                "✓".green(),
                beatify_test_name(&test.name),
                time_value.green(),
                time_unit.green().dimmed()
            );
            passed += 1;
        } else {
            println!(
                "  {} {} {}{}",
                "✗".red(),
                beatify_test_name(&test.name),
                time_value.red(),
                time_unit.red().dimmed()
            );
            failed += 1;

            let formatter = FormatterContext {
                contract_abi: abi.clone(),
                accounts,
                build_cache: runner.build_cache.clone(),
                emulations: runner.emulations.clone(),
                known_addresses: runner.known_addresses.clone(),
                known_code_cells: runner.known_code_cells.clone(),
                backtrace: runner.config.backtrace.clone(),
            };

            match &get_result {
                GetMethodResult::Success(result) => {
                    let exit_code = result.vm_exit_code as i64;

                    let exit_code_info = retrace::find_exception_info(&result.vm_log, source_map);

                    if gas_limit_exceeded {
                        println!(
                            "    {} Gas limit exceeded: used {}, limit {}",
                            "└─".dimmed(),
                            gas_used.to_string().red(),
                            test.gas_limit.unwrap_or(0).to_string().green()
                        );
                    } else if let Some(ref assert_failure) = assert_failure {
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
                                &assert_failure.right,
                                &assert_failure.right_type,
                                8,
                            );

                            println!("        Actual:   {}", left.red());
                            println!("        Expected: {}", right.green());
                        }

                        if let AssertFailure::TransactionNotFound(assert_failure) = &assert_failure
                        {
                            let params =
                                formatter.format_search_transaction_parameters(assert_failure, abi);

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
                                println!("        {}", line);
                            }
                        }

                        if let AssertFailure::TransactionIsFound(assert_failure) = &assert_failure {
                            let params =
                                formatter.format_search_transaction_parameters(assert_failure, abi);

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
                                                loc.context.event_function.as_ref().map(
                                                    |func_name| {
                                                        let location = format!(
                                                            "{}:{}:{}",
                                                            SourceLocation::normalize_path(
                                                                &loc.loc.file
                                                            ),
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
                                } else if runner.config.backtrace.is_none() {
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

            if runner.config.teamcity {
                TeamcityReporter::on_test_failed(
                    &beatify_test_name(&test.name),
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

        if runner.config.teamcity {
            TeamcityReporter::on_test_finished(
                &beatify_test_name(&test.name),
                file_path,
                duration_ms,
            );
        }

        if runner.config.coverage {
            // For coverage, we need to process test logs as well, so register it here
            if let GetMethodResult::Success(get_result) = get_result {
                runner.emulations.get_results.push(get_result);
                runner.build_cache.memoize(
                    &test.name,
                    &file_path.to_string(),
                    &code_cell
                        .to_boc_b64(false)
                        .expect("Failed to encode code cell to BoC"),
                    &code_cell
                        .cell_hash()
                        .expect("Failed to get code cell hash")
                        .to_hex()
                        .to_ascii_uppercase(),
                    source_map.clone(),
                )
            }
        }
    }

    if !filtered_tests.is_empty() && runner.config.teamcity {
        TeamcityReporter::on_test_suite_finished(file_path);
    }

    let coverage = if runner.config.coverage {
        Some(collect_coverage(&runner.emulations, &runner.build_cache))
    } else {
        None
    };

    TestStats {
        passed,
        failed,
        skipped,
        todo,
        coverage,
    }
}

fn beatify_test_name(name: &String) -> String {
    name.replace("-", " ")
        .replace("_", " ")
        .to_string()
        .trim_start_matches("test ")
        .to_string()
}

fn contract_address(code: &Arc<Cell>) -> TonAddress {
    let state_init = CellBuilder::new()
        .store_bit(false)
        .expect("Failed to store bounce flag")
        .store_bit(false)
        .expect("Failed to store maybe libraries")
        .store_ref_cell_optional(Some(&code))
        .expect("Failed to store code cell")
        .store_ref_cell_optional(Some(&ArcCell::default()))
        .expect("Failed to store data cell")
        .store_bit(false)
        .expect("Failed to store maybe tick/tock")
        .build()
        .expect("Failed to build state init cell");

    let dest_address = TonAddress::new(0, state_init.cell_hash());
    dest_address
}

#[derive(Debug)]
struct TestDescriptor {
    pub id: i32,
    pub name: String,
    pub annotations: Vec<String>,
    pub expected_exit_code: Option<i32>,
    pub gas_limit: Option<u64>,
    pub todo_description: Option<String>,
}

fn find_all_test(content: &String) -> Vec<TestDescriptor> {
    let Ok(tree) = tolk_parser::parser::parse(&content) else {
        return vec![];
    };
    let root_node = tree.root_node();
    let mut cursor = root_node.walk();

    root_node
        .children(&mut cursor)
        .flat_map(|child| {
            if child.kind() == "get_method_declaration" {
                let Some(name_node) = child.child_by_field_name("name") else {
                    return vec![];
                };
                let raw_name = name_node
                    .utf8_text(content.as_bytes())
                    .map(|text| text.to_string());

                let Ok(raw_name) = raw_name else {
                    return vec![];
                };
                let name = raw_name.trim_matches('`').to_string();

                // get fun `test-foo`() or get fun test_foo()
                if name.starts_with("test-") || name.starts_with("test_") {
                    let id = (CRC16.checksum(name.as_bytes()) & 0xff_ff) as i32 | 0x1_00_00;
                    let test_annotations = annotations::find_test_annotations(content, child);

                    return vec![TestDescriptor {
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
