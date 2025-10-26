use crate::context::{
    AnyExecutor, AssertFailure, BuildCache, Context, DebugContext, KnownAddress, KnownAddresses,
    TransactionGenericAssertFailure,
};
use crate::{asserts_exts, exts, io_exts};
use abi::{ContractAbi, contract_abi};
use anyhow::anyhow;
use crossbeam_channel::unbounded;
use dap::events::Event;
use dap::prelude::{Request, Response};
use emulator::blockchain::Blockchain;
use emulator::emulator::Emulator;
use emulator::exit_codes;
use emulator::get_executor::{GetExecutor, GetMethodParams, GetMethodResult};
use emulator::step_get_executor::StepGetExecutor;
use emulator::tuple::stack::{Tuple, TupleItem, format_item_with_type};
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use owo_colors::OwoColorize;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use teamcity::TeamcityReporter;
use tolkc::source_map::SourceMap;
use tonlib_core::TonAddress;
use tonlib_core::cell::{ArcCell, Cell, CellBuilder};
use tonlib_core::tlb_types::tlb::TLB;
use tree_sitter::Node;
use tycho_types::boc::Boc;
use tycho_types::cell::Load;
use tycho_types::models::{AccountState, IntAddr, MsgInfo, ShardAccount, Transaction};

mod teamcity;

const CRC16: crc::Crc<u16> = crc::Crc::<u16>::new(&crc::CRC_16_XMODEM);

#[derive(Debug)]
struct CustomAnnotationValues {
    annotations: Vec<String>,
    expected_exit_code: Option<i32>,
    gas_limit: Option<u64>,
    todo_description: Option<String>,
}

pub fn test_cmd(path: &String, filter: Option<&str>, teamcity: bool) -> Result<(), anyhow::Error> {
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
        let result = run_tests_for_file(&file, filter, teamcity);
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

    Ok(())
}

fn find_test_files_recursively(dir_path: &str) -> Result<Vec<String>, anyhow::Error> {
    let mut test_files = Vec::new();

    fn visit_dir(dir: &Path, test_files: &mut Vec<String>) -> Result<(), anyhow::Error> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
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

    let compilation_result = tolkc::compile_fast(Path::new(&tmp_test_filename));
    let result = match compilation_result {
        tolkc::CompilerResult::Success(result) => {
            let _ = fs::remove_file(&tmp_test_filename);

            let code_cell = ArcCell::from_boc_b64(&*result.code_boc64)?;
            let data_cell = ArcCell::default();

            let stats = run_all_tests(file, tests, &code_cell, &data_cell, &abi, filter, teamcity);
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
    filter: Option<&str>,
    teamcity: bool,
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

            match &result.get_result {
                GetMethodResult::Success(result) => {
                    let exit_code = result.vm_exit_code as i64;

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
                            let diff_output = format_tuple_diff(
                                &assert_failure.left,
                                &assert_failure.right,
                                &assert_failure.left_type,
                                &assert_failure.right_type,
                                &abi,
                                &accounts,
                                &build_cache,
                                &known_addresses,
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
                            let value = format_tuple_value(
                                &assert_failure.left,
                                &assert_failure.left_type,
                                &accounts,
                                &abi,
                                &build_cache,
                                &known_addresses,
                                8,
                            );
                            println!("         {}", value.dimmed());
                        }

                        if let AssertFailure::Bin(assert_failure) = &assert_failure
                            && assert_failure.is_ord()
                        {
                            let left = format_tuple_value(
                                &assert_failure.left,
                                &assert_failure.left_type,
                                &accounts,
                                &abi,
                                &build_cache,
                                &known_addresses,
                                8,
                            );

                            let right = format_tuple_value(
                                &assert_failure.left,
                                &assert_failure.left_type,
                                &accounts,
                                &abi,
                                &build_cache,
                                &known_addresses,
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
                                assert_failure.txs,
                                format_address(
                                    &accounts,
                                    &build_cache,
                                    &assert_failure.txs,
                                    &assert_failure.params.from
                                ),
                                format_address(
                                    &accounts,
                                    &build_cache,
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
                                assert_failure.txs,
                                format_address(
                                    &accounts,
                                    &build_cache,
                                    &assert_failure.txs,
                                    &assert_failure.params.from
                                ),
                                format_address(
                                    &accounts,
                                    &build_cache,
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

                            if let Some(info) = exit_codes::get_exit_code_info(exit_code) {
                                println!("      {} {}", "├─".dimmed(), info.description.dimmed());
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
                    &abi,
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
) -> TestResult {
    // thread::sleep(Duration::from_secs(2));

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

    let (req_sender, req_receiver) = unbounded::<Request>();
    let (response_sender, response_receiver) = unbounded::<Response>();
    let (event_sender, event_receiver) = unbounded::<Event>();

    let debug_get_executor = StepGetExecutor::new(params.clone());
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
        dbg_ctx: &mut DebugContext {
            executors: vec![AnyExecutor::Get(debug_get_executor)],
            current_executor_id: 0,
            marks: vec![Default::default()],
            source_maps: vec![SourceMap {
                version: "".to_string(),
                language: None,
                compiler_version: None,
                files: vec![],
                globals: vec![],
                locations: vec![],
            }],
            locations: vec![],
            pseudo_step: 0,
            response_sender,
            event_sender,
            req_receiver,
        },
    };

    exts::register_get_extensions(&mut get_executor, &mut ctx);
    io_exts::register_get_extensions(&mut get_executor, &mut ctx);
    asserts_exts::register_get_extensions(&mut get_executor, &mut ctx);

    let result = get_executor.run_get_method(Default::default(), params);
    TestResult {
        get_result: result,
        captured_stdout: ctx.stdout_buffer,
        captured_stderr: ctx.stderr_buffer,
        assert_failure: (*ctx.assert_failure).clone(),
        expected_exit_code: ctx
            .expected_exit_code
            .clone()
            .map(|value| value.to_i32())
            .unwrap_or(None),
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
struct TestAnnotations {
    pub annotations: Vec<String>,
    pub expected_exit_code: Option<i32>,
    pub gas_limit: Option<u64>,
    pub todo_description: Option<String>,
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

                if name.starts_with("test") {
                    let id = (CRC16.checksum(name.as_bytes()) & 0xff_ff) as i32 | 0x1_00_00;
                    let test_annotations = find_test_annotations(content, child);

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

fn parse_annotation_object(content: &String, object_node: Node) -> CustomAnnotationValues {
    let Some(arguments) = object_node.child_by_field_name("arguments") else {
        return CustomAnnotationValues {
            annotations: Vec::new(),
            expected_exit_code: None,
            gas_limit: None,
            todo_description: None,
        };
    };

    let mut annotations = Vec::new();
    let mut expected_exit_code = None;
    let mut gas_limit = None;
    let mut todo_description = None;

    let mut cursor = arguments.walk();

    for field in arguments.children(&mut cursor) {
        if field.kind() == "instance_argument" {
            let Some(name_node) = field.child_by_field_name("name") else {
                continue;
            };

            let field_name = name_node.utf8_text(content.as_bytes()).unwrap_or("");

            match field_name {
                "skip" => {
                    let is_true = field
                        .child_by_field_name("value")
                        .map(|value| is_boolean_true(content, value))
                        .unwrap_or(true); // @custom({ skip }) -> true

                    if is_true {
                        annotations.push("skip".to_string());
                    }
                    continue;
                }
                "todo" => {
                    if let Some(value_node) = field.child_by_field_name("value") {
                        if let Some(description) = parse_string_literal(content, value_node) {
                            annotations.push("todo".to_string());
                            todo_description = Some(description);
                        } else if value_node.kind() == "boolean_literal"
                            && is_boolean_true(content, value_node)
                        {
                            annotations.push("todo".to_string());
                            todo_description = Some("TODO".to_string());
                        }
                    }
                    continue;
                }
                _ => {}
            }

            if let Some(value_node) = field.child_by_field_name("value") {
                match field_name {
                    "fail_with" => {
                        if let Some(number) = parse_number_literal(content, value_node) {
                            if let Ok(code) = number.parse::<i32>() {
                                expected_exit_code = Some(code);
                            }
                        }
                    }
                    "gas_limit" => {
                        if let Some(number) = parse_number_literal(content, value_node) {
                            if let Ok(limit) = number.parse::<u64>() {
                                gas_limit = Some(limit);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    CustomAnnotationValues {
        annotations,
        expected_exit_code,
        gas_limit,
        todo_description,
    }
}

fn is_boolean_true(content: &String, node: Node) -> bool {
    if node.kind() == "boolean_literal" {
        let text = node.utf8_text(content.as_bytes()).unwrap_or("");
        text == "true"
    } else {
        false
    }
}

fn parse_number_literal(content: &String, node: Node) -> Option<String> {
    if node.kind() == "number_literal" {
        let text = node.utf8_text(content.as_bytes()).unwrap_or("");
        Some(text.to_string())
    } else {
        None
    }
}

fn parse_string_literal(content: &String, node: Node) -> Option<String> {
    if node.kind() == "string_literal" {
        let text = node.utf8_text(content.as_bytes()).unwrap_or("");
        let unquoted = text.trim_matches('"');
        Some(unquoted.to_string())
    } else {
        None
    }
}

fn find_test_annotations(content: &String, child: Node) -> TestAnnotations {
    let mut annotations = Vec::new();
    let mut expected_exit_code = None;
    let mut gas_limit = None;
    let mut todo_description = None;
    let Some(annotations_node) = child.child_by_field_name("annotations") else {
        return TestAnnotations {
            annotations,
            expected_exit_code,
            gas_limit,
            todo_description,
        };
    };

    let mut cursor = annotations_node.walk();
    for annotation in annotations_node.children(&mut cursor) {
        if annotation.kind() != "annotation" {
            continue;
        }

        if let Some(name_node) = annotation.child_by_field_name("name") {
            let annotation_name = name_node.utf8_text(content.as_bytes()).unwrap_or("");
            if annotation_name != "custom" {
                continue;
            }
            let Some(args_node) = annotation.child_by_field_name("arguments") else {
                continue;
            };

            let mut arg_cursor = args_node.walk();

            for child in args_node.children(&mut arg_cursor) {
                match child.kind() {
                    "string_literal" => {
                        let text = child.utf8_text(content.as_bytes()).unwrap_or("");
                        let unquoted = text.trim_matches('"');
                        match unquoted {
                            "skip" => {
                                annotations.push("skip".to_string());
                            }
                            "todo" => {
                                annotations.push("todo".to_string());
                            }
                            _ => {}
                        }
                    }
                    "object_literal" => {
                        let values = parse_annotation_object(content, child);

                        annotations.extend(values.annotations);
                        if values.expected_exit_code.is_some() {
                            expected_exit_code = values.expected_exit_code;
                        }
                        if values.gas_limit.is_some() {
                            gas_limit = values.gas_limit;
                        }
                        if values.todo_description.is_some() {
                            todo_description = values.todo_description;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    TestAnnotations {
        annotations,
        expected_exit_code,
        gas_limit,
        todo_description,
    }
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

fn format_tuple_diff(
    left: &Tuple,
    right: &Tuple,
    left_type: &str,
    right_type: &str,
    abi: &ContractAbi,
    accounts: &HashMap<String, ShardAccount>,
    build_cache: &BuildCache,
    known_addresses: &KnownAddresses,
) -> String {
    let left_type_str = left_type.to_string();
    let left_item = TupleItem::TypedTuple {
        abi: abi.find_type(&left_type_str),
        contract_abi: abi.clone(),
        type_name: left_type_str,
        items: (**left).clone(),
        accounts: accounts.clone(),
        build_cache: build_cache.to_tuple_build_cache(),
        known_addresses: known_addresses.to_tuple_known_addresses(),
    };
    let right_type_str = right_type.to_string();
    let right_item = TupleItem::TypedTuple {
        abi: abi.find_type(&right_type_str),
        contract_abi: abi.clone(),
        type_name: right_type_str,
        items: (**right).clone(),
        accounts: accounts.clone(),
        build_cache: build_cache.to_tuple_build_cache(),
        known_addresses: known_addresses.to_tuple_known_addresses(),
    };

    format_tuple_item_diff(&left_item, &right_item)
}

fn highlight_actual_expected(message: &str) -> String {
    let result = message
        .replace("actual", &"actual".red().to_string())
        .replace("expected", &"expected".green().to_string());

    result.to_string()
}

fn add_indent_to_lines(text: &str, indent: usize) -> String {
    let indent_str = " ".repeat(indent);
    text.lines()
        .map(|line| format!("{}{}", indent_str, line))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_tuple_value(
    tuple: &Tuple,
    type_name: &String,
    accounts: &HashMap<String, ShardAccount>,
    abi: &ContractAbi,
    build_cache: &BuildCache,
    known_addresses: &KnownAddresses,
    indent: usize,
) -> String {
    let item = TupleItem::TypedTuple {
        abi: abi.find_type(type_name),
        contract_abi: abi.clone(),
        type_name: type_name.to_string(),
        items: (**tuple).clone(),
        accounts: accounts.clone(),
        build_cache: build_cache.to_tuple_build_cache(),
        known_addresses: known_addresses.to_tuple_known_addresses(),
    };
    let raw_str = format!("{}", item);

    if !raw_str.contains("\n") {
        return raw_str;
    }

    let lines: Vec<_> = raw_str.lines().collect();
    let mut result = lines[0].to_string() + "\n";
    result += &add_indent_to_lines(&lines[1..].join("\n"), indent);
    result
}

fn format_tuple_item_diff(left: &TupleItem, right: &TupleItem) -> String {
    let (
        TupleItem::TypedTuple {
            type_name: left_type,
            items: left_items,
            abi,
            contract_abi,
            accounts,
            build_cache,
            ..
        },
        TupleItem::TypedTuple {
            type_name: right_type,
            items: right_items,
            ..
        },
    ) = (left, right)
    else {
        return format!("{} != {}", left.red(), right.green());
    };

    if left_type != right_type {
        return format!("{} != {}", left, right);
    }

    if let Some(struct_desc) = abi {
        if left_items.len() == struct_desc.fields.len() {
            let mut result = format!("{} {{\n", left_type);

            for (field, (left_item, right_item)) in struct_desc
                .fields
                .iter()
                .zip(left_items.iter().zip(right_items.iter()))
            {
                if left_item != right_item {
                    result.push_str(&format!(
                        "    {}: {}\n",
                        field.name.yellow(),
                        format_item_with_type(left_item, &field.type_info.human_readable).red()
                    ));
                    result.push_str(&format!(
                        "    {:<width$}  {}\n",
                        "",
                        format_item_with_type(right_item, &field.type_info.human_readable).green(),
                        width = field.name.len()
                    ));
                } else {
                    result.push_str(&format!(
                        "    {}{} {}\n",
                        field.name.dimmed(),
                        ":".dimmed(),
                        format_item_with_type(left_item, &field.type_info.human_readable).dimmed()
                    ));
                }
            }

            result.push_str("}");
            result
        } else {
            format!("{} != {}", left, right)
        }
    } else {
        let mut result = "(\n".to_string();
        let max_len = left_items.len().max(right_items.len());

        for i in 0..max_len {
            let left_val = left_items.get(i);
            let right_val = right_items.get(i);

            match (left_val, right_val) {
                (Some(left_val), Some(right_val)) => {
                    if left_val != right_val {
                        result.push_str(&format!("    {},\n", left_val.red()));
                        result.push_str(&format!("    {}\n", right_val.green()));
                    } else {
                        result.push_str(&format!("    {},\n", left_val.dimmed()));
                    }
                }
                (Some(left_val), None) => {
                    result.push_str(&format!("    {},\n", left_val.red()));
                }
                (None, Some(right_val)) => {
                    result.push_str(&format!("    {}\n", right_val.green()));
                }
                (None, None) => {}
            }
        }

        result.push_str(")");
        result
    }
}

fn format_addr_hash(addr: &IntAddr) -> String {
    let raw = addr.as_std().unwrap().display_base64(true).to_string();
    raw[..6].to_string() + ".." + &raw[raw.len() - 6..]
}

fn format_address(
    accounts: &HashMap<String, ShardAccount>,
    build_cache: &BuildCache,
    txs: &TupleItem,
    addr: &Option<IntAddr>,
) -> String {
    let Some(addr) = addr else {
        return "<any>".cyan().to_string();
    };

    let TupleItem::TypedTuple { items, .. } = txs else {
        return format_addr_hash(&addr);
    };

    let TupleItem::Tuple(items) = &items[0] else {
        return format!("{}", items[0]);
    };

    let txs = items
        .iter()
        .filter_map(|el| match el {
            TupleItem::Cell(cell) => Some(cell),
            _ => None,
        })
        .map(|x| {
            let result = x.to_boc_b64(false).unwrap();
            let tx_cell: tycho_types::cell::Cell = Boc::decode_base64(&result).unwrap();
            let mut tx_slice = tx_cell.as_slice().unwrap();
            Transaction::load_from(&mut tx_slice).unwrap()
        })
        .collect::<Vec<_>>();

    let mut known_contracts: Vec<IntAddr> = vec![];

    for tx in &txs {
        let in_msg = tx.load_in_msg().unwrap();
        if let Some(in_msg) = &in_msg
            && let MsgInfo::Int(info) = &in_msg.info
        {
            // It's O(N) but we need order, and we don't have many (thousands) transactions
            if !known_contracts.contains(&info.src) {
                known_contracts.push(info.src.clone());
            }
            if !known_contracts.contains(&info.dst) {
                known_contracts.push(info.dst.clone());
            }
        }
    }

    let mut contract_letters: HashMap<IntAddr, String> = HashMap::new();

    for (index, addr) in known_contracts.iter().enumerate() {
        let letter = char::from_u32('A' as u32 + index as u32)
            .unwrap_or_else(|| char::from_digit(index as u32, 10).unwrap());
        contract_letters.insert(addr.clone(), letter.to_string());
    }

    let mut builder = "".to_string();

    let contract_type = get_contract_type(accounts, build_cache, addr);

    let letter = contract_letters.get(&addr);
    if let Some(letter) = letter {
        builder += format!("{} {} ", contract_type.cyan(), letter.bold()).as_str();
    }

    builder += format_addr_hash(&addr).dimmed().to_string().as_str();

    builder
}

fn get_contract_type(
    accounts: &HashMap<String, ShardAccount>,
    build_cache: &BuildCache,
    addr: &IntAddr,
) -> String {
    let account = accounts.get(&addr.to_string());
    let Some(account) = account else {
        return "".to_string();
    };

    let account_data = account.load_account();
    let Ok(Some(data)) = account_data else {
        return "".to_string();
    };

    let AccountState::Active(info) = data.state else {
        return "".to_string();
    };

    let Some(code) = &info.code else {
        return "".to_string();
    };

    let compilation_result = build_cache.built.iter().find(|(_name, result)| {
        result.code_hash.to_ascii_lowercase() == code.repr_hash().to_string()
    });

    if let Some(result) = compilation_result {
        return result.1.name.clone();
    }

    "".to_string()
}
