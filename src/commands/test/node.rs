use super::reporting::{
    ReporterManager, TestExecutionContext, TestFailureExecutionContext, TestReport, TestStatus,
    TestSuiteStats, extract_suite_name,
};
use super::{
    Pos, TestDescriptor, TestRunner, TestStats, build_overrides_for_mutations,
    empty_test_selection_message, need_to_build, resolve_test_output_paths_from_project_root,
};
use crate::commands::build::{BuildCommandOptions, build_cmd};
use crate::commands::common::error_fmt;
use crate::context::{
    AssertFailure, DisplayParam, KnownAddress, TransactionGenericAssertFailure,
    TransactionNotFoundParams,
};
use crate::file_build_cache::FileBuildCache;
use crate::formatter::FormatterContext;
use acton_config::color::OwoColorize;
use acton_config::config::{ActonConfig, project_root as configured_project_root};
use acton_config::test::TestConfig;
use anyhow::{Context, anyhow};
use num_bigint::BigInt;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::Deserialize;
use std::borrow::Cow;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tolk_compiler::SourceMap;
use ton_emulator::emulator::{SendMessageResult, SendMessageResultSuccess};
use ton_executor::get::{GetMethodResult, GetMethodResultSuccess};
use ton_source_map::SourceLocation;
use tvm_ffi::stack::{Tuple, TupleItem};
use tycho_types::boc::Boc;
use tycho_types::cell::Cell;
use tycho_types::models::{IntAddr, ShardAccount, StdAddr, StdAddrFormat, Transaction};
use walkdir::WalkDir;

const EVENT_PREFIX: &str = "__ACTON_NODE_EVENT__";

pub fn test_node_cmd(path: String, config: &TestConfig) -> anyhow::Result<()> {
    let project_root = configured_project_root();
    let mut config = config.clone();
    resolve_test_output_paths_from_project_root(&mut config, project_root);

    if need_to_build() {
        build_cmd(BuildCommandOptions {
            clear_cache: config.clear_cache,
            quiet_no_contracts: true,
            ..BuildCommandOptions::default()
        })?;
    }
    println!("     {} node tests", "Running".green().bold());

    let test_files = resolve_node_test_files(&path)?;
    let acton_config = ActonConfig::load()?;
    let mut file_cache = FileBuildCache::new(None)?;
    let mut reporter = ReporterManager::new();
    let reporter_project_root =
        dunce::canonicalize(project_root).unwrap_or_else(|_| project_root.to_path_buf());
    TestRunner::setup_reporters(&mut reporter, &config, &reporter_project_root, None);
    reporter.init()?;

    let testing_started_at = Instant::now();
    reporter.on_testing_started()?;

    let mut total_passed = 0;
    let mut total_failed = 0;
    let mut total_skipped = 0;
    let mut total_todo = 0;

    let worker = resolve_node_worker(project_root)?;
    let mut runner = TestRunner::new(
        acton_config,
        config.clone(),
        None,
        &mut file_cache,
        &mut reporter,
        build_overrides_for_mutations(&config)?,
    )?;

    for (index, file) in test_files.iter().enumerate() {
        let stats = run_node_tests_for_file(&mut runner, &worker, file)?;
        total_passed += stats.passed;
        total_failed += stats.failed;
        total_skipped += stats.skipped;
        total_todo += stats.todo;

        if index + 1 < test_files.len() && config.report_formats.is_empty() {
            println!();
        }

        if config.fail_fast && total_failed > 0 {
            break;
        }
    }

    let total_tests = total_passed + total_failed + total_skipped + total_todo;
    let global_stats = TestSuiteStats {
        duration: testing_started_at.elapsed(),
        failed: total_failed,
        passed: total_passed,
        skipped: total_skipped,
        todo: total_todo,
        total: total_tests,
    };
    runner.reporter_manager.on_testing_finished(&global_stats)?;

    if let Some(message) =
        empty_test_selection_message(&test_files_as_strings(&test_files), &config, total_tests)
    {
        runner.reporter_manager.finalize()?;
        println!("\n{message}");
        std::process::exit(1);
    }

    runner.reporter_manager.finalize()?;

    if total_failed > 0 {
        std::process::exit(1)
    }

    Ok(())
}

pub(super) fn run_node_tests_for_file(
    runner: &mut TestRunner<'_>,
    worker: &Path,
    file: &Path,
) -> anyhow::Result<TestStats> {
    let output = Command::new("bun")
        .arg(worker)
        .arg(file)
        .current_dir(configured_project_root())
        .env_remove("ACTON_TEST_HOST")
        .env(
            "ACTON_TEST_FAIL_FAST",
            if runner.config.fail_fast { "1" } else { "0" },
        )
        .env(
            "ACTON_NODE_COVERAGE",
            if runner.config.coverage { "1" } else { "0" },
        )
        .env(
            "ACTON_NODE_TRACE",
            if runner.config.save_test_trace.is_some() {
                "1"
            } else {
                "0"
            },
        )
        .envs(
            runner
                .config
                .filter
                .as_ref()
                .map(|filter| ("ACTON_TEST_FILTER", filter.as_str())),
        )
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| "Failed to run `bun` for node tests")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut events = Vec::new();
    let mut raw_stdout = String::new();

    for line in stdout.lines() {
        if let Some(raw) = line.strip_prefix(EVENT_PREFIX) {
            events.push(serde_json::from_str::<NodeEvent>(raw).with_context(|| {
                format!("Failed to parse node test event from {}", file.display())
            })?);
        } else {
            raw_stdout.push_str(line);
            raw_stdout.push('\n');
        }
    }

    if events.is_empty() {
        anyhow::bail!(
            "Node test worker produced no events for {}\nstdout:\n{}\nstderr:\n{}",
            file.display(),
            raw_stdout.trim(),
            stderr.trim()
        );
    }

    run_reported_node_events(runner, file, events, raw_stdout, stderr.to_string())
}

fn run_reported_node_events(
    runner: &mut TestRunner<'_>,
    file: &Path,
    events: Vec<NodeEvent>,
    raw_stdout: String,
    raw_stderr: String,
) -> anyhow::Result<TestStats> {
    let mut tests = Vec::new();
    let mut stats = TestStats {
        failed: 0,
        passed: 0,
        skipped: 0,
        stopped: false,
        todo: 0,
    };
    let source_map = Arc::new(SourceMap::default());
    let file = dunce::canonicalize(file).unwrap_or_else(|_| file.to_path_buf());
    ensure_node_contract_metadata(runner)?;

    for event in events {
        match event {
            NodeEvent::Tests { tests: found, .. } => {
                tests = found
                    .into_iter()
                    .map(|test| TestDescriptor {
                        annotations: Vec::new(),
                        declared_parameter_count: 0,
                        expected_exit_code: None,
                        fuzz: None,
                        gas_limit: None,
                        id: test.id,
                        name: Arc::from(test.name),
                        parameters: Vec::new(),
                        pos: Pos {
                            column: test.column,
                            row: test.row,
                            uri: file.to_string_lossy().to_string(),
                        },
                        status_description: None,
                    })
                    .collect::<Vec<_>>();
                runner.reporter_manager.on_suite_started(&file, &tests)?;
            }
            NodeEvent::Coverage { id, records } => {
                if let Some(test) = tests.iter().find(|test| test.id == id) {
                    save_node_coverage_records(runner, &test.name, records);
                }
            }
            NodeEvent::Trace { id, records } => {
                if let Some(test) = tests.iter().find(|test| test.id == id) {
                    save_node_trace_records(runner, &test.name, records)?;
                }
            }
            NodeEvent::Treasury { records, .. } => {
                save_node_treasury_records(runner, records)?;
            }
            NodeEvent::TestStarted { id } => {
                if let Some(test) = tests.iter().find(|test| test.id == id) {
                    let report = base_report(test, &file, source_map.clone());
                    runner.reporter_manager.on_test_started(&report)?;
                }
            }
            NodeEvent::TestFinished {
                assertion,
                duration_ms,
                id,
                message,
                stack,
                status,
            } => {
                let Some(test) = tests.iter().find(|test| test.id == id) else {
                    continue;
                };
                let mut report = base_report(test, &file, source_map.clone());
                if runner.config.save_test_trace.is_some() {
                    report.trace_path = Some(super::trace::trace_file_name(&test.name));
                }
                report.duration = Duration::from_secs_f64(duration_ms / 1000.0);

                match status {
                    NodeStatus::Passed => {
                        report.status = TestStatus::Passed;
                        stats.passed += 1;
                    }
                    NodeStatus::Failed => {
                        report.status = TestStatus::Failed;
                        stats.failed += 1;
                        fill_failed_report(
                            runner,
                            &mut report,
                            assertion,
                            message,
                            stack,
                            raw_stdout.clone(),
                            raw_stderr.clone(),
                        )?;
                    }
                }
                dump_node_trace_if_available(runner, test)?;
                runner.reporter_manager.on_test_finished(&report)?;
            }
            NodeEvent::Fatal { message, stack } => {
                anyhow::bail!(
                    "Cannot run node tests in {}: {}\n{}",
                    file.display(),
                    message,
                    stack.unwrap_or_default()
                );
            }
        }
    }

    let suite_stats = TestSuiteStats {
        duration: Duration::default(),
        failed: stats.failed,
        passed: stats.passed,
        skipped: stats.skipped,
        todo: stats.todo,
        total: stats.passed + stats.failed + stats.skipped + stats.todo,
    };
    runner
        .reporter_manager
        .on_suite_finished(&file, &suite_stats)?;

    Ok(stats)
}

fn base_report(test: &TestDescriptor, file: &Path, source_map: Arc<SourceMap>) -> TestReport {
    TestReport {
        abi: None,
        backtrace: None,
        column: test.pos.column,
        detailed_message: None,
        details: None,
        duration: Duration::default(),
        execution: None,
        failed_transaction_context: None,
        failed_transactions: None,
        file_path: file.to_path_buf(),
        gas_limit: None,
        location: None,
        message: None,
        name: test.name.clone(),
        row: test.pos.row,
        show_bodies: false,
        source_map,
        status: TestStatus::Passed,
        suite_name: extract_suite_name(file),
        trace_path: None,
    }
}

fn fill_failed_report(
    runner: &TestRunner<'_>,
    report: &mut TestReport,
    assertion: Option<NodeAssertion>,
    message: Option<String>,
    stack: Option<String>,
    stdout: String,
    stderr: String,
) -> anyhow::Result<()> {
    let context = empty_failure_context(runner);

    if let Some(NodeAssertion::Transaction(assertion)) = assertion {
        let failure =
            transaction_assert_failure(assertion, &report.file_path, report.row, report.column)?;
        let formatter = formatter_for_context(runner, &context);
        report.message = failure.message();
        report.details = failure.location().map(|l| l.format_full());
        report.location = failure.location();
        let detailed = formatter.format_detailed_assert_failure(&failure);
        report.detailed_message = Some(FormatterContext::strip_ansi_text(&detailed));
        if let AssertFailure::TransactionNotFound(tx_failure) = &failure {
            report.failed_transactions = Some(formatter.parse_failed_transactions(&tx_failure.txs));
            report.failed_transaction_context =
                Some(formatter.get_failed_transaction_context(tx_failure));
        }
        report.execution = Some(TestExecutionContext {
            assert_failure: Some(failure),
            expected_exit_code: 0,
            failure: Some(context),
            fuzz: None,
            gas_used: 0,
            stderr,
            stdout,
            vm_log: None,
        });
        return Ok(());
    }

    report.message = message.or_else(|| Some("Node test failed".to_string()));
    report.detailed_message = stack;
    report.execution = Some(TestExecutionContext {
        assert_failure: None,
        expected_exit_code: 0,
        failure: None,
        fuzz: None,
        gas_used: 0,
        stderr,
        stdout,
        vm_log: None,
    });
    Ok(())
}

fn empty_failure_context(runner: &TestRunner<'_>) -> TestFailureExecutionContext {
    TestFailureExecutionContext {
        accounts: FxHashMap::<StdAddr, ShardAccount>::default(),
        available_wallets: runner
            .acton_config
            .wallets
            .as_ref()
            .map(|wallets| wallets.wallets.keys().cloned().collect())
            .unwrap_or_default(),
        build_cache: runner.build_cache.clone(),
        emulations: runner.emulations.clone(),
        fork_net: runner.config.fork_net.clone(),
        get_result: GetMethodResult::Success(GetMethodResultSuccess {
            code: Arc::from(""),
            gas_used: "0".to_string(),
            missing_library: None,
            stack: Arc::from(""),
            success: true,
            vm_exit_code: 0,
            vm_log: Arc::from(""),
        }),
        has_wallets_config: runner.acton_config.wallets.is_some(),
        known_addresses: runner.known_addresses.clone(),
        known_code_cells: runner.known_code_cells.clone(),
        network: runner.config.fork_net.clone(),
    }
}

fn ensure_node_contract_metadata(runner: &mut TestRunner<'_>) -> anyhow::Result<()> {
    let project_root = configured_project_root().to_path_buf();
    super::coverage::compile_project_contracts_for_coverage(
        &mut runner.build_cache,
        runner.file_build_cache,
        &runner.acton_config,
        &project_root,
    )
}

fn save_node_coverage_records(
    runner: &mut TestRunner<'_>,
    test_name: &str,
    records: Vec<NodeCoverageRecord>,
) {
    for record in records {
        if record.vm_log.is_empty() {
            continue;
        }

        runner.emulations.save_get_method(
            test_name,
            GetMethodResultSuccess {
                code: Arc::from(record.code),
                gas_used: "0".to_string(),
                missing_library: None,
                stack: Arc::from(""),
                success: true,
                vm_exit_code: 0,
                vm_log: Arc::from(record.vm_log),
            },
        );
    }
}

fn save_node_trace_records(
    runner: &mut TestRunner<'_>,
    test_name: &str,
    records: Vec<NodeTraceRecord>,
) -> anyhow::Result<()> {
    for record in records {
        runner.emulations.save_message(
            test_name,
            vec![SendMessageResult::Success(
                node_trace_record_to_send_result(record)?,
            )],
        );
    }

    Ok(())
}

fn node_trace_record_to_send_result(
    record: NodeTraceRecord,
) -> anyhow::Result<SendMessageResultSuccess> {
    let tx_cell = Boc::decode_base64(&record.raw_transaction)
        .with_context(|| "Failed to decode node trace transaction BoC")?;
    let transaction = tx_cell
        .parse::<Transaction>()
        .with_context(|| "Failed to parse node trace transaction")?;
    let shard_account_before = parse_node_trace_shard_account(&record.shard_account_before)?;
    let shard_account = parse_node_trace_shard_account(&record.shard_account)?;
    let code = record.code.as_deref().map(Boc::decode_base64).transpose()?;

    Ok(SendMessageResultSuccess {
        actions: record.actions.map(Arc::from),
        child_transactions: Vec::new(),
        code,
        executor_logs: Arc::from(record.executor_logs.unwrap_or_default()),
        externals: Vec::new(),
        missing_libraries: FxHashSet::default(),
        parent_transaction: None,
        raw_transaction: Arc::from(record.raw_transaction),
        shard_account,
        shard_account_before,
        transaction,
        vm_log: Arc::from(record.vm_log),
    })
}

fn parse_node_trace_shard_account(raw: &str) -> anyhow::Result<ShardAccount> {
    let cell =
        Boc::decode_base64(raw).with_context(|| "Failed to decode node trace shard account BoC")?;
    cell.parse::<ShardAccount>()
        .with_context(|| "Failed to parse node trace shard account")
}

fn save_node_treasury_records(
    runner: &mut TestRunner<'_>,
    records: Vec<NodeTreasuryRecord>,
) -> anyhow::Result<()> {
    for record in records {
        let (address, _) = StdAddr::from_str_ext(&record.address, StdAddrFormat::any())
            .with_context(|| {
                format!(
                    "Invalid treasury address from node test: {}",
                    record.address
                )
            })?;
        runner
            .known_addresses
            .addresses
            .insert(address, KnownAddress { name: record.name });
    }

    Ok(())
}

fn dump_node_trace_if_available(
    runner: &TestRunner<'_>,
    test: &TestDescriptor,
) -> anyhow::Result<()> {
    let Some(trace_dir) = &runner.config.save_test_trace else {
        return Ok(());
    };

    let Some(emulations) = runner.emulations.results_of(&test.name) else {
        eprintln!(
            "Warning: trace export is enabled for test '{}', but no emulated transactions were recorded; {} will not be written to {}",
            test.name,
            super::trace::trace_file_name(&test.name),
            trace_dir,
        );
        return Ok(());
    };

    super::trace::dump_test_transactions(
        test,
        &runner.build_cache,
        &runner.known_addresses,
        emulations,
        trace_dir,
    )
}

fn formatter_for_context<'a>(
    runner: &'a TestRunner<'_>,
    context: &'a TestFailureExecutionContext,
) -> FormatterContext<'a> {
    FormatterContext {
        accounts: Cow::Borrowed(&context.accounts),
        available_wallets: context.available_wallets.clone(),
        backtrace: runner.config.backtrace,
        build_cache: Cow::Borrowed(&context.build_cache),
        emulations: Cow::Borrowed(&context.emulations),
        fork_net: context.fork_net.clone(),
        has_wallets_config: context.has_wallets_config,
        known_addresses: Cow::Borrowed(&context.known_addresses),
        known_code_cells: Cow::Borrowed(&context.known_code_cells),
        network: context.network.clone(),
        show_bodies: runner.config.show_bodies,
    }
}

fn transaction_assert_failure(
    assertion: NodeTransactionAssertion,
    file: &Path,
    fallback_row: usize,
    fallback_column: usize,
) -> anyhow::Result<AssertFailure> {
    let (txs, parsed_txs) = send_result_tuples_from_raw_transactions(&assertion.transactions)?;
    let location = node_assertion_location(&assertion, file, fallback_row, fallback_column);
    let message = transaction_assertion_message(&assertion);
    let params = transaction_params(assertion.match_)?;
    Ok(AssertFailure::TransactionNotFound(
        TransactionGenericAssertFailure {
            location,
            message: Some(message),
            params,
            parsed_txs,
            txs,
        },
    ))
}

fn node_assertion_location(
    assertion: &NodeTransactionAssertion,
    file: &Path,
    fallback_row: usize,
    fallback_column: usize,
) -> Option<SourceLocation> {
    let line = assertion.row.unwrap_or(fallback_row) as i64;
    let column = assertion.column.unwrap_or(fallback_column) as i64;
    Some(SourceLocation {
        column,
        end_column: column,
        end_line: line,
        file: file.to_string_lossy().to_string(),
        length: 0,
        line,
    })
}

fn transaction_assertion_message(assertion: &NodeTransactionAssertion) -> String {
    match assertion.label.as_str() {
        "failed transaction" => "expect(<actual>).toHaveFailedTx(<expected>)",
        "successful deploy transaction" => "expect(<actual>).toHaveSuccessfulDeploy(<expected>)",
        "successful transaction" => "expect(<actual>).toHaveSuccessfulTx(<expected>)",
        _ => assertion.message.as_str(),
    }
    .to_string()
}

fn send_result_tuples_from_raw_transactions(
    transactions: &[String],
) -> anyhow::Result<(Vec<TupleItem>, Vec<Transaction>)> {
    let mut txs = Vec::new();
    let mut parsed_txs = Vec::new();

    for raw in transactions {
        let cell = Boc::decode_base64(raw)
            .with_context(|| "Failed to decode transaction BoC from node assertion")?;
        let parsed = cell
            .parse::<Transaction>()
            .with_context(|| "Failed to parse transaction from node assertion")?;
        txs.push(send_result_tuple(cell, &parsed));
        parsed_txs.push(parsed);
    }

    Ok((txs, parsed_txs))
}

fn send_result_tuple(tx_cell: Cell, tx: &Transaction) -> TupleItem {
    TupleItem::Tuple(Tuple(vec![
        TupleItem::Cell(tx_cell),
        TupleItem::Int(BigInt::from(tx.lt)),
        TupleItem::Tuple(Tuple(Vec::new())),
        TupleItem::Null,
        TupleItem::Cell(Cell::default()),
        TupleItem::Tuple(Tuple(Vec::new())),
        TupleItem::Int(BigInt::ZERO),
        TupleItem::Tuple(Tuple(Vec::new())),
    ]))
}

fn transaction_params(match_: NodeTransactionMatch) -> anyhow::Result<TransactionNotFoundParams> {
    Ok(TransactionNotFoundParams {
        aborted: None,
        action_exit_code: None,
        body: None,
        bounce: None,
        bounced: None,
        compute_phase_skipped: None,
        deploy: match_.deploy.map(DisplayParam::Value),
        exit_code: match_.exit_code.map(DisplayParam::Value),
        from: parse_optional_address(match_.from)?,
        opcode: None,
        send_mode: None,
        state_init: None,
        success: match_.success.map(DisplayParam::Value),
        to: parse_optional_address(match_.to)?,
        value: None,
    })
}

fn parse_optional_address(
    address: Option<String>,
) -> anyhow::Result<Option<DisplayParam<IntAddr>>> {
    let Some(address) = address else {
        return Ok(None);
    };
    let (address, _) = StdAddr::from_str_ext(&address, StdAddrFormat::any())
        .with_context(|| format!("Invalid address from node assertion: {address}"))?;
    Ok(Some(DisplayParam::Value(IntAddr::Std(address))))
}

fn resolve_node_test_files(path: &str) -> anyhow::Result<Vec<PathBuf>> {
    if !fs::exists(path).unwrap_or(false) {
        anyhow::bail!(error_fmt::file_not_found(path));
    }

    let metadata = fs::metadata(path).with_context(|| format!("Cannot access '{path}'"))?;
    if metadata.is_file() {
        if !path.ends_with(".test.ts") {
            anyhow::bail!("Node test file must end with {}", ".test.ts".yellow());
        }
        return Ok(vec![
            dunce::canonicalize(path).unwrap_or_else(|_| PathBuf::from(path)),
        ]);
    }

    if !metadata.is_dir() {
        anyhow::bail!("Path '{path}' is neither a file nor a directory");
    }

    let mut files = Vec::new();
    for entry in WalkDir::new(path).follow_links(false) {
        let entry = entry?;
        if entry.file_type().is_file() && entry.path().to_string_lossy().ends_with(".test.ts") {
            files.push(
                dunce::canonicalize(entry.path()).unwrap_or_else(|_| entry.path().to_path_buf()),
            );
        }
    }
    files.sort_unstable();
    Ok(files)
}

pub(super) fn resolve_node_worker(project_root: &Path) -> anyhow::Result<PathBuf> {
    let package_root = project_root.join("node_modules").join("@ton").join("acton");
    let candidates = [
        package_root.join("src").join("test-worker.ts"),
        package_root.join("dist").join("test-worker.js"),
    ];

    candidates
        .into_iter()
        .find(|path| path.exists())
        .ok_or_else(|| {
            anyhow!(
                "Cannot find @ton/acton node test worker. Run `bun install` in the project first."
            )
        })
}

fn test_files_as_strings(test_files: &[PathBuf]) -> Vec<String> {
    test_files
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect()
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum NodeEvent {
    #[serde(rename = "coverage")]
    Coverage {
        id: i32,
        records: Vec<NodeCoverageRecord>,
    },
    #[serde(rename = "trace")]
    Trace {
        id: i32,
        records: Vec<NodeTraceRecord>,
    },
    #[serde(rename = "treasury")]
    Treasury {
        #[allow(dead_code)]
        id: i32,
        records: Vec<NodeTreasuryRecord>,
    },
    #[serde(rename = "fatal")]
    Fatal {
        message: String,
        stack: Option<String>,
    },
    #[serde(rename = "testFinished")]
    TestFinished {
        assertion: Option<NodeAssertion>,
        #[serde(rename = "durationMs")]
        duration_ms: f64,
        id: i32,
        message: Option<String>,
        stack: Option<String>,
        status: NodeStatus,
    },
    #[serde(rename = "testStarted")]
    TestStarted { id: i32 },
    #[serde(rename = "tests")]
    Tests {
        #[allow(dead_code)]
        file: String,
        tests: Vec<NodeTest>,
    },
}

#[derive(Debug, Deserialize)]
struct NodeTest {
    column: usize,
    id: i32,
    name: String,
    row: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NodeCoverageRecord {
    code: String,
    vm_log: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NodeTraceRecord {
    actions: Option<String>,
    code: Option<String>,
    executor_logs: Option<String>,
    raw_transaction: String,
    shard_account: String,
    shard_account_before: String,
    vm_log: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NodeTreasuryRecord {
    address: String,
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum NodeStatus {
    Failed,
    Passed,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum NodeAssertion {
    #[serde(rename = "transaction")]
    Transaction(NodeTransactionAssertion),
}

#[derive(Debug, Deserialize)]
struct NodeTransactionAssertion {
    label: String,
    column: Option<usize>,
    #[serde(rename = "match")]
    match_: NodeTransactionMatch,
    message: String,
    row: Option<usize>,
    transactions: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NodeTransactionMatch {
    deploy: Option<bool>,
    exit_code: Option<u32>,
    from: Option<String>,
    success: Option<bool>,
    to: Option<String>,
}
