use clap::{Parser, Subcommand};
use emulator::exit_codes;
use emulator::get_executor::{GetExecutor, GetMethodParams, GetMethodResult};
use emulator::tuple::stack::{Tuple, TupleItem};
use emulator_rs::context::{AssertFailure, Context};
use emulator_rs::{asserts_exts, exts, io_exts};
use owo_colors::OwoColorize;
use std::collections::HashMap;
use std::fs;
use std::ops::Add;
use std::path::Path;
use std::process;
use std::sync::Arc;
use std::time::Instant;
use tonlib_core::TonAddress;
use tonlib_core::cell::{ArcCell, Cell, CellBuilder};
use tonlib_core::tlb_types::tlb::TLB;
use tree_sitter::Node;

const CRC16: crc::Crc<u16> = crc::Crc::<u16>::new(&crc::CRC_16_XMODEM);

#[derive(Parser)]
#[command(name = "acton")]
#[command(about = "TON blockchain development tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Test { file: String },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Test { file } => {
            if !file.ends_with("_test.tolk") {
                eprintln!("File must end with __test.tolk");
                process::exit(1);
            }

            let content = match fs::read_to_string(&file) {
                Ok(content) => content,
                Err(err) => {
                    eprintln!("Error reading file '{}': {}", file, err);
                    process::exit(1);
                }
            };

            let tests = find_all_test(file.clone(), &content);

            let executable_code = inject_locations_into_expect_calls(&content, &file);
            let tmp_test_filename = "test_".to_string().add(&*file);

            fs::write(&tmp_test_filename, executable_code).unwrap();

            let compilation_result = tolkc::compile(Path::new(&tmp_test_filename));
            match compilation_result {
                tolkc::CompilerResult::Success(result) => {
                    let code_cell = ArcCell::from_boc_b64(&*result.code_boc64).unwrap();
                    let data_cell = ArcCell::default();
                    run_all_tests(&file, tests, &code_cell, &data_cell);
                }
                tolkc::CompilerResult::Error(error) => {
                    eprintln!("Cannot compile test file {}", error.message);
                    process::exit(1);
                }
            }
        }
    }
}

fn run_all_tests(
    file_path: &str,
    tests: Vec<TestDescriptor>,
    code_cell: &Arc<Cell>,
    data_cell: &Arc<Cell>,
) {
    let cwd = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());

    println!(
        "\n{} {}\n",
        " TEST ".bold().on_cyan(),
        cwd.display().dimmed()
    );
    println!("{}", "─".repeat(50).dimmed());

    let relative_path = Path::new(file_path)
        .strip_prefix(cwd)
        .unwrap_or_else(|_| Path::new(file_path));
    println!(
        " {} {} {}",
        ">".dimmed(),
        relative_path.display().to_string(),
        format!("({} tests)", tests.len()).dimmed()
    );

    let total_start_time = Instant::now();
    let dest_address = contract_address(&code_cell);

    let mut passed = 0;
    let mut failed = 0;
    let mut skipped = 0;

    for (_i, test) in tests.iter().enumerate() {
        if test.annotations.contains(&"skip".to_string()) {
            println!("  {} {} {}", "○".dimmed(), test.name, "skipped".dimmed());
            skipped += 1;
            continue;
        }

        let start_time = Instant::now();
        let result = execute_test(test, &code_cell, &data_cell, &dest_address);
        let duration = start_time.elapsed();
        let TestResult {
            captured_stdout,
            captured_stderr,
            assert_failure,
            ..
        } = result;

        let exit_code = match &result.get_result {
            GetMethodResult::Success(result) => result.vm_exit_code,
            GetMethodResult::Error(_) => 999,
        };

        let duration_ms = duration.as_millis();
        let (time_value, time_unit) = if duration_ms > 0 {
            (duration_ms.to_string(), "ms")
        } else {
            (duration.as_micros().to_string(), "μs")
        };

        if exit_code == 0 {
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

                    if let Some(assert_failure) = assert_failure
                        && exit_code == 567
                    {
                        let diff_output = format_tuple_diff(
                            &assert_failure.left,
                            &assert_failure.right,
                            &assert_failure.left_type,
                            &assert_failure.right_type,
                        );

                        if let Some(message) = &assert_failure.message {
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

                        for line in diff_output.lines() {
                            println!("        {}", line);
                        }

                        if let Some(location) = &assert_failure.location {
                            if !location.is_empty() {
                                println!("      {} at {}", "└─".dimmed(), location.dimmed());
                            }
                        }
                    } else {
                        println!(
                            "    {} exit_code={}",
                            "└─".dimmed(),
                            exit_code.to_string().yellow()
                        );

                        if let Some(info) = exit_codes::get_exit_code_info(exit_code) {
                            println!("      {} {}", "├─".dimmed(), info.description.dimmed());
                            println!("      {} Phase: {}", "└─".dimmed(), info.phase.dimmed());
                        }
                    }
                }
                GetMethodResult::Error(error) => {
                    println!("    {} {}", "└─".dimmed(), error.error.yellow());
                }
            }
        }

        if !captured_stdout.trim().is_empty() {
            println!("    {} Test output:", "└─".dimmed());
            for line in captured_stdout.trim().lines() {
                println!("       {}", line.dimmed());
            }
        }

        if !captured_stderr.trim().is_empty() {
            println!("    {} Test stderr:", "└─".dimmed());
            for line in captured_stderr.trim().lines() {
                println!("       {}", line.bright_red());
            }
        }
    }

    let total_duration = total_start_time.elapsed();
    let total_duration_ms = total_duration.as_millis();

    println!("{}", "─".repeat(50).dimmed());

    if failed == 0 && skipped == 0 {
        println!(
            " {} {} {} {} {}{}",
            "✓".green().bold(),
            passed.to_string().green().bold(),
            "passed".green().bold(),
            "in".dimmed(),
            total_duration_ms.to_string().green(),
            "ms".green().dimmed()
        );
    } else if failed == 0 && skipped > 0 {
        println!(
            " {} {} {}, {} {} {} {} {}{}",
            "✓".green().bold(),
            passed.to_string().green().bold(),
            "passed".green().bold(),
            "○".yellow().bold(),
            skipped.to_string().yellow().bold(),
            "skipped".yellow().bold(),
            "in".dimmed(),
            total_duration_ms.to_string().green(),
            "ms".green().dimmed()
        );
    } else {
        println!(
            " {} {} {}, {} {} {}, {} {} {} {} {}{}",
            "✓".green().bold(),
            passed.to_string().green().bold(),
            "passed".green().bold(),
            "✗".red().bold(),
            failed.to_string().red().bold(),
            "failed".red().bold(),
            "○".yellow().bold(),
            skipped.to_string().yellow().bold(),
            "skipped".yellow().bold(),
            "in".dimmed(),
            total_duration_ms.to_string().red(),
            "ms".red().dimmed()
        );
    }

    if failed > 0 {
        println!(
            "\n{}",
            "Some tests failed. Check the output above for details.".red()
        );
    }
}

struct TestResult {
    get_result: GetMethodResult,
    captured_stdout: String,
    captured_stderr: String,
    assert_failure: Option<AssertFailure>,
}

fn execute_test(
    test: &TestDescriptor,
    code_cell: &Arc<Cell>,
    data_cell: &Arc<Cell>,
    dest_address: &TonAddress,
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

    let mut ctx = Context {
        stdout_buffer: "".to_string(),
        stderr_buffer: "".to_string(),
        capture_test_output: true,
        assert_failure: &mut None,
    };

    exts::register_get_extensions(
        &mut get_executor,
        (&mut ctx) as *mut _ as *mut std::ffi::c_void,
    );
    io_exts::register_get_extensions(
        &mut get_executor,
        (&mut ctx) as *mut _ as *mut std::ffi::c_void,
    );
    asserts_exts::register_get_extensions(
        &mut get_executor,
        (&mut ctx) as *mut _ as *mut std::ffi::c_void,
    );

    let result = get_executor.run_get_method(Default::default(), params);
    TestResult {
        get_result: result,
        captured_stdout: ctx.stdout_buffer,
        captured_stderr: ctx.stderr_buffer,
        assert_failure: (*ctx.assert_failure).clone(),
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
}

fn find_all_test(file: String, content: &String) -> Vec<TestDescriptor> {
    let tree = tolk_parser::parser::parse(&content);
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

                    return vec![TestDescriptor {
                        file: file.clone(),
                        id,
                        name: name.to_string(),
                        annotations: find_test_annotations(content, child),
                    }];
                }
            };

            vec![]
        })
        .collect()
}

fn find_test_annotations(content: &String, child: Node) -> Vec<String> {
    let mut annotations = Vec::new();
    let Some(annotations_node) = child.child_by_field_name("annotations") else {
        return vec![];
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

            let args_text = args_node.utf8_text(content.as_bytes()).unwrap_or("");
            if args_text.contains("\"skip\"") {
                annotations.push("skip".to_string());
            }
        }
    }
    annotations
}

fn inject_locations_into_expect_calls(content: &str, file_path: &str) -> String {
    let tree = tolk_parser::parser::parse(content);
    let root_node = tree.root_node();

    abi::process_struct_definitions(&root_node, content, file_path);
    emulator::tuple::stack::set_struct_description_getter(abi::get_struct_description);

    let mut replacements = Vec::new();
    find_expect_calls(&root_node, content, file_path, &mut replacements);

    let mut result = content.to_string();

    for (start, end, replacement) in replacements.into_iter().rev() {
        result.replace_range(start..end, &replacement);
    }

    result + "\n\nfun main() {}"
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

    if node.kind() == "function_call" {
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

            if arg_count == 1 {
                let start = args_node.end_byte() - 1;
                let end = args_node.end_byte() - 1;

                let lines: Vec<&str> = content[..start].lines().collect();
                let line_number = lines.len();

                let location = format!(", \"{file_path}:{line_number}\"",);
                replacements.push((start, end, location));
            }
        }
    }
}

fn format_tuple_diff(left: &Tuple, right: &Tuple, left_type: &str, right_type: &str) -> String {
    let left_item = TupleItem::TypedTuple {
        type_name: left_type.to_string(),
        items: (**left).clone(),
    };
    let right_item = TupleItem::TypedTuple {
        type_name: right_type.to_string(),
        items: (**right).clone(),
    };

    format_tuple_item_diff(&left_item, &right_item)
}

fn highlight_actual_expected(message: &str) -> String {
    let result = message
        .replace("actual", &"actual".red().to_string())
        .replace("expected", &"expected".green().to_string());

    result.to_string()
}

fn format_tuple_item_diff(left: &TupleItem, right: &TupleItem) -> String {
    match (left, right) {
        (
            TupleItem::TypedTuple {
                type_name: left_type,
                items: left_items,
            },
            TupleItem::TypedTuple {
                type_name: right_type,
                items: right_items,
            },
        ) => {
            if left_type != right_type {
                return format!("{} != {}", left, right);
            }

            if let Some(struct_desc) = abi::get_struct_description(left_type) {
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
                                left_item.red()
                            ));
                            result.push_str(&format!(
                                "    {:<width$}  {}\n",
                                "",
                                right_item.green(),
                                width = field.name.len()
                            ));
                        } else {
                            result.push_str(&format!(
                                "    {}{} {}\n",
                                field.name.dimmed(),
                                ":".dimmed(),
                                left_item.dimmed()
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
        _ => {
            format!("{} != {}", left.red(), right.green())
        }
    }
}
