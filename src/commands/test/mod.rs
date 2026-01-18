use crate::commands::build::build_cmd;
use crate::commands::common::error_fmt;
use crate::commands::test::coverage::{
    collect_coverage, generate_lcov_file, generate_text_file, print_coverage_summary,
};
use crate::commands::test::instrumentation::inject_locations_into_expect_calls;
use crate::commands::test::reporting::console::{ConsoleConfig, ConsoleReporter};
use crate::commands::test::reporting::dot::DotReporter;
use crate::commands::test::reporting::junit::{JUnitConfig, JUnitReporter};
use crate::commands::test::reporting::teamcity::TeamCityReporter;
use crate::commands::test::reporting::ui::{UiReporter, start_ui_server};
use crate::commands::test::reporting::{
    ReporterManager, TestExecutionContext, TestReport, TestStatus, TestSuiteStats,
    extract_suite_name,
};
use crate::context::{
    AssertFailure, AssertsContext, BuildCache, BuildContext, ChainContext, Context, DebugCtx,
    EmulationsState, Env, IoContext, KnownAddresses,
};
use crate::debugger::any_executor::AnyExecutor;
use crate::debugger::dap::DapTransport;
use crate::debugger::debug_context::DebugContext;
use crate::ffi;
use crate::file_build_cache::FileBuildCache;
use abi::{ContractAbi, contract_abi};
use acton_config::config::{ActonConfig, ContractDependency, DependencyKind};
use acton_config::test::{BacktraceMode, CoverageFormat, ReportFormat, TestConfig};
use anyhow::anyhow;
use emulator::emulator::Emulator;
use emulator::world_state::{
    AccountsState, LocalAccountsState, RemoteAccountState, RemoteSnapshotCache, WorldState,
};
use globset::{Glob, GlobSet, GlobSetBuilder};
use log::{debug, error, warn};
use num_traits::ToPrimitive;
use owo_colors::OwoColorize;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, UNIX_EPOCH};
use std::{fs, process};
use ton_executor::get::step::StepGetExecutor;
use ton_executor::get::{GetExecutor, GetMethodResult, RunGetMethodArgs};
use ton_executor::{DEFAULT_CONFIG, ExecutorVerbosity};
use ton_source_map::SourceMap;
use tonlib_core::TonAddress;
use tonlib_core::cell::{ArcCell, Cell, CellBuilder};
use tonlib_core::tlb_types::tlb::TLB;
use tvmffi::serde::serialize_tuple;
use tvmffi::stack::Tuple;
use tycho_types::boc::Boc;
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
    emulations: EmulationsState,
    transport: DapTransport,
    reporter_manager: &'a mut ReporterManager,
    mutation_overrides: BTreeMap<String, ArcCell>,
    remote_cache: RemoteSnapshotCache,
    /// Contracts used as `library_ref` dependency. We need to register it for correct
    /// work of dependent contracts.
    ref_contracts: BTreeMap<String, tycho_types::cell::Cell>,
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

        let mut ref_contracts = BTreeMap::new();
        if let Some(contracts) = acton_config.contracts() {
            // collect contracts used as a `library_ref` dependency
            let mut contracts_by_ref = vec![];
            for contract in contracts.values() {
                let Some(depends) = &contract.depends else {
                    continue;
                };

                for depend in depends {
                    if let ContractDependency::Detailed { name, kind, .. } = depend
                        && kind == &DependencyKind::LibraryRef
                    {
                        contracts_by_ref.push(name.clone());
                    }
                }
            }

            // extract code of that contracts to later register in `WorldState`
            for contract in contracts_by_ref {
                let Some(contract_info) = contracts.get(&contract) else {
                    continue;
                };

                let Some(cached) =
                    cache.get(&contract_info.src, config.debug, 2, "1.2".to_string())
                else {
                    warn!("No build cache for contract {}", &contract_info.src);
                    continue;
                };

                let Ok(cell) = Boc::decode_base64(&cached.code_boc64) else {
                    warn!(
                        "Cannot deserialize code of {}: {}",
                        &contract_info.src, cached.code_boc64
                    );
                    continue;
                };
                ref_contracts.insert(contract, cell);
            }
        }

        Self {
            config,
            acton_config,
            build_cache: BuildCache::new(),
            file_build_cache: cache,
            known_addresses: KnownAddresses::new(),
            known_code_cells: HashMap::new(),
            emulations: EmulationsState::new(),
            transport,
            reporter_manager,
            mutation_overrides,
            ref_contracts,
            remote_cache: RemoteSnapshotCache::new(),
        }
    }

    fn setup_reporters(
        reporter_manager: &mut ReporterManager,
        config: &TestConfig,
        ui_reporter: Option<UiReporter>,
    ) {
        if config.report_formats.is_empty()
            || config.report_formats.contains(&ReportFormat::Console)
        {
            let console_config = ConsoleConfig { show_output: true };
            reporter_manager.add_reporter(Box::new(ConsoleReporter::new(console_config)));
        }

        if let Some(ui_reporter) = ui_reporter {
            reporter_manager.add_reporter(Box::new(ui_reporter));
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
        if self.config.debug || self.config.backtrace == Some(BacktraceMode::Full) {
            // for these modes we need all logs for work
            return ExecutorVerbosity::FullLocationStackVerbose;
        }

        if self.config.coverage {
            // for coverage, we need at least locations to map to actual source code
            return ExecutorVerbosity::FullLocationStackVerbose;
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

        let now = std::time::SystemTime::now();
        let duration_since_epoch = now.duration_since(UNIX_EPOCH).expect("Time went backwards");

        let params = RunGetMethodArgs {
            code: code_cell
                .to_boc_b64(false)
                .map_err(|err| anyhow!("Failed to encode code cell to BoC: {err}"))?,
            data: ArcCell::default().to_boc_b64(false)?, // for tests, we use empty cell as a data
            verbosity,
            libs: Default::default(),
            address: dest_address.to_string(),
            unixtime: duration_since_epoch.as_secs().try_into()?,
            balance: "10".to_owned(),
            rand_seed: "0000000000000000000000000000000000000000000000000000000000000000"
                .to_owned(),
            gas_limit: "0".to_owned(),
            method_id: test.id,
            debug_enabled: true,
            extra_currencies: HashMap::new(),
            prev_blocks_info: None,
        };

        let mut emulator = Emulator::new(verbosity, None)?;
        let state = match &self.config.fork_net {
            Some(net) => AccountsState::Remote(RemoteAccountState::new(
                net.to_string(),
                self.config.fork_block_number,
                self.config.api_key.clone(),
                self.remote_cache.clone(),
            )),
            None => AccountsState::Local(LocalAccountsState::new()),
        };
        let mut world_state = WorldState::new(state);

        // Register all ref dependency to correct work
        for cell in self.ref_contracts.values() {
            world_state.register_lib(cell.clone());
        }

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
                explorer: None,
                api_key: self.config.api_key.clone(),
                fork_net: self.config.fork_net.as_ref().map(ToString::to_string),
                running_id: test.name.clone(),
            },
            io: IoContext {
                stdout_buffer: String::new(),
                stderr_buffer: String::new(),
                capture_output: true,
            },
            asserts: AssertsContext {
                assert_failure: &mut assert_failure,
                expected_exit_code: &mut expected_exit_code,
            },
            chain: ChainContext {
                world_state: &mut world_state,
                emulator: &mut emulator,
                emulations: &mut self.emulations,
            },
            build: BuildContext {
                build_cache: &mut self.build_cache,
                file_build_cache: self.file_build_cache,
                known_addresses: &mut self.known_addresses,
                known_code_cells: &mut self.known_code_cells,
                need_debug_info: self.config.debug
                    || self.config.backtrace == Some(BacktraceMode::Full)
                    || self.config.coverage,
                backtrace: self.config.backtrace.as_ref().map(ToString::to_string),
            },
            debug: DebugCtx::Disabled,
            is_broadcasting: false,
            network: self.config.fork_net.as_ref().map(ToString::to_string),
        };

        let (result, captured_stdout, captured_stderr, assert_failure, expected_exit_code) =
            if self.config.debug {
                let stack = serialize_tuple(&Tuple::empty())?.to_boc_b64(false)?;
                let mut executor = StepGetExecutor::new(&stack, &params, Some(DEFAULT_CONFIG))?;
                ffi::register(&mut executor, &mut ctx);

                let mut dbg_ctx = DebugContext::new(
                    self.transport.clone(),
                    AnyExecutor::Get(executor.clone()),
                    source_map,
                    test.name.clone(),
                );

                ctx.debug = DebugCtx::new(&mut dbg_ctx);

                executor.prepare(test.id, &stack)?;

                ctx.debug.ctx().process_incoming_requests(true)?;

                let get_result = executor.finish(&params.code)?;

                if let Some(trace_dir) = &self.config.save_test_trace
                    && let Some(emulations) = ctx.chain.emulations.results_of(&test.name)
                {
                    trace::dump_test_transactions(
                        test,
                        ctx.build.build_cache,
                        ctx.build.known_addresses,
                        emulations,
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
                let mut executor = GetExecutor::new(&params)?;
                ffi::register(&mut executor, &mut ctx);

                let stack = serialize_tuple(&Tuple::empty())?.to_boc_b64(false)?;
                let get_result = executor.run_get_method(&stack, &params, Some(DEFAULT_CONFIG))?;

                if let Some(trace_dir) = &self.config.save_test_trace
                    && let Some(emulations) = ctx.chain.emulations.results_of(&test.name)
                {
                    trace::dump_test_transactions(
                        test,
                        ctx.build.build_cache,
                        ctx.build.known_addresses,
                        emulations,
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
            accounts: world_state.get_accounts().clone(),
        })
    }
}

pub fn test_cmd(path: Option<String>, config: &TestConfig) -> anyhow::Result<()> {
    // First we need to build all contracts and generate all dependency files with code
    build_cmd(None, config.clear_cache, None, None, false)?;
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
        if !path.ends_with(".test.tolk") {
            anyhow::bail!("Test file must end with {}", ".test.tolk".yellow());
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

    let ui_reporter = if config.ui {
        Some(UiReporter::new())
    } else {
        None
    };

    let reports_for_ui = ui_reporter.as_ref().map(UiReporter::get_reports_arc);

    let mut global_reporter = ReporterManager::new();
    TestRunner::setup_reporters(&mut global_reporter, config, ui_reporter);
    global_reporter.init()?;
    global_reporter.on_testing_started()?;

    let mut file_cache = FileBuildCache::new(None)?;

    let mut total_passed = 0;
    let mut total_failed = 0;
    let mut total_skipped = 0;
    let mut total_todo = 0;

    let mut runner = TestRunner::new(
        acton_config,
        config.clone(),
        &mut file_cache,
        &mut global_reporter,
        build_overrides_for_mutations(config)?,
    );

    for (index, file) in test_files.iter().enumerate() {
        let result = run_tests_for_file(&mut runner, file);
        match result {
            Ok(stats) => {
                total_passed += stats.passed;
                total_failed += stats.failed;
                total_skipped += stats.skipped;
                total_todo += stats.todo;

                if index + 1 < test_files.len()
                    && config.report_formats.contains(&ReportFormat::Console)
                {
                    println!();
                }
            }
            Err(err) => {
                eprintln!("{err}");
                total_failed += 1;
            }
        }

        if config.fail_fast && total_failed > 0 {
            break;
        }
    }

    let total_tests = total_passed + total_failed + total_skipped + total_todo;

    let global_stats = TestSuiteStats {
        total: total_tests,
        passed: total_passed,
        failed: total_failed,
        skipped: total_skipped,
        todo: total_todo,
        duration: Duration::default(),
    };
    runner.reporter_manager.on_testing_finished(&global_stats)?;

    if config.coverage {
        let coverage = collect_coverage(&runner.emulations, &runner.build_cache);
        print_coverage_summary(&coverage);

        if let Some(format_type) = &config.coverage_format {
            println!();
            match format_type {
                CoverageFormat::Lcov => {
                    let lcov_path = config.coverage_file.as_deref().unwrap_or("lcov.info");
                    if let Err(err) = generate_lcov_file(&coverage, lcov_path) {
                        eprintln!("Warning: Failed to generate LCOV file '{lcov_path}': {err}");
                    } else {
                        println!("LCOV file saved in {lcov_path}");
                    }
                }
                CoverageFormat::Text => {
                    let text_path = config.coverage_file.as_deref().unwrap_or("coverage.txt");
                    if let Err(err) = generate_text_file(&coverage, text_path) {
                        eprintln!(
                            "Warning: Failed to generate text coverage file '{text_path}': {err}"
                        );
                    } else {
                        println!("Text coverage file saved in {text_path}");
                    }
                }
            }
        }
    }

    global_reporter.finalize()?;

    if config.ui
        && let Some(reports) = reports_for_ui
    {
        let reports = reports.lock().expect("cannot lock mutex").clone();
        let trace_dir = config.save_test_trace.clone();
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        rt.block_on(async { start_ui_server(reports, trace_dir, config.ui_port).await })?;
    }

    if let Some(filter) = &config.filter
        && total_tests == 0
    {
        // there is some `--filter` and no test ran, likely something is wrong
        println!(
            "{}",
            color_print::cformat!(
                "\nNo tests matched filter <yellow>{filter}</>, please check the filter spelling/pattern."
            )
        );
        process::exit(1);
    }

    if total_failed > 0 {
        process::exit(1)
    }
    Ok(())
}

fn build_overrides_for_mutations(
    config: &TestConfig,
) -> anyhow::Result<BTreeMap<String, Arc<Cell>>> {
    let mut mutation_overrides = BTreeMap::new();

    if let Some((name, code_b64)) = config
        .mutate_overrides
        .as_ref()
        .unwrap_or(&String::new())
        .split_once(':')
    {
        let code_cell = ArcCell::from_boc_b64(code_b64)?;
        mutation_overrides.insert(name.to_owned(), code_cell);
    }
    Ok(mutation_overrides)
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

    let includes: Option<GlobSet> = if include_patterns.is_empty() {
        None
    } else {
        let mut include_builder = GlobSetBuilder::new();
        for p in include_patterns {
            include_builder.add(Glob::new(p)?);
        }
        Some(include_builder.build()?)
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
                if name.ends_with(".test.tolk.test.tolk") {
                    // skip temp test file
                    continue;
                }
                if !name.ends_with(".test.tolk") {
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
                Ok(()) => {}
                Err(err) => {
                    error!("Cannot cache result of compilation {file}: {err}",);
                }
            }
        }
        tolkc::CompilerResult::Error(_) => {}
    }
    Ok(compilation_result)
}

fn run_tests_for_file(runner: &mut TestRunner, file: &str) -> anyhow::Result<TestStats> {
    let content = match fs::read_to_string(file) {
        Ok(content) => content,
        Err(err) => {
            return Err(anyhow!("Error reading file '{file}': {err}"));
        }
    };

    let tests = find_all_test(file, &content);

    let abi = contract_abi(content.as_str(), file);

    let executable_code = inject_locations_into_expect_calls(&content, file);
    let tmp_test_filename = file.to_owned() + ".test.tolk";

    fs::write(&tmp_test_filename, executable_code)?;

    let config = &runner.config;
    let need_debug_info =
        config.debug || config.backtrace == Some(BacktraceMode::Full) || config.coverage;
    let now = Instant::now();
    let compilation_result =
        compile_test_file(runner.file_build_cache, &tmp_test_filename, need_debug_info)?;
    debug!("Test file '{file}' compilation time: {:?}", now.elapsed());

    match compilation_result {
        tolkc::CompilerResult::Success(result) => {
            let _ = fs::remove_file(&tmp_test_filename);

            let code_cell = ArcCell::from_boc_b64(&result.code_boc64)?;
            let stats = run_file_tests(
                runner,
                file,
                tests,
                &code_cell,
                &abi,
                &result.source_map.unwrap_or_default(),
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
    let abs_file_path = fs::canonicalize(file_path).unwrap_or_else(|_| PathBuf::from(file_path));
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
        .on_suite_started(&abs_file_path, &filtered_tests)?;

    let dest_address = contract_address(code_cell)?;

    let mut passed = 0;
    let mut failed = 0;
    let mut skipped = 0;
    let mut todo = 0;

    for test in &filtered_tests {
        let suite_name = extract_suite_name(&abs_file_path);
        let mut test_report = TestReport {
            name: test.name.clone(),
            suite_name: suite_name.clone(),
            file_path: abs_file_path.to_string_lossy().to_string(),
            row: test.pos.row,
            column: test.pos.column,
            duration: Duration::default(),
            gas_limit: test.gas_limit,
            status: TestStatus::Passed,
            message: None,
            details: None,
            abi: abi.clone(),
            source_map: source_map.clone(),
            backtrace: runner.config.backtrace.as_ref().map(ToString::to_string),
            execution: None,
            trace_path: runner
                .config
                .save_test_trace
                .as_ref()
                .map(|_| format!("{}_trace.json", test.name)),
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
            test_passed = false;
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
            // For coverage, we need to process test logs as well for unit tests coverage,
            // so register it here manually
            if let GetMethodResult::Success(get_result) = get_result {
                runner.emulations.save_get_method(&test.name, get_result);
                // TODO: remove this memoize somehow
                let content = fs::read_to_string(file_path).unwrap_or_default();
                runner.build_cache.memoize(
                    &test.name,
                    file_path,
                    &code_cell.to_boc_b64(false)?,
                    &code_cell.cell_hash()?.to_hex().to_ascii_uppercase(),
                    source_map.clone(),
                    Some(contract_abi(&content, file_path)),
                );
            }
        }

        if !test_passed && runner.config.fail_fast {
            // since test is failed, early break from test loop
            break;
        }
    }

    let suite_stats = TestSuiteStats {
        total: passed + failed + skipped + todo,
        passed,
        failed,
        skipped,
        todo,
        duration: Duration::default(), // TODO: track suite duration
    };
    runner
        .reporter_manager
        .on_suite_finished(&abs_file_path, &suite_stats)?;

    if runner.config.snapshot.is_some() {
        match profiling::collect_profile(runner, abi) {
            Ok(()) => {}
            Err(err) => {
                eprintln!(
                    "{}: Cannot collect profiling result: {}",
                    "Error".red(),
                    err
                );
            }
        }
    }

    Ok(TestStats {
        passed,
        failed,
        skipped,
        todo,
    })
}

fn contract_address(code: &ArcCell) -> anyhow::Result<TonAddress> {
    let state_init = CellBuilder::new()
        .store_bit(false)?
        .store_bit(false)?
        .store_ref_cell_optional(Some(code))?
        .store_ref_cell_optional(Some(&ArcCell::default()))?
        .store_bit(false)?
        .build()?;

    Ok(TonAddress::new(0, state_init.cell_hash()))
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
    let Ok(tree) = tolk_syntax::parse(content) else {
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
                    .map(ToString::to_string)
                    .ok()?;

                let name = raw_name.trim_matches('`').to_string();

                // get fun `test-foo`() or get fun test_foo() or get fun `test foo`()
                if name.starts_with("test-")
                    || name.starts_with("test_")
                    || name.starts_with("test ")
                {
                    let id = i32::from(CRC16.checksum(name.as_bytes())) | 0x1_00_00;
                    let test_annotations = annotations::find_test_annotations(content, child);

                    return Some(TestDescriptor {
                        id,
                        name,
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
            }

            None
        })
        .collect()
}
