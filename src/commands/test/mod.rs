use crate::commands::build::build_cmd;
use crate::commands::common::error_fmt;
use crate::commands::test::coverage::{
    collect_coverage, generate_lcov_file, generate_text_file, print_coverage_summary,
};
use crate::commands::test::instrumentation::prepare_test_file;
use crate::commands::test::reporting::console::{ConsoleConfig, ConsoleReporter};
use crate::commands::test::reporting::dot::DotReporter;
use crate::commands::test::reporting::junit::{JUnitConfig, JUnitReporter};
use crate::commands::test::reporting::teamcity::TeamCityReporter;
use crate::commands::test::reporting::ui::{UiReporter, start_ui_server};
use crate::commands::test::reporting::{
    FailedTransactionContext, MatcherEvent, ReporterManager, TestExecutionContext, TestReport,
    TestStatus, TestSuiteStats, TransactionQueryCandidate, TransactionQueryFailure,
    TransactionQueryMismatch, extract_suite_name,
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
use crate::formatter::FormatterContext;
use acton_config::config::{ActonConfig, ContractDependency, DependencyKind};
use acton_config::test::{BacktraceMode, CoverageFormat, ReportFormat, TestConfig};
use anyhow::anyhow;
use dunce;
use globset::{Glob, GlobSet, GlobSetBuilder};
use log::{debug, error, warn};
use num_traits::ToPrimitive;
use owo_colors::OwoColorize;
use regex::Regex;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, UNIX_EPOCH};
use std::{fs, process};
use tolk_syntax::{AstNode, HasName, SourceFile};
use ton_abi::{ContractAbi, contract_abi, contract_abi_with_file};
use ton_emulator::emulator::Emulator;
use ton_emulator::world_state::{
    AccountsState, LocalAccountsState, RemoteAccountState, RemoteSnapshotCache, WorldState,
};
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
pub mod reporting;
pub mod trace;

const CRC16: crc::Crc<u16> = crc::Crc::<u16>::new(&crc::CRC_16_XMODEM);
const JEST_RESULTS_PATH: &str = ".acton/jest-results.json";
const JEST_MATCHER_EVENTS_PATH: &str = ".acton/jest-matcher-events.jsonl";
const JEST_SETUP_FILE_PATH: &str = ".acton/acton-jest-setup.cjs";
const JEST_SETUP_SCRIPT: &str = include_str!("../../../assets/jest/acton-jest-matchers.cjs");

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JestResults {
    #[serde(default)]
    test_results: Vec<JestSuiteResult>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JestSuiteResult {
    #[serde(default)]
    name: String,
    #[serde(default)]
    assertion_results: Vec<JestAssertionResult>,
    #[serde(default)]
    message: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JestAssertionResult {
    #[serde(default)]
    title: String,
    #[serde(default)]
    full_name: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    failure_messages: Vec<String>,
    duration: Option<f64>,
    location: Option<JestLocation>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JestLocation {
    #[serde(default)]
    line: Option<usize>,
    #[serde(default)]
    column: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawJestMatcherEvent {
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    matcher: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    test_name: String,
    #[serde(default)]
    test_path: String,
    #[serde(default)]
    received: Option<String>,
    #[serde(default)]
    expected: Vec<String>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    location: Option<String>,
    #[serde(default)]
    transaction_query: Option<RawTransactionQueryFailure>,
    #[serde(default)]
    transaction_traces: Vec<RawJestTransactionTraceList>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawJestTransactionTraceList {
    #[serde(default)]
    transactions: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawTransactionQueryFailure {
    #[serde(default)]
    pattern: serde_json::Value,
    #[serde(default)]
    candidates: Vec<RawTransactionQueryCandidate>,
    #[serde(default)]
    negated: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct RawTransactionQueryCandidate {
    #[serde(default)]
    transaction: serde_json::Value,
    #[serde(default)]
    mismatches: Vec<RawTransactionQueryMismatch>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawTransactionQueryMismatch {
    #[serde(default)]
    field: String,
    #[serde(default)]
    expected: String,
    #[serde(default)]
    actual: String,
}

#[derive(Debug)]
pub struct TestResult {
    pub get_result: GetMethodResult,
    pub captured_stdout: String,
    pub captured_stderr: String,
    pub assert_failure: Option<AssertFailure>,
    pub expected_exit_code: Option<i32>,
    pub accounts: FxHashMap<String, ShardAccount>,
}

#[derive(Debug)]
pub struct TestRunner<'a> {
    config: TestConfig,
    acton_config: ActonConfig,
    build_cache: BuildCache,
    file_build_cache: &'a mut FileBuildCache,
    known_addresses: KnownAddresses,
    known_code_cells: FxHashMap<String, String>,
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
                    cache.get(&contract_info.src, config.debug, 2, "1.3".to_string())
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
            known_code_cells: FxHashMap::default(),
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
        abi: Arc<ContractAbi>,
        source_map: Arc<SourceMap>,
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
        let config_b64: Option<&str> = None;

        let mut emulator = Emulator::new(verbosity, config_b64)?;
        let state = match &self.config.fork_net {
            Some(net) => AccountsState::Remote(RemoteAccountState::new(
                net.clone(),
                self.config.fork_block_number,
                self.config.api_key.clone(),
                self.remote_cache.clone(),
            )),
            None => AccountsState::Local(LocalAccountsState::new()),
        };
        let mut world_state = WorldState::new(state, config_b64)?;

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
                fork_net: self.config.fork_net.clone(),
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
                backtrace: self.config.backtrace,
            },
            debug: DebugCtx::Disabled,
            is_broadcasting: false,
            network: self.config.fork_net.clone(),
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
    if config.run_jest {
        return test_cmd_jest(path, config);
    }

    // First we need to build all contracts and generate all dependency files with code
    build_cmd(None, config.clear_cache, None, None, None, false)?;
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
            dunce::canonicalize(&path)
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

    runner.reporter_manager.finalize()?;

    if config.snapshot.is_some() || config.baseline_snapshot.is_some() {
        match profiling::collect_profile(&runner) {
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

    if config.ui
        && let Some(reports) = reports_for_ui
    {
        let reports = reports.lock().expect("cannot lock mutex").clone();
        let trace_dir = config.save_test_trace.clone();
        let project_root = std::env::current_dir().unwrap_or_default();
        let project_root = dunce::canonicalize(project_root)
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default())
            .to_string_lossy()
            .to_string();
        let project_root = if project_root.ends_with(std::path::MAIN_SEPARATOR) {
            project_root
        } else {
            format!("{}{}", project_root, std::path::MAIN_SEPARATOR)
        };
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        rt.block_on(async {
            start_ui_server(reports, trace_dir, project_root, config.ui_port).await
        })?;
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

fn test_cmd_jest(path: Option<String>, config: &TestConfig) -> anyhow::Result<()> {
    println!("     {} tests", "Running".green().bold());

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
    if !metadata.is_file() && !metadata.is_dir() {
        anyhow::bail!("Path '{path}' is neither a file nor a directory");
    }

    let ui_reporter = if config.ui {
        Some(UiReporter::new())
    } else {
        None
    };
    let reports_for_ui = ui_reporter.as_ref().map(UiReporter::get_reports_arc);

    let mut reporter_manager = ReporterManager::new();
    TestRunner::setup_reporters(&mut reporter_manager, config, ui_reporter);
    reporter_manager.init()?;
    reporter_manager.on_testing_started()?;

    let (jest_results, mut matcher_events_by_test, mut transaction_traces_by_test) =
        run_jest_results(
            &path,
            config.filter.as_deref(),
            config.save_test_trace.is_some(),
        )?;

    let mut total_passed = 0;
    let mut total_failed = 0;
    let mut total_skipped = 0;
    let mut total_todo = 0;
    let mut should_stop = false;

    for suite in jest_results.test_results {
        let suite_path = if suite.name.is_empty() {
            dunce::canonicalize(&path).unwrap_or_else(|_| PathBuf::from(&path))
        } else {
            dunce::canonicalize(&suite.name).unwrap_or_else(|_| PathBuf::from(&suite.name))
        };

        let mut assertions = suite.assertion_results;
        if assertions.is_empty() && !suite.message.trim().is_empty() {
            assertions.push(synthetic_suite_failure_assertion(&suite.message));
        }

        let suite_tests = make_jest_test_descriptors(&suite_path, &assertions);
        reporter_manager.on_suite_started(&suite_path, &suite_tests)?;

        let mut suite_stats = TestSuiteStats::default();
        let suite_path_key = suite_path.to_string_lossy().to_string();

        for assertion in assertions {
            let status = map_jest_status(&assertion.status);
            let test_name = jest_test_name(&assertion).to_owned();
            let matcher_events =
                take_jest_matcher_events(&mut matcher_events_by_test, &suite_path_key, &test_name);
            let transaction_traces = take_jest_transaction_traces(
                &mut transaction_traces_by_test,
                &suite_path_key,
                &test_name,
            );
            let duration = map_jest_duration(assertion.duration);
            let (message, details) = jest_messages(&assertion, &status, matcher_events.as_deref());

            let test_report = TestReport {
                name: test_name.into(),
                suite_name: extract_suite_name(&suite_path),
                file_path: suite_path.clone(),
                row: assertion
                    .location
                    .as_ref()
                    .and_then(|loc| loc.line)
                    .unwrap_or(0),
                column: assertion
                    .location
                    .as_ref()
                    .and_then(|loc| loc.column)
                    .unwrap_or(0),
                duration,
                gas_limit: None,
                status,
                message,
                detailed_message: None,
                failed_transactions: None,
                failed_transaction_context: None,
                details,
                matcher_events,
                location: None,
                abi: Arc::new(ContractAbi::default()),
                source_map: Arc::new(SourceMap::default()),
                backtrace: config.backtrace,
                execution: None,
                trace_path: None,
            };
            let mut test_report = test_report;
            enrich_jest_transaction_matcher_context(&mut test_report);
            maybe_dump_jest_test_trace(
                &mut test_report,
                config.save_test_trace.as_deref(),
                transaction_traces,
            );

            reporter_manager.on_test_started(&test_report)?;
            reporter_manager.on_test_finished(&test_report)?;
            suite_stats.add_test(&test_report.status, test_report.duration);

            match test_report.status {
                TestStatus::Passed => total_passed += 1,
                TestStatus::Failed => {
                    total_failed += 1;
                    if config.fail_fast {
                        should_stop = true;
                    }
                }
                TestStatus::Skipped => total_skipped += 1,
                TestStatus::Todo => total_todo += 1,
            }

            if should_stop {
                break;
            }
        }

        reporter_manager.on_suite_finished(&suite_path, &suite_stats)?;
        if should_stop {
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
    reporter_manager.on_testing_finished(&global_stats)?;
    reporter_manager.finalize()?;

    if config.ui
        && let Some(reports) = reports_for_ui
    {
        let reports = reports.lock().expect("cannot lock mutex").clone();
        let trace_dir = config.save_test_trace.clone();
        let project_root = std::env::current_dir().unwrap_or_default();
        let project_root = dunce::canonicalize(project_root)
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default())
            .to_string_lossy()
            .to_string();
        let project_root = if project_root.ends_with(std::path::MAIN_SEPARATOR) {
            project_root
        } else {
            format!("{}{}", project_root, std::path::MAIN_SEPARATOR)
        };
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        rt.block_on(async {
            start_ui_server(reports, trace_dir, project_root, config.ui_port).await
        })?;
    }

    if let Some(filter) = &config.filter
        && total_tests == 0
    {
        println!(
            "{}",
            color_print::cformat!(
                "\nNo tests matched filter <yellow>{filter}</>, please check the filter spelling/pattern."
            )
        );
        process::exit(1);
    }

    if total_failed > 0 {
        process::exit(1);
    }

    Ok(())
}

#[allow(clippy::type_complexity)]
fn run_jest_results(
    path: &str,
    filter: Option<&str>,
    capture_traces: bool,
) -> anyhow::Result<(
    JestResults,
    HashMap<(String, String), Vec<MatcherEvent>>,
    HashMap<(String, String), Vec<trace::TransactionList>>,
)> {
    let output_path = PathBuf::from(JEST_RESULTS_PATH);
    let matcher_events_path = PathBuf::from(JEST_MATCHER_EVENTS_PATH);
    let setup_path = PathBuf::from(JEST_SETUP_FILE_PATH);
    prepare_jest_bridge_files(&output_path, &matcher_events_path, &setup_path)?;

    let mut failures = Vec::new();

    let mut npm_cmd = process::Command::new("npm");
    npm_cmd.arg("test").arg("--");
    npm_cmd.env("ACTON_JEST_MATCHERS_FILE", &matcher_events_path);
    npm_cmd.env(
        "ACTON_JEST_CAPTURE_TRANSACTIONS",
        if capture_traces { "1" } else { "0" },
    );
    append_jest_args(&mut npm_cmd, path, filter, &output_path, &setup_path);
    match npm_cmd.output() {
        Ok(output) => {
            if output_path.exists() {
                let results = read_jest_results(&output_path)?;
                let (matcher_events, transaction_traces) =
                    match read_jest_matcher_events(&matcher_events_path) {
                        Ok(events) => events,
                        Err(err) => {
                            warn!("Cannot parse Jest matcher events: {err}");
                            (HashMap::new(), HashMap::new())
                        }
                    };
                return Ok((results, matcher_events, transaction_traces));
            }
            failures.push(format_command_failure("npm test", &output, &output_path));
        }
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                failures.push("`npm` was not found in PATH".to_owned());
            } else {
                failures.push(format!("Failed to run `npm test`: {err}"));
            }
        }
    }

    let mut npx_cmd = process::Command::new("npx");
    npx_cmd.arg("jest");
    npx_cmd.env("ACTON_JEST_MATCHERS_FILE", &matcher_events_path);
    npx_cmd.env(
        "ACTON_JEST_CAPTURE_TRANSACTIONS",
        if capture_traces { "1" } else { "0" },
    );
    append_jest_args(&mut npx_cmd, path, filter, &output_path, &setup_path);
    match npx_cmd.output() {
        Ok(output) => {
            if output_path.exists() {
                let results = read_jest_results(&output_path)?;
                let (matcher_events, transaction_traces) =
                    match read_jest_matcher_events(&matcher_events_path) {
                        Ok(events) => events,
                        Err(err) => {
                            warn!("Cannot parse Jest matcher events: {err}");
                            (HashMap::new(), HashMap::new())
                        }
                    };
                return Ok((results, matcher_events, transaction_traces));
            }
            failures.push(format_command_failure("npx jest", &output, &output_path));
        }
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                failures.push("`npx` was not found in PATH".to_owned());
            } else {
                failures.push(format!("Failed to run `npx jest`: {err}"));
            }
        }
    }

    anyhow::bail!(
        "Jest bridge failed to produce '{}'.\n{}",
        output_path.display(),
        failures.join("\n\n")
    );
}

fn prepare_jest_bridge_files(
    output_path: &Path,
    matcher_events_path: &Path,
    setup_path: &Path,
) -> anyhow::Result<()> {
    for path in [output_path, matcher_events_path, setup_path] {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
    }

    for path in [output_path, matcher_events_path] {
        if path.exists() {
            let _ = fs::remove_file(path);
        }
    }

    fs::write(setup_path, JEST_SETUP_SCRIPT)?;
    Ok(())
}

fn append_jest_args(
    command: &mut process::Command,
    path: &str,
    filter: Option<&str>,
    output_path: &Path,
    setup_path: &Path,
) {
    let setup_module_path = absolute_path_string(setup_path);

    command
        .arg("--json")
        .arg("--outputFile")
        .arg(output_path)
        .arg("--testLocationInResults")
        .arg("--setupFilesAfterEnv")
        .arg(setup_module_path)
        .arg("--runInBand");

    if let Some(filter) = filter
        && !filter.trim().is_empty()
    {
        command.arg("--testNamePattern").arg(filter);
    }

    if path != "." {
        command.arg(path);
    }
}

fn absolute_path_string(path: &Path) -> String {
    dunce::canonicalize(path)
        .unwrap_or_else(|_| {
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            cwd.join(path)
        })
        .to_string_lossy()
        .to_string()
}

fn read_jest_matcher_events(
    events_path: &Path,
) -> anyhow::Result<(
    HashMap<(String, String), Vec<MatcherEvent>>,
    HashMap<(String, String), Vec<trace::TransactionList>>,
)> {
    if !events_path.exists() {
        return Ok((HashMap::new(), HashMap::new()));
    }

    let raw = fs::read_to_string(events_path)?;
    let mut out: HashMap<(String, String), Vec<MatcherEvent>> = HashMap::new();
    let mut traces: HashMap<(String, String), Vec<trace::TransactionList>> = HashMap::new();

    for (index, line) in raw.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let parsed: RawJestMatcherEvent = serde_json::from_str(line)
            .map_err(|err| anyhow!("Cannot parse matcher event line {}: {err}", index + 1))?;

        if parsed.test_name.trim().is_empty() {
            continue;
        }

        let test_path = normalize_jest_test_path(&parsed.test_path);
        let key = (test_path, parsed.test_name);

        if parsed.kind.as_deref() == Some("transaction_dump") {
            let collected = parsed
                .transaction_traces
                .iter()
                .filter_map(raw_trace_list_to_transactions)
                .collect::<Vec<_>>();
            if !collected.is_empty() {
                traces.entry(key).or_default().extend(collected);
            }
            continue;
        }

        out.entry(key).or_default().push(MatcherEvent {
            matcher: parsed.matcher,
            status: parsed.status,
            received: parsed.received,
            expected: parsed.expected,
            message: parsed.message,
            location: parsed.location,
            transaction_query: parsed
                .transaction_query
                .map(|query| TransactionQueryFailure {
                    pattern: query.pattern,
                    candidates: query
                        .candidates
                        .into_iter()
                        .map(|candidate| TransactionQueryCandidate {
                            transaction: candidate.transaction,
                            mismatches: candidate
                                .mismatches
                                .into_iter()
                                .map(|mismatch| TransactionQueryMismatch {
                                    field: mismatch.field,
                                    expected: mismatch.expected,
                                    actual: mismatch.actual,
                                })
                                .collect(),
                        })
                        .collect(),
                    negated: query.negated,
                }),
        });
    }

    Ok((out, traces))
}

fn normalize_jest_test_path(path: &str) -> String {
    if path.trim().is_empty() {
        return String::new();
    }
    dunce::canonicalize(path)
        .unwrap_or_else(|_| PathBuf::from(path))
        .to_string_lossy()
        .to_string()
}

fn take_jest_matcher_events(
    matcher_events_by_test: &mut HashMap<(String, String), Vec<MatcherEvent>>,
    suite_path: &str,
    test_name: &str,
) -> Option<Vec<MatcherEvent>> {
    if let Some(events) =
        matcher_events_by_test.remove(&(suite_path.to_owned(), test_name.to_owned()))
    {
        return Some(events);
    }

    matcher_events_by_test.remove(&(String::new(), test_name.to_owned()))
}

fn take_jest_transaction_traces(
    traces_by_test: &mut HashMap<(String, String), Vec<trace::TransactionList>>,
    suite_path: &str,
    test_name: &str,
) -> Vec<trace::TransactionList> {
    if let Some(traces) = traces_by_test.remove(&(suite_path.to_owned(), test_name.to_owned())) {
        return traces;
    }

    traces_by_test
        .remove(&(String::new(), test_name.to_owned()))
        .unwrap_or_default()
}

fn raw_trace_list_to_transactions(
    list: &RawJestTransactionTraceList,
) -> Option<trace::TransactionList> {
    let transactions = list
        .transactions
        .iter()
        .filter_map(transaction_json_to_trace_transaction)
        .collect::<Vec<_>>();
    if transactions.is_empty() {
        return None;
    }
    Some(trace::TransactionList { transactions })
}

fn read_jest_results(output_path: &Path) -> anyhow::Result<JestResults> {
    let raw = fs::read_to_string(output_path)?;
    serde_json::from_str::<JestResults>(&raw).map_err(|err| {
        anyhow!(
            "Cannot parse Jest JSON report '{}': {err}",
            output_path.display()
        )
    })
}

fn format_command_failure(label: &str, output: &process::Output, output_path: &Path) -> String {
    let mut message = format!(
        "`{label}` exited with status {} but did not produce '{}'",
        output.status,
        output_path.display()
    );

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if !stdout.is_empty() {
        message.push_str("\nstdout:\n");
        message.push_str(&truncate_output(&stdout));
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if !stderr.is_empty() {
        message.push_str("\nstderr:\n");
        message.push_str(&truncate_output(&stderr));
    }

    message
}

fn truncate_output(text: &str) -> String {
    const MAX_CHARS: usize = 1200;
    if text.chars().count() <= MAX_CHARS {
        return text.to_owned();
    }

    let head: String = text.chars().take(MAX_CHARS).collect();
    format!("{head}\n... (truncated)")
}

fn make_jest_test_descriptors(
    suite_path: &Path,
    assertions: &[JestAssertionResult],
) -> Vec<TestDescriptor> {
    let uri = suite_path.to_string_lossy().to_string();
    assertions
        .iter()
        .map(|assertion| TestDescriptor {
            id: 0,
            name: jest_test_name(assertion).into(),
            annotations: Vec::new(),
            expected_exit_code: None,
            gas_limit: None,
            todo_description: None,
            pos: Pos {
                row: assertion
                    .location
                    .as_ref()
                    .and_then(|loc| loc.line)
                    .unwrap_or(0),
                column: assertion
                    .location
                    .as_ref()
                    .and_then(|loc| loc.column)
                    .unwrap_or(0),
                uri: uri.clone(),
            },
        })
        .collect()
}

fn map_jest_status(status: &str) -> TestStatus {
    match status {
        "passed" => TestStatus::Passed,
        "failed" => TestStatus::Failed,
        "todo" => TestStatus::Todo,
        "pending" | "skipped" | "disabled" => TestStatus::Skipped,
        _ => TestStatus::Skipped,
    }
}

fn map_jest_duration(duration_ms: Option<f64>) -> Duration {
    let Some(duration_ms) = duration_ms else {
        return Duration::default();
    };
    if !duration_ms.is_finite() || duration_ms <= 0.0 {
        return Duration::default();
    }

    Duration::from_secs_f64(duration_ms / 1000.0)
}

fn jest_test_name(assertion: &JestAssertionResult) -> &str {
    if assertion.full_name.trim().is_empty() {
        assertion.title.as_str()
    } else {
        assertion.full_name.as_str()
    }
}

fn jest_messages(
    assertion: &JestAssertionResult,
    status: &TestStatus,
    matcher_events: Option<&[MatcherEvent]>,
) -> (Option<String>, Option<String>) {
    if assertion.failure_messages.is_empty() {
        return match status {
            TestStatus::Failed => {
                let matcher_message = matcher_events.and_then(|events| {
                    events
                        .iter()
                        .find(|event| event.status.eq_ignore_ascii_case("failed"))
                        .and_then(|event| event.message.clone())
                });
                (
                    matcher_message.or_else(|| Some("Jest test failed".to_owned())),
                    None,
                )
            }
            TestStatus::Todo => (None, Some("TODO".to_owned())),
            _ => (None, None),
        };
    }

    let details = assertion.failure_messages.join("\n\n");
    match status {
        TestStatus::Failed => (Some(details), None),
        TestStatus::Todo | TestStatus::Skipped => (None, Some(details)),
        TestStatus::Passed => (None, None),
    }
}

fn enrich_jest_transaction_matcher_context(test_report: &mut TestReport) {
    if test_report.status != TestStatus::Failed {
        return;
    }

    let Some(events) = &test_report.matcher_events else {
        return;
    };
    let Some(event) = events
        .iter()
        .find(|event| event.transaction_query.is_some())
    else {
        return;
    };
    let Some(query) = &event.transaction_query else {
        return;
    };

    let context = transaction_query_to_failed_context(query);
    if context.from_address.is_some() || context.to_address.is_some() || !context.params.is_empty()
    {
        test_report.failed_transaction_context = Some(context);
    }

    let failed_transactions = transaction_query_to_failed_transactions(query);
    if !failed_transactions.is_empty() {
        test_report.failed_transactions = Some(failed_transactions);
    }

    if test_report.details.is_none() {
        test_report.details = event.location.clone();
    }

    if test_report.detailed_message.is_none() {
        test_report.detailed_message = event.message.clone();
    }

    test_report.message = Some(transaction_query_summary(
        query,
        test_report.failed_transaction_context.as_ref(),
    ));
}

fn transaction_query_summary(
    query: &TransactionQueryFailure,
    context: Option<&FailedTransactionContext>,
) -> String {
    if query.negated {
        if let Some(context) = context {
            let from = context.from_address.as_deref().unwrap_or("<any>");
            let to = context.to_address.as_deref().unwrap_or("<any>");
            return format!("Unexpected transaction from {from} to {to}");
        }

        return format!(
            "Unexpected transaction matching pattern {} (checked {} candidate(s))",
            compact_json(&query.pattern, 220),
            query.candidates.len()
        );
    }

    if let Some(context) = context {
        let from = context.from_address.as_deref().unwrap_or("<any>");
        let to = context.to_address.as_deref().unwrap_or("<any>");
        return format!("Cannot find transaction from {from} to {to}");
    }

    format!(
        "Cannot find transaction matching pattern {} (checked {} candidate(s))",
        compact_json(&query.pattern, 220),
        query.candidates.len()
    )
}

fn transaction_query_to_failed_context(
    query: &TransactionQueryFailure,
) -> FailedTransactionContext {
    let mut from_address = None;
    let mut to_address = None;
    let mut params = Vec::new();

    if let Some(pattern) = query.pattern.as_object() {
        for (key, value) in pattern {
            let formatted = format_pattern_value(value);
            match key.as_str() {
                "from" => from_address = Some(formatted),
                "to" | "on" => {
                    if to_address.is_none() {
                        to_address = Some(formatted);
                    }
                }
                _ => params.push((key.clone(), formatted)),
            }
        }
    }

    FailedTransactionContext {
        from_address,
        to_address,
        params,
    }
}

fn transaction_query_to_failed_transactions(
    query: &TransactionQueryFailure,
) -> Vec<trace::TransactionInfo> {
    query
        .candidates
        .iter()
        .filter_map(transaction_query_candidate_to_failed_transaction)
        .collect()
}

fn maybe_dump_jest_test_trace(
    test_report: &mut TestReport,
    trace_dir: Option<&str>,
    precomputed_traces: Vec<trace::TransactionList>,
) {
    let Some(trace_dir) = trace_dir else {
        return;
    };

    let traces = if precomputed_traces.is_empty() {
        collect_jest_trace_lists(test_report)
    } else {
        precomputed_traces
    };
    if traces.is_empty() {
        return;
    }

    let test_descriptor = TestDescriptor {
        id: 0,
        name: test_report.name.clone(),
        annotations: vec![],
        expected_exit_code: None,
        gas_limit: None,
        todo_description: None,
        pos: Pos {
            row: test_report.row,
            column: test_report.column,
            uri: test_report.file_path.to_string_lossy().to_string(),
        },
    };

    if let Err(err) = trace::dump_precomputed_test_trace(&test_descriptor, traces, trace_dir) {
        warn!("Cannot dump Jest trace for '{}': {err}", test_report.name);
        return;
    }

    test_report.trace_path = Some(format!("{}_trace.json", test_report.name));
}

fn collect_jest_trace_lists(test_report: &TestReport) -> Vec<trace::TransactionList> {
    let mut traces = Vec::new();

    if let Some(events) = &test_report.matcher_events {
        for event in events
            .iter()
            .filter(|event| event.status.eq_ignore_ascii_case("failed"))
        {
            let Some(query) = &event.transaction_query else {
                continue;
            };

            let transactions = transaction_query_to_failed_transactions(query);
            if transactions.is_empty() {
                continue;
            }

            traces.push(trace::TransactionList { transactions });
        }
    }

    if traces.is_empty()
        && let Some(transactions) = &test_report.failed_transactions
        && !transactions.is_empty()
    {
        traces.push(trace::TransactionList {
            transactions: transactions.clone(),
        });
    }

    traces
}

fn transaction_query_candidate_to_failed_transaction(
    candidate: &TransactionQueryCandidate,
) -> Option<trace::TransactionInfo> {
    transaction_json_to_trace_transaction(&candidate.transaction)
}

fn transaction_json_to_trace_transaction(tx: &serde_json::Value) -> Option<trace::TransactionInfo> {
    let tx = tx.as_object()?;

    let lt = tx.get("lt").and_then(json_value_as_string)?;
    let raw_transaction = tx.get("raw_transaction").and_then(json_value_as_string)?;
    if raw_transaction.is_empty() {
        return None;
    }

    let parent_transaction = tx.get("parent_transaction").and_then(json_value_as_string);
    let child_transactions = tx
        .get("child_transactions")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(json_value_as_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let shard_account_before = tx
        .get("shard_account_before")
        .and_then(json_value_as_string)
        .unwrap_or_default();
    let shard_account = tx
        .get("shard_account")
        .and_then(json_value_as_string)
        .unwrap_or_default();
    let vm_log_diff = tx
        .get("vm_log_diff")
        .and_then(json_value_as_string)
        .unwrap_or_default();
    let executor_logs = tx
        .get("executor_logs")
        .and_then(json_value_as_string)
        .unwrap_or_default();
    let actions = tx
        .get("actions")
        .and_then(json_value_as_string)
        .map(Arc::from);
    let dest_contract_info = tx.get("dest_contract_info").and_then(json_value_as_string);

    Some(trace::TransactionInfo {
        lt,
        raw_transaction: raw_transaction.into(),
        parent_transaction,
        child_transactions,
        shard_account_before,
        shard_account,
        vm_log_diff,
        executor_logs: executor_logs.into(),
        actions,
        dest_contract_info,
    })
}

fn json_value_as_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn format_pattern_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Number(value) => value.to_string(),
        serde_json::Value::Bool(value) => value.to_string(),
        serde_json::Value::Null => "null".to_owned(),
        _ => compact_json(value, 180),
    }
}

fn compact_json(value: &serde_json::Value, max_chars: usize) -> String {
    let raw = serde_json::to_string(value).unwrap_or_else(|_| value.to_string());
    if raw.chars().count() <= max_chars {
        return raw;
    }
    let head = raw.chars().take(max_chars).collect::<String>();
    format!("{head}...")
}

fn synthetic_suite_failure_assertion(message: &str) -> JestAssertionResult {
    JestAssertionResult {
        title: "suite setup".to_owned(),
        full_name: "suite setup".to_owned(),
        status: "failed".to_owned(),
        failure_messages: vec![message.to_owned()],
        duration: Some(0.0),
        location: None,
    }
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
    acton_config: &ActonConfig,
) -> anyhow::Result<tolkc::CompilerResult> {
    let cache_entry = file_cache.get(file, need_debug_info, 0, "1.3".to_string());
    if let Some(cache_entry) = cache_entry {
        return Ok(tolkc::CompilerResult::Success(
            tolkc::compiler::CompilerResultSuccess {
                fift_code: cache_entry.fift_code,
                code_boc64: cache_entry.code_boc64,
                code_hash_hex: cache_entry.code_hash_hex,
                source_map: cache_entry.source_map,
                abi: cache_entry.abi,
            },
        ));
    }

    let compiler = tolkc::Compiler::new(0).with_mappings(&acton_config.mappings);
    let compilation_result = compiler.compile(Path::new(file), need_debug_info);
    match &compilation_result {
        tolkc::CompilerResult::Success(result) => {
            let cache_result = file_cache.put(file, result, need_debug_info, 0, "1.3".to_string());
            match cache_result {
                Ok(()) => {}
                Err(err) => {
                    error!("Cannot cache result of compilation {file}: {err}",);
                }
            }
        }
        tolkc::CompilerResult::Error(_) => {
            // handled in caller
        }
    }
    Ok(compilation_result)
}

fn run_tests_for_file(runner: &mut TestRunner, filepath: &str) -> anyhow::Result<TestStats> {
    let content = match fs::read_to_string(filepath) {
        Ok(content) => content,
        Err(err) => {
            return Err(anyhow!("Error reading file '{filepath}': {err}"));
        }
    };

    let file = tolk_syntax::parse(&content);
    let tests = find_all_test(filepath, &file, &content);

    let abi = contract_abi_with_file(
        content.as_str(),
        filepath,
        &file,
        &runner.acton_config.mappings,
    );

    let executable_code = prepare_test_file(&file, &content);
    let tmp_test_filename = filepath.to_owned() + ".test.tolk";

    fs::write(&tmp_test_filename, executable_code)?;

    let config = &runner.config;
    let need_debug_info =
        config.debug || config.backtrace == Some(BacktraceMode::Full) || config.coverage;

    let now = Instant::now();
    let compilation_result = compile_test_file(
        runner.file_build_cache,
        &tmp_test_filename,
        need_debug_info,
        &runner.acton_config,
    )?;
    let _ = fs::remove_file(&tmp_test_filename);
    debug!(
        "Test file '{filepath}' compilation time: {:?}",
        now.elapsed()
    );

    let result = match compilation_result {
        tolkc::CompilerResult::Success(result) => result,
        tolkc::CompilerResult::Error(error) => {
            let normalized_filepath = error.message.replace(&tmp_test_filename, filepath);
            let trimmed_message = normalized_filepath.trim();
            anyhow::bail!(trimmed_message.to_string())
        }
    };

    let code_cell = ArcCell::from_boc_b64(&result.code_boc64)?;
    let source_map = result.source_map.unwrap_or_default();
    let stats = run_file_tests(
        runner,
        filepath,
        tests,
        &code_cell,
        Arc::new(abi),
        Arc::new(source_map),
    )?;
    Ok(stats)
}

fn run_file_tests(
    runner: &mut TestRunner,
    file_path: &str,
    tests: Vec<TestDescriptor>,
    code: &ArcCell,
    abi: Arc<ContractAbi>,
    source_map: Arc<SourceMap>,
) -> anyhow::Result<TestStats> {
    let file_path = dunce::canonicalize(file_path).unwrap_or_else(|_| PathBuf::from(file_path));
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
        .on_suite_started(&file_path, &filtered_tests)?;

    let dest_address = contract_address(code)?;

    let mut passed = 0;
    let mut failed = 0;
    let mut skipped = 0;
    let mut todo = 0;

    for test in &filtered_tests {
        let suite_name = extract_suite_name(&file_path);
        let mut test_report = TestReport {
            name: test.name.clone(),
            suite_name,
            file_path: file_path.clone(),
            row: test.pos.row,
            column: test.pos.column,
            duration: Duration::default(),
            gas_limit: test.gas_limit,
            status: TestStatus::Passed,
            message: None,
            detailed_message: None,
            failed_transactions: None,
            failed_transaction_context: None,
            details: None,
            matcher_events: None,
            location: None,
            abi: abi.clone(),
            source_map: source_map.clone(),
            backtrace: runner.config.backtrace,
            execution: None,
            trace_path: runner
                .config
                .save_test_trace
                .as_ref()
                .map(|_| format!("{}_trace.json", test.name)),
        };

        runner.reporter_manager.on_test_started(&test_report)?;

        if test.annotations.contains(&TestAnnotation::Todo) {
            test_report.status = TestStatus::Todo;
            test_report.details = test.todo_description.clone();
            runner.reporter_manager.on_test_finished(&test_report)?;
            todo += 1;
            continue;
        }

        if test.annotations.contains(&TestAnnotation::Skip) {
            test_report.status = TestStatus::Skipped;
            runner.reporter_manager.on_test_finished(&test_report)?;
            skipped += 1;
            continue;
        }

        let start_time = Instant::now();
        let result =
            runner.execute_test(test, code, &dest_address, abi.clone(), source_map.clone());
        let result = match result {
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

            let formatter = FormatterContext {
                contract_abi: abi.clone(),
                accounts: Cow::Borrowed(&accounts),
                build_cache: Cow::Borrowed(&runner.build_cache),
                emulations: Cow::Borrowed(&runner.emulations),
                known_addresses: Cow::Borrowed(&runner.known_addresses),
                known_code_cells: Cow::Borrowed(&runner.known_code_cells),
                backtrace: runner.config.backtrace,
                fork_net: None,
                network: None,
                api_key: None,
            };

            if let Some(failure) = &assert_failure {
                test_report.message = failure.message();
                test_report.details = failure.location().map(|l| l.format_full());
                test_report.location = failure.location();
                test_report.detailed_message =
                    Some(formatter.format_detailed_assert_failure(failure, abi.clone()));

                if let AssertFailure::TransactionNotFound(tx_failure)
                | AssertFailure::TransactionIsFound(tx_failure) = failure
                {
                    test_report.failed_transactions =
                        Some(formatter.parse_failed_transactions(&tx_failure.txs));
                    test_report.failed_transaction_context =
                        Some(formatter.get_failed_transaction_context(tx_failure, abi.clone()));
                }
            } else if expected_exit_code != 0 {
                test_report.message = Some(format!(
                    "Expected exit_code={expected_exit_code}, got={exit_code}"
                ));
                if let GetMethodResult::Success(result) = &get_result {
                    test_report.detailed_message =
                        Some(formatter.format_detailed_exit_code(&test_report, result, exit_code));
                }
            } else {
                test_report.message = Some(format!("exit_code={exit_code}"));
                if let GetMethodResult::Success(result) = &get_result {
                    test_report.detailed_message =
                        Some(formatter.format_detailed_exit_code(&test_report, result, exit_code));
                }
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
                let content = fs::read_to_string(&file_path).unwrap_or_default();
                runner.build_cache.memoize(
                    &test.name,
                    &file_path,
                    &code.to_boc_b64(false)?,
                    &code.cell_hash()?.to_hex().to_ascii_uppercase(),
                    source_map.clone(),
                    Some(
                        contract_abi(
                            &content,
                            file_path.to_string_lossy().as_ref(),
                            &runner.acton_config.mappings,
                        )
                        .into(),
                    ),
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
        .on_suite_finished(&file_path, &suite_stats)?;

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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TestAnnotation {
    Todo,
    Skip,
}

#[derive(Debug)]
pub struct TestDescriptor {
    pub id: i32,
    pub name: Arc<str>,
    pub annotations: Vec<TestAnnotation>,
    pub expected_exit_code: Option<i32>,
    pub gas_limit: Option<u64>,
    pub todo_description: Option<String>,
    pub pos: Pos,
}

fn find_all_test(
    file_path: &str,
    file: &anyhow::Result<SourceFile>,
    content: &str,
) -> Vec<TestDescriptor> {
    let Ok(file) = file else {
        return vec![];
    };

    file.get_methods()
        .filter_map(|method| {
            let name_node = method.name()?;
            let name = name_node.normalized_name(content);

            // get fun `test-foo`() or get fun test_foo() or get fun `test foo`()
            if name.starts_with("test-") || name.starts_with("test_") || name.starts_with("test ") {
                let id = i32::from(CRC16.checksum(name.as_bytes())) | 0x1_00_00;
                let test_annotations = annotations::find_test_annotations(content, method);

                return Some(TestDescriptor {
                    id,
                    name: name.into(),
                    annotations: test_annotations.annotations,
                    expected_exit_code: test_annotations.expected_exit_code,
                    gas_limit: test_annotations.gas_limit,
                    todo_description: test_annotations.todo_description,
                    pos: Pos {
                        row: name_node.syntax().start_position().row,
                        column: name_node.syntax().start_position().column,
                        uri: file_path.to_owned(),
                    },
                });
            }

            None
        })
        .collect()
}
