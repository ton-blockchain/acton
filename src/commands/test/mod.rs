use crate::commands::build::build_cmd;
use crate::commands::common::{
    error_fmt, executor_verbosity_for_cli_level, max_executor_verbosity,
};
use crate::commands::test::coverage::{
    collect_coverage, generate_lcov_file, generate_lcov_report, generate_text_file,
    print_coverage_summary, total_coverage_score_percentage,
};
use crate::commands::test::reporting::console::{ConsoleConfig, ConsoleReporter};
use crate::commands::test::reporting::dot::DotReporter;
use crate::commands::test::reporting::junit::{JUnitConfig, JUnitReporter};
use crate::commands::test::reporting::teamcity::TeamCityReporter;
use crate::commands::test::reporting::ui::{UiReporter, reserve_ui_listener, start_ui_server};
use crate::commands::test::reporting::{
    FuzzExecutionContext, ReporterManager, TestExecutionContext, TestFailureExecutionContext,
    TestReport, TestStatus, TestSuiteStats, extract_suite_name,
};
use crate::context::{
    AssertFailure, AssertsContext, BuildCache, BuildContext, ChainContext, Context, DebugCtx,
    DebugStopRequested, EmulationsState, Env, IoContext, KnownAddresses, is_debug_stop_requested,
};
use crate::ffi;
use crate::file_build_cache::FileBuildCache;
use crate::formatter::FormatterContext;
use crate::retrace;
use acton_config::color::OwoColorize;
use acton_config::config::{
    ActonConfig, ContractDependency, DependencyKind, project_root as configured_project_root,
};
use acton_config::test::{BacktraceMode, CoverageFormat, ReportFormat, TestConfig};
use acton_debug::replayer::TolkReplayer;
use acton_debug::{
    DapTransport, ReplayerDebugSession, reserve_dap_listener, start_dap_server_with_listener,
};
use anyhow::anyhow;
use crossbeam_channel::{Sender, unbounded};
use dunce;
use globset::{Glob, GlobSet, GlobSetBuilder};
use log::{debug, error, warn};
use path_absolutize::Absolutize;
use regex::Regex;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::net::TcpListener;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, UNIX_EPOCH};
use std::{fs, process};
use tolk_compiler::TolkSourceMap;
use tolk_compiler::abi::ContractABI as CompilerContractABI;
use tolk_syntax::{AstNode, HasName, SourceFile};
use ton_abi::{ContractAbi, ContractAbiParseCache, contract_abi, contract_abi_with_file};
use ton_emulator::emulator::Emulator;
use ton_emulator::world_state::{
    AccountsState, LocalAccountsState, RemoteAccountState, RemoteSnapshotCache, WorldState,
};
use ton_executor::get::step::StepGetExecutor;
use ton_executor::get::{GetExecutor, GetMethodResult, GetMethodResultSuccess, RunGetMethodArgs};
use ton_executor::{DEFAULT_CONFIG, ExecutorVerbosity};
use tvm_ffi::serde::serialize_tuple;
use tvm_ffi::stack::Tuple;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, HashBytes};
use tycho_types::models::{ShardAccount, StdAddr};
use walkdir::WalkDir;

mod annotations;
mod coverage;
mod fuzz;
pub mod mutation;
mod profiling;
pub mod reporting;
pub mod trace;

const CRC16: crc::Crc<u16> = crc::Crc::<u16>::new(&crc::CRC_16_XMODEM);
pub(crate) use self::fuzz::FuzzConfig;
use self::fuzz::{FuzzParameter, attach_test_parameter_metadata, validate_test_configuration};

#[derive(Debug, Clone, Copy)]
struct EvaluatedTestCase {
    passed: bool,
    actual_exit_code: i32,
    gas_used: u64,
    expected_exit_code: i32,
}

#[derive(Debug)]
pub struct TestResult {
    pub get_result: GetMethodResult,
    pub captured_stdout: String,
    pub captured_stderr: String,
    pub assert_failure: Option<AssertFailure>,
    pub expected_exit_code: Option<i32>,
    pub accounts: FxHashMap<StdAddr, ShardAccount>,
    pub executed_get_methods: Vec<GetMethodResultSuccess>,
    pub fuzz: Option<FuzzExecutionContext>,
}

#[derive(Debug)]
pub struct TestRunner<'a> {
    config: TestConfig,
    project_root: PathBuf,
    acton_config: ActonConfig,
    build_cache: BuildCache,
    file_build_cache: &'a mut FileBuildCache,
    known_addresses: KnownAddresses,
    known_code_cells: FxHashMap<HashBytes, String>,
    emulations: EmulationsState,
    transport: DapTransport,
    reporter_manager: &'a mut ReporterManager,
    mutation_overrides: BTreeMap<String, Cell>,
    remote_cache: RemoteSnapshotCache,
    abi_parse_cache: ContractAbiParseCache,
    fuzz_seed: u64,
    /// Contracts used as `library_ref` dependency. We need to register it for correct
    /// work of dependent contracts.
    ref_contracts: BTreeMap<String, Cell>,
}

impl<'a> TestRunner<'a> {
    pub fn new(
        acton_config: ActonConfig,
        config: TestConfig,
        debug_listener: Option<TcpListener>,
        cache: &'a mut FileBuildCache,
        reporter_manager: &'a mut ReporterManager,
        mutation_overrides: BTreeMap<String, Cell>,
    ) -> anyhow::Result<TestRunner<'a>> {
        let transport = if let Some(listener) = debug_listener {
            start_dap_server_with_listener(listener)?
        } else {
            DapTransport::dummy()
        };
        let project_root = configured_project_root().to_path_buf();
        let fuzz_seed = config.fuzz_seed.unwrap_or_else(rand::random);

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

                let Some(cached) = cache.get(&contract_info.src, config.debug, false, 2, "1.3")
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

        Ok(Self {
            config,
            project_root,
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
            abi_parse_cache: ContractAbiParseCache::new(),
            fuzz_seed,
        })
    }

    fn setup_reporters(
        reporter_manager: &mut ReporterManager,
        config: &TestConfig,
        ui_reporter: Option<UiReporter>,
        show_suite_prefix_on_console_lines: bool,
    ) {
        if config.report_formats.is_empty()
            || config.report_formats.contains(&ReportFormat::Console)
        {
            let console_config = ConsoleConfig {
                show_output: true,
                show_suite_prefix: show_suite_prefix_on_console_lines,
            };
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

    fn effective_log_verbosity(&self) -> ExecutorVerbosity {
        let mut verbosity = executor_verbosity_for_cli_level(self.config.verbosity);

        if self.config.debug || self.config.backtrace == Some(BacktraceMode::Full) {
            // for these modes we need all logs for work
            verbosity =
                max_executor_verbosity(verbosity, ExecutorVerbosity::FullLocationStackVerbose);
        }

        if self.config.coverage {
            // for coverage, we need at least locations to map to actual source code
            verbosity = max_executor_verbosity(verbosity, ExecutorVerbosity::FullLocationStack);
        }

        verbosity
    }

    fn execute_test(
        &mut self,
        test: &TestDescriptor,
        code_cell: &Cell,
        dest_address: &str,
        abi: Arc<ContractAbi>,
        compiler_abi: Option<Arc<CompilerContractABI>>,
        source_map: Arc<TolkSourceMap>,
    ) -> anyhow::Result<TestResult> {
        if let Some(fuzz) = test.fuzz {
            return self.execute_fuzz_test(
                test,
                code_cell,
                dest_address,
                abi,
                compiler_abi,
                source_map,
                fuzz,
            );
        }

        let stack = &Tuple::empty();
        self.execute_test_case(
            test,
            code_cell,
            dest_address,
            abi,
            compiler_abi,
            source_map,
            stack,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_test_case(
        &mut self,
        test: &TestDescriptor,
        code_cell: &Cell,
        dest_address: &str,
        abi: Arc<ContractAbi>,
        compiler_abi: Option<Arc<CompilerContractABI>>,
        source_map: Arc<TolkSourceMap>,
        stack: &Tuple,
    ) -> anyhow::Result<TestResult> {
        let verbosity = self.effective_log_verbosity();

        let now = std::time::SystemTime::now();
        let duration_since_epoch = now.duration_since(UNIX_EPOCH).expect("Time went backwards");

        let params = RunGetMethodArgs {
            code: Boc::encode_base64(code_cell),
            data: Boc::encode_base64(Cell::default()), // for tests, we use empty cell as a data
            verbosity,
            libs: Default::default(),
            address: dest_address.to_owned(),
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
                project_root: self.project_root.clone(),
                abi,
                show_bodies: self.config.show_bodies,
                default_log_level: verbosity,
                wallets: self.acton_config.wallets.as_ref(),
                open_wallets: Default::default(), // in tests, we never use real wallets
                build_override: self.mutation_overrides.clone(),
                explorer: None,
                fork_net: self.config.fork_net.clone(),
                running_id: test.name.clone(),
                test_code: Some(code_cell.clone()),
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
            message_iters: Default::default(),
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

        let stack = Boc::encode_base64(serialize_tuple(stack)?);

        let (result, captured_stdout, captured_stderr, assert_failure, expected_exit_code) =
            if self.config.debug {
                let mut executor = StepGetExecutor::new(&stack, &params, Some(DEFAULT_CONFIG))?;
                ffi::register(&mut executor, &mut ctx);
                executor.prepare(test.id, &stack)?;
                let mut replayer =
                    TolkReplayer::new_live_vm(source_map.as_ref(), executor.clone().into())?;
                replayer.set_compiler_abi(compiler_abi);
                let mut dbg_session =
                    ReplayerDebugSession::new(self.transport.clone(), replayer, test.name.clone());
                ctx.debug = DebugCtx::new(&mut dbg_session);

                if ctx.debug.process_incoming_requests(true)? {
                    return Err(DebugStopRequested.into());
                }

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
                    *ctx.asserts.expected_exit_code,
                )
            } else {
                let mut executor = GetExecutor::new(&params)?;
                ffi::register(&mut executor, &mut ctx);

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
                    *ctx.asserts.expected_exit_code,
                )
            };

        let mut captured_stdout = captured_stdout;
        Self::append_debug_output(&mut captured_stdout, &result, verbosity);

        let executed_get_methods = if self.config.coverage {
            // save results for coverage only in coverage mode since cloning is expensive due to logs
            match &result {
                GetMethodResult::Success(success) => vec![success.clone()],
                GetMethodResult::Error(_) => Vec::new(),
            }
        } else {
            Vec::new()
        };

        Ok(TestResult {
            get_result: result,
            captured_stdout,
            captured_stderr,
            assert_failure,
            expected_exit_code,
            accounts: world_state.take_accounts(),
            executed_get_methods,
            fuzz: None,
        })
    }

    fn append_debug_output(
        stdout: &mut String,
        get_result: &GetMethodResult,
        verbosity: ExecutorVerbosity,
    ) {
        if matches!(verbosity, ExecutorVerbosity::Off) {
            return;
        }

        let GetMethodResult::Success(result) = get_result else {
            return;
        };

        let debug_output = result
            .vm_log
            .lines()
            .filter_map(|line| line.strip_prefix("#DEBUG#:"))
            .map(str::trim_start)
            .collect::<Vec<_>>()
            .join("\n");

        if debug_output.is_empty() {
            return;
        }

        if !stdout.is_empty() && !stdout.ends_with('\n') {
            stdout.push('\n');
        }
        stdout.push_str(&debug_output);
        stdout.push('\n');
    }
}

fn evaluate_test_case(
    test: &TestDescriptor,
    get_result: &GetMethodResult,
    assert_failure: Option<&AssertFailure>,
    dynamic_expected_exit_code: Option<i32>,
) -> EvaluatedTestCase {
    let (exit_code, gas_used) = match get_result {
        GetMethodResult::Success(result) => {
            let gas_used = result.gas_used.parse::<u64>().unwrap_or(0);
            (result.vm_exit_code, gas_used)
        }
        GetMethodResult::Error(_) => (999, 0),
    };

    let expected_exit_code = dynamic_expected_exit_code
        .or(test.expected_exit_code)
        .unwrap_or(0);

    let gas_limit_exceeded = if let Some(limit) = test.gas_limit {
        gas_used > limit
    } else {
        false
    };
    let failed = exit_code != expected_exit_code
        || gas_limit_exceeded
        || matches!(assert_failure, Some(AssertFailure::Assume(_)))
        || (exit_code == 0 && assert_failure.is_some());

    EvaluatedTestCase {
        passed: !failed,
        actual_exit_code: exit_code,
        gas_used,
        expected_exit_code,
    }
}

pub fn test_cmd(path: Option<String>, config: &TestConfig) -> anyhow::Result<()> {
    let project_root = configured_project_root();
    let mut config = config.clone();
    resolve_test_output_paths_from_project_root(&mut config, project_root);
    if config.fuzz_seed.is_none() {
        config.fuzz_seed = Some(rand::random());
    }

    // First we need to build all contracts and generate all dependency files with code.
    // Internal mutation child runs may skip this via environment variable.
    if need_to_build() {
        build_cmd(None, config.clear_cache, None, None, None, None, false)?;
    }
    println!("     {} tests", "Running".green().bold());

    // If path is omitted, default to project root.
    let path = path.unwrap_or_else(|| project_root.to_string_lossy().to_string());

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
        let search_root = dunce::canonicalize(&path).unwrap_or_else(|_| PathBuf::from(&path));
        let project_root_abs =
            dunce::canonicalize(project_root).unwrap_or_else(|_| project_root.to_path_buf());
        find_test_files_recursively(
            &search_root,
            &project_root_abs,
            &config.exclude_patterns,
            &config.include_patterns,
        )?
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect()
    } else {
        anyhow::bail!("Path '{path}' is neither a file nor a directory");
    };

    let acton_config = ActonConfig::load()?;
    let debug_listener = if config.debug {
        Some(reserve_dap_listener(config.debug_port)?)
    } else {
        None
    };
    // try to reserve port before test run for better UX
    let ui_listener = if config.ui {
        Some(reserve_ui_listener(config.ui_port)?)
    } else {
        None
    };

    let ui_reporter = if config.ui {
        Some(UiReporter::new())
    } else {
        None
    };
    let run_files_in_parallel = should_run_test_files_in_parallel(&config, test_files.len());

    let reports_for_ui = ui_reporter.as_ref().map(UiReporter::get_reports_arc);

    let mut global_reporter = ReporterManager::new();
    TestRunner::setup_reporters(
        &mut global_reporter,
        &config,
        ui_reporter,
        run_files_in_parallel,
    );
    global_reporter.init()?;
    global_reporter.on_testing_started()?;

    let mut file_cache = FileBuildCache::new(None)?;
    let mutation_overrides = build_overrides_for_mutations(&config)?;

    let mut total_passed = 0;
    let mut total_failed = 0;
    let mut total_skipped = 0;
    let mut total_todo = 0;

    let mut runner = TestRunner::new(
        acton_config,
        config.clone(),
        debug_listener,
        &mut file_cache,
        &mut global_reporter,
        mutation_overrides.clone(),
    )?;

    if run_files_in_parallel {
        let stats = run_test_files_in_parallel(&mut runner, &test_files, &mutation_overrides)?;
        total_passed += stats.passed;
        total_failed += stats.failed;
        total_skipped += stats.skipped;
        total_todo += stats.todo;
    } else {
        for (index, file) in test_files.iter().enumerate() {
            let result = run_tests_for_file(&mut runner, file);
            match result {
                Ok(stats) => {
                    total_passed += stats.passed;
                    total_failed += stats.failed;
                    total_skipped += stats.skipped;
                    total_todo += stats.todo;

                    if stats.stopped {
                        break;
                    }

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

    let mut coverage_lcov = None;
    let mut coverage_threshold_failed = false;

    if config.coverage {
        let project_root = configured_project_root().to_path_buf();
        let wrapper_roots: Vec<_> = runner
            .acton_config
            .mappings()
            .into_iter()
            .flat_map(IntoIterator::into_iter)
            .filter_map(|(key, path)| (key == "@wrappers").then_some(path))
            .map(PathBuf::from)
            .map(|path| {
                let path = if path.is_absolute() {
                    path
                } else {
                    project_root.join(path)
                };
                dunce::canonicalize(&path).unwrap_or(path)
            })
            .collect();
        let coverage = collect_coverage(
            &runner.emulations,
            &runner.build_cache,
            &wrapper_roots,
            config.coverage_include_wrappers,
            config.coverage_include_tests,
        );
        print_coverage_summary(&coverage);
        if config.ui {
            coverage_lcov = Some(generate_lcov_report(&coverage));
        }

        if let Some(format_type) = &config.coverage_format {
            println!();
            match format_type {
                CoverageFormat::Lcov => {
                    let lcov_path = config.coverage_file.as_deref().unwrap_or("lcov.info");
                    generate_lcov_file(&coverage, lcov_path).map_err(|err| {
                        anyhow!("Failed to generate LCOV file '{lcov_path}': {err}")
                    })?;
                    println!("LCOV file saved in {lcov_path}");
                }
                CoverageFormat::Text => {
                    let text_path = config.coverage_file.as_deref().unwrap_or("coverage.txt");
                    generate_text_file(&coverage, text_path).map_err(|err| {
                        anyhow!("Failed to generate text coverage file '{text_path}': {err}")
                    })?;
                    println!("Text coverage file saved in {text_path}");
                }
            }
        }

        if !config.ui
            && let Some(minimum_percent) = config.coverage_minimum_percent
        {
            if !minimum_percent.is_finite() || !(0.0..=100.0).contains(&minimum_percent) {
                anyhow::bail!(
                    "coverage minimum percent must be between 0 and 100, got {minimum_percent}"
                );
            }
            let actual_percent = total_coverage_score_percentage(&coverage);
            if actual_percent < minimum_percent {
                coverage_threshold_failed = true;
                println!(
                    "\n{}: coverage score {:.2}% is below the required minimum of {:.2}%.",
                    "Error".red(),
                    actual_percent,
                    minimum_percent
                );
            }
        }
    }

    runner.reporter_manager.finalize()?;

    if config.snapshot.is_some() || config.baseline_snapshot.is_some() {
        match profiling::collect_profile(&runner) {
            Ok(()) => {}
            Err(err) => {
                if config.fail_on_diff {
                    return Err(err);
                }
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
        let listener =
            ui_listener.ok_or_else(|| anyhow!("internal error: UI listener was not reserved"))?;
        let reports = reports.lock().expect("cannot lock mutex").clone();
        let trace_dir = config.save_test_trace.clone();
        let project_root = dunce::canonicalize(configured_project_root())
            .unwrap_or_else(|_| configured_project_root().to_path_buf())
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
            start_ui_server(reports, trace_dir, project_root, coverage_lcov, listener).await
        })?;
    }

    if let Some(filter) = &config.filter
        && total_tests == 0
    {
        // there is some `--filter` and no test ran, likely something is wrong
        println!(
            "\nNo tests matched filter {}, please check the filter spelling/pattern.",
            filter.yellow()
        );
        process::exit(1);
    }

    if total_failed > 0 || coverage_threshold_failed {
        process::exit(1)
    }
    Ok(())
}

fn need_to_build() -> bool {
    let Ok(value) = std::env::var("ACTON_INTERNAL_SKIP_BUILD") else {
        return true;
    };

    value.trim() != "1"
}

fn resolve_test_output_paths_from_project_root(config: &mut TestConfig, project_root: &Path) {
    config.save_test_trace = config
        .save_test_trace
        .as_deref()
        .map(|path| resolve_project_relative_path(project_root, path));
    config.junit_path = config
        .junit_path
        .as_deref()
        .map(|path| resolve_project_relative_path(project_root, path));
}

fn resolve_project_relative_path(project_root: &Path, path: &str) -> String {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_string_lossy().to_string()
    } else {
        project_root.join(path).to_string_lossy().to_string()
    }
}

fn build_overrides_for_mutations(config: &TestConfig) -> anyhow::Result<BTreeMap<String, Cell>> {
    let mut mutation_overrides = BTreeMap::new();

    if let Some((name, code_b64)) = config
        .mutate_overrides
        .as_ref()
        .unwrap_or(&String::new())
        .split_once(':')
    {
        let code_cell = Boc::decode_base64(code_b64)
            .map_err(|e| anyhow!("Failed to decode mutation override for {name}: {e}"))?;
        mutation_overrides.insert(name.to_owned(), code_cell);
    }
    Ok(mutation_overrides)
}

pub fn find_test_files_recursively(
    dir_path: &Path,
    project_root: &Path,
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
        "**/.codex/**",
        "**/.claude/**",
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

    let root = dir_path;

    let it = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            if !entry.file_type().is_dir() {
                return true;
            }
            let p = entry.path();
            let rel = p.strip_prefix(project_root).unwrap_or(p);
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

            let rel = path.strip_prefix(project_root).unwrap_or(path);

            if let Some(name) = rel.file_name().and_then(|s| s.to_str()) {
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

#[derive(Debug, Default)]
struct TestStats {
    passed: usize,
    failed: usize,
    skipped: usize,
    todo: usize,
    stopped: bool,
}

impl TestStats {
    const fn add_assign(&mut self, other: &TestStats) {
        self.passed += other.passed;
        self.failed += other.failed;
        self.skipped += other.skipped;
        self.todo += other.todo;
        self.stopped |= other.stopped;
    }
}

#[derive(Debug)]
struct TestFileRunOutcome {
    stats: TestStats,
    build_cache: BuildCache,
    emulations: EmulationsState,
}

#[derive(Debug)]
enum ReporterEvent {
    SuiteStarted {
        file_path: PathBuf,
        tests: Vec<TestDescriptor>,
    },
    TestStarted(TestReport),
    TestFinished(TestReport),
    SuiteFinished {
        file_path: PathBuf,
        stats: TestSuiteStats,
    },
}

#[derive(Debug)]
enum ParallelTestRunMessage {
    ReporterEvent(Box<ReporterEvent>),
    Outcome(anyhow::Result<TestFileRunOutcome>),
}

struct StreamingReporter {
    sender: Sender<ParallelTestRunMessage>,
}

impl StreamingReporter {
    const fn new(sender: Sender<ParallelTestRunMessage>) -> Self {
        Self { sender }
    }

    fn send_event(&self, event: ReporterEvent) {
        let _ = self
            .sender
            .send(ParallelTestRunMessage::ReporterEvent(Box::new(event)));
    }
}

impl reporting::TestReporter for StreamingReporter {
    fn on_suite_started(
        &mut self,
        file_path: &Path,
        tests: &[TestDescriptor],
    ) -> anyhow::Result<()> {
        self.send_event(ReporterEvent::SuiteStarted {
            file_path: file_path.to_path_buf(),
            tests: tests.to_vec(),
        });
        Ok(())
    }

    fn on_test_started(&mut self, test: &TestReport) -> anyhow::Result<()> {
        self.send_event(ReporterEvent::TestStarted(test.clone()));
        Ok(())
    }

    fn on_test_finished(&mut self, test: &TestReport) -> anyhow::Result<()> {
        self.send_event(ReporterEvent::TestFinished(test.clone()));
        Ok(())
    }

    fn on_suite_finished(
        &mut self,
        file_path: &Path,
        stats: &TestSuiteStats,
    ) -> anyhow::Result<()> {
        self.send_event(ReporterEvent::SuiteFinished {
            file_path: file_path.to_path_buf(),
            stats: stats.clone(),
        });
        Ok(())
    }
}

const fn should_run_test_files_in_parallel(config: &TestConfig, test_file_count: usize) -> bool {
    test_file_count > 1 && !config.debug && !config.fail_fast
}

fn run_test_files_in_parallel(
    runner: &mut TestRunner,
    test_files: &[String],
    mutation_overrides: &BTreeMap<String, Cell>,
) -> anyhow::Result<TestStats> {
    let (sender, receiver) = unbounded();

    for file_path in test_files {
        let file_path = file_path.clone();
        let sender = sender.clone();
        let acton_config = runner.acton_config.clone();
        let config = runner.config.clone();
        let mutation_overrides = mutation_overrides.clone();

        rayon::spawn(move || {
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                collect_test_file_run_outcome(
                    &file_path,
                    acton_config,
                    config,
                    mutation_overrides,
                    sender.clone(),
                )
            }))
            .map_err(|panic_payload| {
                let panic_message = if let Some(message) = panic_payload.downcast_ref::<&str>() {
                    (*message).to_owned()
                } else if let Some(message) = panic_payload.downcast_ref::<String>() {
                    message.clone()
                } else {
                    "unknown panic".to_owned()
                };
                anyhow!(
                    "internal error: parallel test worker panicked while running '{file_path}': {panic_message}"
                )
            })
            .and_then(|result| result);

            let _ = sender.send(ParallelTestRunMessage::Outcome(outcome));
        });
    }
    drop(sender);

    let mut stats = TestStats::default();
    for message in receiver {
        match message {
            ParallelTestRunMessage::ReporterEvent(event) => {
                dispatch_reporter_event(runner.reporter_manager, *event)?;
            }
            ParallelTestRunMessage::Outcome(result) => match result {
                Ok(outcome) => {
                    stats.add_assign(&outcome.stats);
                    runner.build_cache.built.extend(outcome.build_cache.built);
                    runner.emulations.results.extend(outcome.emulations.results);
                }
                Err(err) => {
                    eprintln!("{err}");
                    stats.failed += 1;
                }
            },
        }
    }

    Ok(stats)
}

fn collect_test_file_run_outcome(
    filepath: &str,
    acton_config: ActonConfig,
    config: TestConfig,
    mutation_overrides: BTreeMap<String, Cell>,
    sender: Sender<ParallelTestRunMessage>,
) -> anyhow::Result<TestFileRunOutcome> {
    let mut reporter_manager = ReporterManager::new();
    reporter_manager.add_reporter(Box::new(StreamingReporter::new(sender)));

    let mut file_cache = FileBuildCache::new(None)?;
    let mut runner = TestRunner::new(
        acton_config,
        config,
        None,
        &mut file_cache,
        &mut reporter_manager,
        mutation_overrides,
    )?;
    let stats = run_tests_for_file(&mut runner, filepath)?;

    Ok(TestFileRunOutcome {
        stats,
        build_cache: runner.build_cache.clone(),
        emulations: runner.emulations.clone(),
    })
}

fn dispatch_reporter_event(
    reporter_manager: &mut ReporterManager,
    event: ReporterEvent,
) -> anyhow::Result<()> {
    match event {
        ReporterEvent::SuiteStarted { file_path, tests } => {
            reporter_manager.on_suite_started(&file_path, &tests)?;
        }
        ReporterEvent::TestStarted(test) => {
            reporter_manager.on_test_started(&test)?;
        }
        ReporterEvent::TestFinished(test) => {
            reporter_manager.on_test_finished(&test)?;
        }
        ReporterEvent::SuiteFinished { file_path, stats } => {
            reporter_manager.on_suite_finished(&file_path, &stats)?;
        }
    }

    Ok(())
}

fn compile_test_file(
    file_cache: &mut FileBuildCache,
    file: &str,
    need_debug_info: bool,
    acton_config: &ActonConfig,
) -> anyhow::Result<tolk_compiler::CompilerResult> {
    let cache_entry = file_cache.get(file, need_debug_info, false, 0, "1.3");
    if let Some(cache_entry) = cache_entry {
        return Ok(tolk_compiler::CompilerResult::Success(
            tolk_compiler::compiler::CompilerResultSuccess {
                fift_code: cache_entry.fift_code.unwrap_or_default(),
                code_boc64: cache_entry.code_boc64,
                code_hash_hex: cache_entry.code_hash_hex,
                debug_mark_base64: cache_entry.debug_mark_base64,
                new_source_map: cache_entry.new_source_map,
                abi: cache_entry.abi,
            },
        ));
    }

    let mappings = acton_config.mappings();
    let compiler = tolk_compiler::Compiler::new(0)
        .with_mappings(&mappings)
        .with_allow_no_entrypoint(true);
    let compilation_result = compiler.compile(Path::new(file), need_debug_info);
    match &compilation_result {
        tolk_compiler::CompilerResult::Success(result) => {
            let cache_result = file_cache.put(file, result, need_debug_info, false, 0, "1.3");
            match cache_result {
                Ok(()) => {}
                Err(err) => {
                    error!("Cannot cache result of compilation {file}: {err}",);
                }
            }
        }
        tolk_compiler::CompilerResult::Error(_) => {
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

    let mappings = runner.acton_config.mappings();

    let abi = contract_abi_with_file(
        content.into(),
        filepath,
        &file,
        &mappings,
        Some(&mut runner.abi_parse_cache),
    );

    let config = &runner.config;
    let need_debug_info =
        config.debug || config.backtrace == Some(BacktraceMode::Full) || config.coverage;

    let now = Instant::now();
    let compilation_result = compile_test_file(
        runner.file_build_cache,
        filepath,
        need_debug_info,
        &runner.acton_config,
    )?;
    debug!(
        "Test file '{filepath}' compilation time: {:?}",
        now.elapsed()
    );

    let result = match compilation_result {
        tolk_compiler::CompilerResult::Success(result) => result,
        tolk_compiler::CompilerResult::Error(error) => {
            let trimmed_message = error.message.trim();
            anyhow::bail!(trimmed_message.to_string())
        }
    };

    let code_cell = Boc::decode_base64(&result.code_boc64)?;
    let source_map = Arc::new(TolkSourceMap::from_code_cell(
        result.new_source_map.unwrap_or_default(),
        &code_cell,
        result.debug_mark_base64.as_deref(),
    )?);
    let compiler_abi = result.abi.map(Arc::new);
    let tests = attach_test_parameter_metadata(tests, &abi, compiler_abi.as_deref());
    let stats = run_file_tests(
        runner,
        filepath,
        tests,
        &code_cell,
        Arc::new(abi),
        compiler_abi,
        source_map,
    )?;
    Ok(stats)
}

#[allow(clippy::too_many_arguments)]
fn run_file_tests(
    runner: &mut TestRunner,
    file_path: &str,
    tests: Vec<TestDescriptor>,
    code: &Cell,
    abi: Arc<ContractAbi>,
    compiler_abi: Option<Arc<tolk_compiler::abi::ContractABI>>,
    source_map: Arc<TolkSourceMap>,
) -> anyhow::Result<TestStats> {
    let file_path = Path::new(file_path).absolutize()?;
    let filtered_tests = if let Some(pattern) = &runner.config.filter {
        let regex = match Regex::new(pattern) {
            Ok(r) => r,
            Err(e) => {
                anyhow::bail!("Invalid regex pattern {}: {e}", pattern.yellow());
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
    let mut stopped = false;
    let mappings = runner.acton_config.mappings();
    for test in &filtered_tests {
        let suite_name = extract_suite_name(&file_path);
        let mut test_report = TestReport {
            name: test.name.clone(),
            suite_name,
            file_path: file_path.to_path_buf(),
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
            location: None,
            abi: abi.clone(),
            compiler_abi: compiler_abi.clone(),
            source_map: source_map.clone(),
            show_bodies: runner.config.show_bodies,
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
            test_report.details = test.status_description.clone();
            runner.reporter_manager.on_test_finished(&test_report)?;
            todo += 1;
            continue;
        }

        if test.annotations.contains(&TestAnnotation::Skip) {
            test_report.status = TestStatus::Skipped;
            test_report.details = test.status_description.clone();
            runner.reporter_manager.on_test_finished(&test_report)?;
            skipped += 1;
            continue;
        }

        if let Err(err) = validate_test_configuration(test, &runner.config) {
            test_report.status = TestStatus::Failed;
            test_report.message = Some(err.to_string());
            runner.reporter_manager.on_test_finished(&test_report)?;
            failed += 1;

            if runner.config.fail_fast {
                break;
            }
            continue;
        }

        let start_time = Instant::now();
        let result = runner.execute_test(
            test,
            code,
            &dest_address,
            abi.clone(),
            compiler_abi.clone(),
            source_map.clone(),
        );
        let result = match result {
            Ok(result) => result,
            Err(err) if is_debug_stop_requested(&err) => {
                test_report.status = TestStatus::Skipped;
                test_report.details = Some("Debug session stopped".to_string());
                test_report.duration = start_time.elapsed();
                runner.reporter_manager.on_test_finished(&test_report)?;
                skipped += 1;
                stopped = true;
                break;
            }
            Err(err) => {
                test_report.status = TestStatus::Failed;
                test_report.message = Some(format!("Cannot execute test '{}': {err}", test.name));
                runner.reporter_manager.on_test_finished(&test_report)?;
                failed += 1;

                if runner.config.fail_fast {
                    break;
                }
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
            executed_get_methods,
            fuzz,
            ..
        } = result;
        let mut assert_failure = assert_failure;

        if let (Some(AssertFailure::GetMethod(failure)), GetMethodResult::Success(result)) =
            (&mut assert_failure, &get_result)
        {
            failure.caller_trace = retrace::find_execution_trace(&result.vm_log, &source_map);
        }

        let outcome = evaluate_test_case(
            test,
            &get_result,
            assert_failure.as_ref(),
            dyn_expected_exit_code,
        );
        let exit_code = outcome.actual_exit_code;
        let expected_exit_code = outcome.expected_exit_code;
        let gas_used = outcome.gas_used;
        let vm_log_diff = match &get_result {
            GetMethodResult::Success(result) => {
                let logs = tvm_logs::convert_to_diff_logs(&result.vm_log);
                (!logs.trim().is_empty()).then_some(logs)
            }
            GetMethodResult::Error(_) => None,
        };
        let test_passed = outcome.passed;

        test_report.duration = duration;
        let failure_execution = if test_passed {
            None
        } else {
            Some(TestFailureExecutionContext {
                get_result: get_result.clone(),
                accounts: accounts.clone(),
                build_cache: runner.build_cache.clone(),
                emulations: runner.emulations.clone(),
                known_addresses: runner.known_addresses.clone(),
                known_code_cells: runner.known_code_cells.clone(),
            })
        };
        test_report.execution = Some(TestExecutionContext {
            gas_used,
            stdout: captured_stdout,
            stderr: captured_stderr,
            vm_log_diff,
            assert_failure: assert_failure.clone(),
            expected_exit_code,
            fuzz: fuzz.clone(),
            failure: failure_execution,
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
                show_bodies: runner.config.show_bodies,
                has_wallets_config: false,
                available_wallets: vec![],
                backtrace: runner.config.backtrace,
                fork_net: None,
                network: None,
            };

            if let Some(failure) = &assert_failure {
                test_report.message = failure.message();
                if test_report.message.is_none()
                    && let AssertFailure::GetMethod(get_method_failure) = failure
                {
                    test_report.message = Some(FormatterContext::strip_ansi_text(
                        &FormatterContext::format_get_method_assert_failure_title(
                            get_method_failure,
                        ),
                    ));
                }
                test_report.details = failure.location().map(|l| l.format_full());
                test_report.location = failure.location();
                let detailed = formatter.format_detailed_assert_failure(failure, abi.clone());
                test_report.detailed_message = Some(FormatterContext::strip_ansi_text(&detailed));

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
            if !executed_get_methods.is_empty() {
                for get_result in executed_get_methods {
                    runner.emulations.save_get_method(&test.name, get_result);
                }

                // TODO: remove this memoize somehow
                let content: Arc<str> = fs::read_to_string(&file_path).unwrap_or_default().into();
                let code_boc64 = Boc::encode_base64(code);
                runner.build_cache.memoize(
                    &test.name,
                    &file_path,
                    &code_boc64,
                    *code.repr_hash(),
                    source_map.clone(),
                    Some(
                        contract_abi(content, file_path.to_string_lossy().as_ref(), &mappings)
                            .into(),
                    ),
                    None,
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
        stopped,
    })
}

fn contract_address(code: &Cell) -> anyhow::Result<String> {
    let mut state_init_builder = CellBuilder::new();
    state_init_builder.store_bit(false)?; // split_depth absent
    state_init_builder.store_bit(false)?; // tick_tock absent
    state_init_builder.store_bit(true)?; // code present
    state_init_builder.store_reference(code.clone())?;
    state_init_builder.store_bit(true)?; // data present
    state_init_builder.store_reference(Cell::default())?;
    state_init_builder.store_bit(false)?; // library absent

    let state_init = state_init_builder.build()?;
    Ok(format!(
        "0:{}",
        hex::encode(state_init.repr_hash().as_array())
    ))
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

#[derive(Debug, Clone)]
pub struct TestDescriptor {
    pub id: i32,
    pub name: Arc<str>,
    pub annotations: Vec<TestAnnotation>,
    fuzz: Option<FuzzConfig>,
    pub expected_exit_code: Option<i32>,
    pub gas_limit: Option<u64>,
    pub status_description: Option<String>,
    pub declared_parameter_count: usize,
    parameters: Vec<FuzzParameter>,
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

            // Preferred style: get fun `test foo`() (legacy dash/underscore forms stay supported)
            if name.starts_with("test-") || name.starts_with("test_") || name.starts_with("test ") {
                let id = i32::from(CRC16.checksum(name.as_bytes())) | 0x1_00_00;
                let test_annotations = annotations::find_test_annotations(content, method);
                let declared_parameter_count = method.parameters().count();

                return Some(TestDescriptor {
                    id,
                    name: name.into(),
                    annotations: test_annotations.annotations,
                    fuzz: test_annotations.fuzz,
                    expected_exit_code: test_annotations.expected_exit_code,
                    gas_limit: test_annotations.gas_limit,
                    status_description: test_annotations.status_description,
                    declared_parameter_count,
                    parameters: Vec::new(),
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
