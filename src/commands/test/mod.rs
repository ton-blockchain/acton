use crate::commands::build::build_cmd;
use crate::commands::common::error_fmt;
use crate::commands::test::coverage::{
    Coverage, collect_coverage, generate_lcov_file, generate_text_file, merge_coverages,
    print_coverage_summary,
};
use crate::commands::test::instrumentation::inject_locations_into_expect_calls;
use crate::commands::test::reporting::console::{ConsoleConfig, ConsoleReporter};
use crate::commands::test::reporting::dot::DotReporter;
use crate::commands::test::reporting::junit::{JUnitConfig, JUnitReporter};
use crate::commands::test::reporting::teamcity::TeamCityReporter;
use crate::commands::test::reporting::{
    ReporterManager, TestExecutionContext, TestReport, TestStatus, TestSuiteStats,
    extract_suite_name,
};
use crate::config::ActonConfig;
use crate::context::{
    AssertFailure, AssertsContext, BuildCache, BuildContext, ChainContext, Context, DebugCtx,
    Emulations, Env, IoContext, KnownAddresses,
};
use crate::debugger::dap::DapTransport;
use crate::debugger::debug_context::DebugContext;
use crate::ffi;
use crate::file_build_cache::FileBuildCache;
use abi::{ContractAbi, contract_abi};
use anyhow::anyhow;
use emulator::AnyExecutor;
use emulator::blockchain::Blockchain;
use emulator::emulator::Emulator;
use emulator::executor::ExecutorVerbosity;
use emulator::get_executor::{GetExecutor, GetMethodParams, GetMethodResult};
use emulator::step_get_executor::StepGetExecutor;
use globset::{Glob, GlobSet, GlobSetBuilder};
use log::{debug, error};
use num_traits::ToPrimitive;
use owo_colors::OwoColorize;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::{fs, process};
use tolkc::source_map::SourceMap;
use tonlib_core::TonAddress;
use tonlib_core::cell::{ArcCell, Cell, CellBuilder};
use tonlib_core::tlb_types::tlb::TLB;
use tycho_types::models::ShardAccount;
use walkdir::WalkDir;

mod annotations;
mod coverage;
mod instrumentation;
pub mod mutation;
mod profiling;
mod reporting;
mod trace;

const CRC16: crc::Crc<u16> = crc::Crc::<u16>::new(&crc::CRC_16_XMODEM);

#[derive(Debug, Clone, PartialEq)]
pub enum ReportFormat {
    Console,
    TeamCity,
    JUnit,
    Dot,
}

#[derive(Debug, Clone)]
pub struct TestConfig {
    pub report_formats: Vec<ReportFormat>,
    pub debug: bool,
    pub debug_port: u16,
    pub backtrace: Option<String>,
    pub coverage: bool,
    pub filter: Option<String>,
    pub coverage_format: Option<String>,
    pub coverage_file: Option<String>,
    pub exclude_patterns: Vec<String>,
    pub include_patterns: Vec<String>,
    pub clear_cache: bool,
    pub junit_path: Option<String>,
    pub junit_merge: bool,
    pub snapshot: Option<String>,
    pub baseline_snapshot: Option<String>,
    pub fork_net: Option<String>,
    pub api_key: Option<String>,
    pub save_test_trace: Option<String>,
    pub mutate: bool,
    pub mutate_overrides: Option<String>,
    pub mutate_contract: Option<String>,
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
    acton_config: ActonConfig,
    build_cache: BuildCache,
    file_build_cache: &'a mut FileBuildCache,
    known_addresses: KnownAddresses,
    known_code_cells: HashMap<String, String>,
    emulations: Emulations,
    transport: DapTransport,
    reporter_manager: &'a mut ReporterManager,
    mutation_overrides: BTreeMap<String, ArcCell>,
}

impl<'a> TestRunner<'a> {
    pub fn new(
        acton_config: ActonConfig,
        config: TestConfig,
        cache: &'a mut FileBuildCache,
        reporter_manager: &'a mut ReporterManager,
        mutation_overrides: BTreeMap<String, ArcCell>,
    ) -> TestRunner<'a> {
        let transport = if config.debug {
            crate::debugger::start_dap_server(config.debug_port)
        } else {
            DapTransport::dummy()
        };

        Self {
            config,
            acton_config,
            build_cache: BuildCache::new(),
            file_build_cache: cache,
            known_addresses: KnownAddresses::new(),
            known_code_cells: HashMap::new(),
            emulations: Emulations::new(),
            transport,
            reporter_manager,
            mutation_overrides,
        }
    }

    fn setup_reporters(reporter_manager: &mut ReporterManager, config: &TestConfig) {
        if config.report_formats.is_empty()
            || config.report_formats.contains(&ReportFormat::Console)
        {
            let console_config = ConsoleConfig { show_output: true };
            reporter_manager.add_reporter(Box::new(ConsoleReporter::new(console_config)));
        }

        if config.report_formats.contains(&ReportFormat::TeamCity) {
            reporter_manager.add_reporter(Box::new(TeamCityReporter::new()));
        }

        if config.report_formats.contains(&ReportFormat::JUnit) {
            let mut junit_config = JUnitConfig::default();
            if let Some(ref path) = config.junit_path {
                junit_config.output_dir = path.into();
            }
            junit_config.merge_suites = config.junit_merge;
            reporter_manager.add_reporter(Box::new(JUnitReporter::new(junit_config)));
        }

        if config.report_formats.contains(&ReportFormat::Dot) {
            reporter_manager.add_reporter(Box::new(DotReporter::new()));
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

        ExecutorVerbosity::Full
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
                .map_err(|err| anyhow!("Failed to encode code cell to BoC: {err}"))?
                .to_string(),
            data: ArcCell::default().to_boc_b64(false)?.to_string(), // for tests, we use empty cell as a data
            verbosity,
            libs: Default::default(),
            address: dest_address.to_string(),
            unixtime: 0,
            balance: "10".to_owned(),
            rand_seed: "0000000000000000000000000000000000000000000000000000000000000000"
                .to_owned(),
            gas_limit: "0".to_owned(),
            method_id: test.id,
            debug_enabled: true,
            extra_currencies: HashMap::new(),
            prev_blocks_info: None,
        };

        let mut emulator = Emulator::new(verbosity);
        let mut blockchain =
            Blockchain::new(self.config.fork_net.clone(), self.config.api_key.clone());

        let mut assert_failure = None;
        let mut expected_exit_code = None;

        let mut ctx = Context {
            env: Env {
                config: &self.acton_config,
                abi,
                default_log_level: verbosity,
                wallets: self.acton_config.wallets.as_ref(),
                open_wallets: Default::default(), // in tests, we never use real wallets
                build_override: self.mutation_overrides.clone(),
            },
            io: IoContext {
                stdout_buffer: "".to_owned(),
                stderr_buffer: "".to_owned(),
                capture_output: true,
            },
            asserts: AssertsContext {
                assert_failure: &mut assert_failure,
                expected_exit_code: &mut expected_exit_code,
            },
            chain: ChainContext {
                blockchain: &mut blockchain,
                emulator: &mut emulator,
                emulations: &mut self.emulations,
            },
            build: BuildContext {
                build_cache: &mut self.build_cache,
                file_build_cache: self.file_build_cache,
                known_addresses: &mut self.known_addresses,
                known_code_cells: &mut self.known_code_cells,
                need_debug_info: self.config.debug
                    || self.config.backtrace == Some("full".to_owned())
                    || self.config.coverage,
                backtrace: self.config.backtrace.clone(),
            },
            debug: DebugCtx::Disabled,
            is_broadcasting: false,
            network: "testnet".to_owned(),
        };

        let (result, captured_stdout, captured_stderr, assert_failure, expected_exit_code) =
            if self.config.debug {
                let mut executor = StepGetExecutor::new(Default::default(), params.clone());
                ffi::register(&mut executor, &mut ctx);

                let mut dbg_ctx = DebugContext::new(
                    self.transport.clone(),
                    AnyExecutor::Get(executor.clone()),
                    source_map,
                    test.name.clone(),
                );

                ctx.debug = DebugCtx::new(&mut dbg_ctx);

                executor.prepare(test.id, Default::default());

                ctx.debug.ctx().process_incoming_requests(true)?;

                let get_result = executor.finish(&params.code);

                if let Some(trace_dir) = &self.config.save_test_trace {
                    trace::dump_test_transactions(
                        test,
                        ctx.build.build_cache,
                        &ctx.chain.emulations.results,
                        trace_dir,
                    )?;
                }

                (
                    get_result,
                    ctx.io.stdout_buffer,
                    ctx.io.stderr_buffer,
                    (*ctx.asserts.assert_failure).clone(),
                    ctx.asserts.expected_exit_code.clone(),
                )
            } else {
                let mut executor = GetExecutor::new(params.clone());
                ffi::register(&mut executor, &mut ctx);

                let get_result = executor.run_get_method(Default::default(), params, None);

                if let Some(trace_dir) = &self.config.save_test_trace {
                    trace::dump_test_transactions(
                        test,
                        ctx.build.build_cache,
                        &ctx.chain.emulations.results,
                        trace_dir,
                    )?;
                }

                (
                    get_result,
                    ctx.io.stdout_buffer,
                    ctx.io.stderr_buffer,
                    (*ctx.asserts.assert_failure).clone(),
                    ctx.asserts.expected_exit_code.clone(),
                )
            };

        Ok(TestResult {
            get_result: result,
            captured_stdout,
            captured_stderr,
            assert_failure,
            expected_exit_code: expected_exit_code.and_then(|value| value.to_i32()),
            accounts: blockchain.get_accounts().clone(),
        })
    }
}

pub fn test_cmd(path: Option<String>, config: &TestConfig) -> anyhow::Result<()> {
    // First we need to build all contracts and generate all dependency files with code
    build_cmd(None, config.clear_cache, None, None)?;
    println!("     {} tests", "Running".green().bold());

    // If path is omitted, default to current directory
    let path = path.unwrap_or_else(|| ".".to_string());

    if !fs::exists(&path).unwrap_or(false) {
        anyhow::bail!(error_fmt::file_not_found(&path));
    }

    let metadata = match fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(err) => {
            anyhow::bail!("Cannot access '{path}': {err}")
        }
    };
    let test_files = if metadata.is_file() {
        if !path.ends_with("_test.tolk") {
            anyhow::bail!("Test file must end with {}", "_test.tolk".yellow());
        }
        vec![
            fs::canonicalize(&path)
                .unwrap_or_else(|_| PathBuf::from(&path))
                .to_string_lossy()
                .to_string(),
        ]
    } else if metadata.is_dir() {
        find_test_files_recursively(&path, &config.exclude_patterns, &config.include_patterns)?
            .into_iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect()
    } else {
        anyhow::bail!("Path '{path}' is neither a file nor a directory");
    };

    let acton_config = ActonConfig::load()?;

    let mut global_reporter = ReporterManager::new();
    TestRunner::setup_reporters(&mut global_reporter, config);
    global_reporter.init()?;
    global_reporter.on_testing_started()?;

    let mut file_cache = FileBuildCache::new(None)?;

    let mut total_passed = 0;
    let mut total_failed = 0;
    let mut total_skipped = 0;
    let mut total_todo = 0;
    let mut coverages = vec![];

    for (index, file) in test_files.iter().enumerate() {
        let result = run_tests_for_file(
            file,
            &acton_config,
            config,
            &mut file_cache,
            &mut global_reporter,
        );
        match result {
            Ok(stats) => {
                total_passed += stats.passed;
                total_failed += stats.failed;
                total_skipped += stats.skipped;
                total_todo += stats.todo;

                if let Some(coverage) = stats.coverage {
                    coverages.push(coverage);
                }

                if index + 1 < test_files.len()
                    && config.report_formats.contains(&ReportFormat::Console)
                {
                    println!()
                }
            }
            Err(err) => {
                eprintln!("{err}");
                total_failed += 1;
            }
        }
    }

    let global_stats = TestSuiteStats {
        total: total_passed + total_failed + total_skipped + total_todo,
        passed: total_passed,
        failed: total_failed,
        skipped: total_skipped,
        ignored: 0,
        todo: total_todo,
        duration: Duration::default(),
    };
    global_reporter.on_testing_finished(&global_stats)?;

    if !coverages.is_empty() {
        let merged_coverage = merge_coverages(&coverages);
        print_coverage_summary(&merged_coverage);

        if let Some(format_type) = &config.coverage_format {
            println!();
            match format_type.as_str() {
                "lcov" => {
                    let lcov_path = config.coverage_file.as_deref().unwrap_or("lcov.info");
                    if let Err(err) = generate_lcov_file(&merged_coverage, lcov_path) {
                        eprintln!("Warning: Failed to generate LCOV file '{lcov_path}': {err}");
                    } else {
                        println!("LCOV file saved in {lcov_path}");
                    }
                }
                "text" => {
                    let text_path = config.coverage_file.as_deref().unwrap_or("coverage.txt");
                    if let Err(err) = generate_text_file(&merged_coverage, text_path) {
                        eprintln!(
                            "Warning: Failed to generate text coverage file '{text_path}': {err}"
                        );
                    } else {
                        println!("Text coverage file saved in {text_path}");
                    }
                }
                _ => {
                    eprintln!(
                        "Warning: Unknown coverage format '{format_type}'. Supported formats: lcov, text"
                    );
                }
            }
        }
    }

    global_reporter.finalize()?;

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
    for p in [
        "**/node_modules/**",
        "**/.git/**",
        "**/target/**",
        "**/.acton/**",
    ] {
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

            if let Some(includes) = &includes
                && !includes.is_match(rel)
            {
                continue;
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
    acton_config: &ActonConfig,
    config: &TestConfig,
    file_cache: &mut FileBuildCache,
    reporter_manager: &mut ReporterManager,
) -> anyhow::Result<TestStats> {
    let content = match fs::read_to_string(file) {
        Ok(content) => content,
        Err(err) => {
            return Err(anyhow!("Error reading file '{file}': {err}"));
        }
    };

    let tests = find_all_test(file, &content);

    let abi = contract_abi(content.as_str(), file);

    let executable_code = inject_locations_into_expect_calls(&content, file);
    let tmp_test_filename = file.to_owned() + "_test.tolk";

    fs::write(&tmp_test_filename, executable_code)?;

    let need_debug_info =
        config.debug || config.backtrace == Some("full".to_string()) || config.coverage;
    let now = Instant::now();
    let compilation_result = compile_test_file(file_cache, &tmp_test_filename, need_debug_info)?;
    debug!("Test file '{file}' compilation time: {:?}", now.elapsed());

    match compilation_result {
        tolkc::CompilerResult::Success(result) => {
            let _ = fs::remove_file(&tmp_test_filename);

            let code_cell = ArcCell::from_boc_b64(&result.code_boc64)?;

            let mut mutation_overrides = BTreeMap::new();

            if let Some((first, second)) = config
                .mutate_overrides
                .as_ref()
                .unwrap_or(&"".to_owned())
                .split_once(":")
            {
                let code_cell = ArcCell::from_boc_b64(second)?;
                mutation_overrides.insert(first.to_owned(), code_cell);
            }

            let mut runner = TestRunner::new(
                acton_config.clone(),
                config.clone(),
                file_cache,
                reporter_manager,
                mutation_overrides,
            );
            let stats = run_file_tests(
                &mut runner,
                file,
                tests,
                &code_cell,
                &abi,
                &result.source_map.unwrap_or(Default::default()),
            )?;
            Ok(stats)
        }
        tolkc::CompilerResult::Error(error) => {
            let _ = fs::remove_file(&tmp_test_filename);
            let normalized_filepath = error.message.replace(&tmp_test_filename, file);
            let trimmed_message = normalized_filepath.trim();
            anyhow::bail!(trimmed_message.to_string())
        }
    }
}

fn run_file_tests(
    runner: &mut TestRunner,
    file_path: &str,
    tests: Vec<TestDescriptor>,
    code_cell: &ArcCell,
    abi: &ContractAbi,
    source_map: &SourceMap,
) -> anyhow::Result<TestStats> {
    let filtered_tests = if let Some(pattern) = &runner.config.filter {
        let regex = match Regex::new(pattern) {
            Ok(r) => r,
            Err(e) => {
                anyhow::bail!(color_print::cformat!(
                    "Invalid regex pattern <yellow>{pattern}</>: {e}"
                ));
            }
        };
        tests
            .into_iter()
            .filter(|test| regex.is_match(&test.name))
            .collect::<Vec<_>>()
    } else {
        tests
    };

    runner
        .reporter_manager
        .on_suite_started(file_path, &filtered_tests)?;

    let dest_address = contract_address(code_cell);

    let mut passed = 0;
    let mut failed = 0;
    let mut skipped = 0;
    let mut todo = 0;

    for test in filtered_tests.iter() {
        let suite_name = extract_suite_name(file_path);
        let mut test_report = TestReport {
            name: test.name.clone(),
            suite_name: suite_name.clone(),
            file_path: file_path.to_string(),
            duration: Duration::default(),
            gas_limit: test.gas_limit,
            status: TestStatus::Passed,
            message: None,
            details: None,
            abi: abi.clone(),
            source_map: source_map.clone(),
            backtrace: runner.config.backtrace.clone(),
            execution: None,
        };

        runner.reporter_manager.on_test_started(&test_report)?;

        if test.annotations.contains(&"todo".to_string()) {
            test_report.status = TestStatus::Todo;
            test_report.details = test.todo_description.clone();
            runner.reporter_manager.on_test_finished(&test_report)?;
            todo += 1;
            continue;
        }

        if test.annotations.contains(&"skip".to_string()) {
            test_report.status = TestStatus::Skipped;
            runner.reporter_manager.on_test_finished(&test_report)?;
            skipped += 1;
            continue;
        }

        let start_time = Instant::now();
        let result = match runner.execute_test(test, code_cell, &dest_address, abi, source_map) {
            Ok(result) => result,
            Err(err) => {
                eprintln!(
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

        let mut test_passed: bool = true; // assume that test is passed

        let expected_exit_code = dyn_expected_exit_code
            .or(test.expected_exit_code)
            .unwrap_or(0);

        if exit_code != expected_exit_code {
            test_passed = false;
        }

        if let Some(limit) = test.gas_limit
            && gas_used > limit
        {
            test_passed = false;
        }

        if exit_code == 0 && assert_failure.is_some() {
            test_passed = false
        }

        test_report.duration = duration;
        test_report.execution = Some(TestExecutionContext {
            get_result: get_result.clone(),
            gas_used,
            stdout: captured_stdout.clone(),
            stderr: captured_stderr.clone(),
            assert_failure: assert_failure.clone(),
            accounts: accounts.clone(),
            expected_exit_code,
            build_cache: runner.build_cache.clone(),
            emulations: runner.emulations.clone(),
            known_addresses: runner.known_addresses.clone(),
            known_code_cells: runner.known_code_cells.clone(),
        });

        if test_passed {
            test_report.status = TestStatus::Passed;
            passed += 1;
        } else {
            test_report.status = TestStatus::Failed;

            if let Some(failure) = &assert_failure {
                test_report.message = failure.message();
                test_report.details = failure.location();
            } else if expected_exit_code != 0 {
                test_report.message = Some(format!(
                    "Expected exit_code={expected_exit_code}, got={exit_code}"
                ));
            } else {
                test_report.message = Some(format!("exit_code={exit_code}"));
            }

            failed += 1;
        }

        runner.reporter_manager.on_test_finished(&test_report)?;

        if runner.config.coverage {
            // For coverage, we need to process test logs as well, so register it here
            if let GetMethodResult::Success(get_result) = get_result {
                runner.emulations.get_results.push(get_result);
                // TODO: remove this memoize somehow
                runner.build_cache.memoize(
                    &test.name,
                    file_path,
                    &code_cell.to_boc_b64(false)?,
                    &code_cell.cell_hash()?.to_hex().to_ascii_uppercase(),
                    source_map.clone(),
                )
            }
        }
    }

    let suite_stats = TestSuiteStats {
        total: passed + failed + skipped + todo,
        passed,
        failed,
        skipped,
        ignored: 0,
        todo,
        duration: Duration::default(), // TODO: track suite duration
    };
    runner
        .reporter_manager
        .on_suite_finished(file_path, &suite_stats)?;

    if runner.config.snapshot.is_some() {
        match profiling::collect_profile(runner, abi) {
            Ok(_) => {}
            Err(err) => {
                eprintln!(
                    "{}: Cannot collect profiling result: {}",
                    "Error".red(),
                    err
                );
            }
        };
    }

    let coverage = if runner.config.coverage {
        Some(collect_coverage(&runner.emulations, &runner.build_cache))
    } else {
        None
    };

    Ok(TestStats {
        passed,
        failed,
        skipped,
        todo,
        coverage,
    })
}

fn contract_address(code: &Arc<Cell>) -> TonAddress {
    let state_init = CellBuilder::new()
        .store_bit(false)
        .expect("Failed to store bounce flag")
        .store_bit(false)
        .expect("Failed to store maybe libraries")
        .store_ref_cell_optional(Some(code))
        .expect("Failed to store code cell")
        .store_ref_cell_optional(Some(&ArcCell::default()))
        .expect("Failed to store data cell")
        .store_bit(false)
        .expect("Failed to store maybe tick/tock")
        .build()
        .expect("Failed to build state init cell");

    TonAddress::new(0, state_init.cell_hash())
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Pos {
    pub row: usize,
    pub column: usize,
    pub uri: String,
}

#[derive(Debug)]
pub struct TestDescriptor {
    pub id: i32,
    pub name: String,
    pub annotations: Vec<String>,
    pub expected_exit_code: Option<i32>,
    pub gas_limit: Option<u64>,
    pub todo_description: Option<String>,
    pub pos: Pos,
}

fn find_all_test(file_path: &str, content: &str) -> Vec<TestDescriptor> {
    let Ok(tree) = tolk_parser::parser::parse(content) else {
        return vec![];
    };
    let root_node = tree.root_node();
    let mut cursor = root_node.walk();

    root_node
        .children(&mut cursor)
        .filter_map(|child| {
            if child.kind() == "get_method_declaration" {
                let name_node = child.child_by_field_name("name")?;
                let raw_name = name_node
                    .utf8_text(content.as_bytes())
                    .map(|text| text.to_string())
                    .ok()?;

                let name = raw_name.trim_matches('`').to_string();

                // get fun `test-foo`() or get fun test_foo() or get fun `test foo`()
                if name.starts_with("test-")
                    || name.starts_with("test_")
                    || name.starts_with("test ")
                {
                    let id = CRC16.checksum(name.as_bytes()) as i32 | 0x1_00_00;
                    let test_annotations = annotations::find_test_annotations(content, child);

                    return Some(TestDescriptor {
                        id,
                        name: name.to_string(),
                        annotations: test_annotations.annotations,
                        expected_exit_code: test_annotations.expected_exit_code,
                        gas_limit: test_annotations.gas_limit,
                        todo_description: test_annotations.todo_description,
                        pos: Pos {
                            row: name_node.start_position().row,
                            column: name_node.start_position().column,
                            uri: file_path.to_owned(),
                        },
                    });
                }
            };

            None
        })
        .collect()
}
