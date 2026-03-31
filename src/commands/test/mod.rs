use crate::commands::build::build_cmd;
use crate::commands::common::error_fmt;
use crate::commands::test::coverage::{
    collect_coverage, generate_lcov_file, generate_text_file, print_coverage_summary,
};
use crate::commands::test::reporting::console::{ConsoleConfig, ConsoleReporter};
use crate::commands::test::reporting::dot::DotReporter;
use crate::commands::test::reporting::junit::{JUnitConfig, JUnitReporter};
use crate::commands::test::reporting::teamcity::TeamCityReporter;
use crate::commands::test::reporting::ui::{UiReporter, reserve_ui_listener, start_ui_server};
use crate::commands::test::reporting::{
    ReporterManager, TestExecutionContext, TestFailureExecutionContext, TestReport, TestStatus,
    TestSuiteStats, extract_suite_name,
};
use crate::context::{
    AssertFailure, AssertsContext, BuildCache, BuildContext, ChainContext, Context, DebugCtx,
    EmulationsState, Env, IoContext, KnownAddresses,
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
use acton_debug::debugger::any_executor::AnyExecutor;
use acton_debug::debugger::dap::{
    DapTransport, reserve_dap_listener, start_dap_server_with_listener,
};
use acton_debug::debugger::replayer_session::ReplayerDebugSession;
use acton_debug::replayer::TolkReplayer;
use anyhow::anyhow;
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
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, UNIX_EPOCH};
use std::{fs, process};
use tolk_syntax::{AstNode, HasName, SourceFile};
use tolkc::TolkSourceMap;
use ton_abi::{ContractAbi, ContractAbiParseCache, contract_abi, contract_abi_with_file};
use ton_emulator::emulator::Emulator;
use ton_emulator::world_state::{
    AccountsState, LocalAccountsState, RemoteAccountState, RemoteSnapshotCache, WorldState,
};
use ton_executor::get::step::StepGetExecutor;
use ton_executor::get::{GetExecutor, GetMethodResult, RunGetMethodArgs};
use ton_executor::{DEFAULT_CONFIG, ExecutorVerbosity};
use tvmffi::serde::serialize_tuple;
use tvmffi::stack::Tuple;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, HashBytes};
use tycho_types::models::{ShardAccount, StdAddr};
use walkdir::WalkDir;

mod annotations;
mod coverage;
pub mod mutation;
mod profiling;
pub mod reporting;
pub mod trace;

const CRC16: crc::Crc<u16> = crc::Crc::<u16>::new(&crc::CRC_16_XMODEM);

#[derive(Debug)]
pub struct TestResult {
    pub get_result: GetMethodResult,
    pub captured_stdout: String,
    pub captured_stderr: String,
    pub assert_failure: Option<AssertFailure>,
    pub expected_exit_code: Option<i32>,
    pub accounts: FxHashMap<StdAddr, ShardAccount>,
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

                let Some(cached) = cache.get(&contract_info.src, config.debug, 2, "1.3") else {
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
        })
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
        code_cell: &Cell,
        dest_address: &str,
        abi: Arc<ContractAbi>,
        tolk_source_map: Arc<TolkSourceMap>,
    ) -> anyhow::Result<TestResult> {
        let verbosity = self.minimal_log_verbosity();

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
                project_root: self.project_root.clone(),
                abi,
                show_bodies: self.config.show_bodies,
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

        let (result, captured_stdout, captured_stderr, assert_failure, expected_exit_code) =
            if self.config.debug {
                let stack = Boc::encode_base64(serialize_tuple(&Tuple::empty())?);
                let mut executor = StepGetExecutor::new(&stack, &params, Some(DEFAULT_CONFIG))?;
                ffi::register(&mut executor, &mut ctx);
                executor.prepare(test.id, &stack)?;
                let replayer = TolkReplayer::new_live_vm(
                    tolk_source_map.as_ref(),
                    AnyExecutor::Get(executor.clone()),
                )?;
                let mut dbg_session =
                    ReplayerDebugSession::new(self.transport.clone(), replayer, test.name.clone());
                ctx.debug = DebugCtx::new(&mut dbg_session);

                ctx.debug.process_incoming_requests(true)?;

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

                let stack = Boc::encode_base64(serialize_tuple(&Tuple::empty())?);
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
        Self::append_debug_output(&mut captured_stdout, &result);

        Ok(TestResult {
            get_result: result,
            captured_stdout,
            captured_stderr,
            assert_failure,
            expected_exit_code,
            accounts: world_state.take_accounts(),
        })
    }

    fn append_debug_output(stdout: &mut String, get_result: &GetMethodResult) {
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

pub fn test_cmd(path: Option<String>, config: &TestConfig) -> anyhow::Result<()> {
    let project_root = configured_project_root();
    let mut config = config.clone();
    resolve_test_output_paths_from_project_root(&mut config, project_root);

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

    let reports_for_ui = ui_reporter.as_ref().map(UiReporter::get_reports_arc);

    let mut global_reporter = ReporterManager::new();
    TestRunner::setup_reporters(&mut global_reporter, &config, ui_reporter);
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
        debug_listener,
        &mut file_cache,
        &mut global_reporter,
        build_overrides_for_mutations(&config)?,
    )?;

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
        rt.block_on(async { start_ui_server(reports, trace_dir, project_root, listener).await })?;
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

    if total_failed > 0 {
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
    let cache_entry = file_cache.get(file, need_debug_info, 0, "1.3");
    if let Some(cache_entry) = cache_entry {
        return Ok(tolkc::CompilerResult::Success(
            tolkc::compiler::CompilerResultSuccess {
                fift_code: cache_entry.fift_code,
                code_boc64: cache_entry.code_boc64,
                code_hash_hex: cache_entry.code_hash_hex,
                debug_mark_base64: cache_entry.debug_mark_base64,
                new_source_map: cache_entry.new_source_map,
                abi: cache_entry.abi,
            },
        ));
    }

    let mappings = acton_config.mappings();
    let compiler = tolkc::Compiler::new(0)
        .with_mappings(&mappings)
        .with_allow_no_entrypoint(true);
    let compilation_result = compiler.compile(Path::new(file), need_debug_info);
    match &compilation_result {
        tolkc::CompilerResult::Success(result) => {
            let cache_result = file_cache.put(file, result, need_debug_info, 0, "1.3");
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
        tolkc::CompilerResult::Success(result) => result,
        tolkc::CompilerResult::Error(error) => {
            let trimmed_message = error.message.trim();
            anyhow::bail!(trimmed_message.to_string())
        }
    };

    let code_cell = Boc::decode_base64(&result.code_boc64)?;
    let tolk_source_map = Arc::new(TolkSourceMap::from_code_cell(
        result.new_source_map.unwrap_or_default(),
        &code_cell,
        result.debug_mark_base64.as_deref(),
    )?);
    let compiler_abi = result.abi.map(Arc::new);
    let stats = run_file_tests(
        runner,
        filepath,
        tests,
        &code_cell,
        Arc::new(abi),
        compiler_abi,
        tolk_source_map,
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
    compiler_abi: Option<Arc<tolkc::abi::ContractABI>>,
    tolk_source_map: Arc<TolkSourceMap>,
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
            tolk_source_map: tolk_source_map.clone(),
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
        let result = runner.execute_test(
            test,
            code,
            &dest_address,
            abi.clone(),
            tolk_source_map.clone(),
        );
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
                test_report.status = TestStatus::Failed;
                runner.reporter_manager.on_test_finished(&test_report)?;
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
        let mut assert_failure = assert_failure;

        if let (Some(AssertFailure::GetMethod(failure)), GetMethodResult::Success(result)) =
            (&mut assert_failure, &get_result)
        {
            failure.caller_trace = retrace::find_execution_trace(&result.vm_log, &tolk_source_map);
        }

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
        let vm_log_diff = match &get_result {
            GetMethodResult::Success(result) => {
                let logs = vmlogs::convert_to_diff_logs(&result.vm_log);
                (!logs.trim().is_empty()).then_some(logs)
            }
            GetMethodResult::Error(_) => None,
        };

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
                api_key: None,
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
            if let GetMethodResult::Success(get_result) = get_result {
                runner.emulations.save_get_method(&test.name, get_result);
                // TODO: remove this memoize somehow
                let content: Arc<str> = fs::read_to_string(&file_path).unwrap_or_default().into();
                let code_boc64 = Boc::encode_base64(code);
                runner.build_cache.memoize(
                    &test.name,
                    &file_path,
                    &code_boc64,
                    *code.repr_hash(),
                    tolk_source_map.clone(),
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
