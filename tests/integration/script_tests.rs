use crate::support::TestOutputExt;
use crate::support::project::{Project, ProjectBuilder};
use crate::support::toncenter::{
    ToncenterV2MockResponse, append_custom_network, append_localnet_network,
    spawn_toncenter_v2_mock, spawn_toncenter_v2_mock_with_capture, toncenter_v2_error_response,
    toncenter_v2_seqno_ok_response,
};

use std::fs;
use std::thread;
use std::time::{Duration, Instant};
use ton_executor::DEFAULT_CONFIG_DICT;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder};

const DEPLOYER_MNEMONIC: &str = "cupboard match uphold miracle fog balance unknown region share hand trophy million toy narrow ability exchange first toast fresh maid report cram strong later";
const REMOTE_GLOBAL_VERSION: u32 = 777;
const REMOTE_GLOBAL_CAPABILITIES: u64 = 0x1234;

fn build_global_version_cell(version: u32, capabilities: u64) -> Cell {
    let mut builder = CellBuilder::new();
    builder
        .store_u8(0xc4)
        .expect("must store GlobalVersion tag");
    builder
        .store_u32(version)
        .expect("must store GlobalVersion version");
    builder
        .store_u64(capabilities)
        .expect("must store GlobalVersion capabilities");
    builder.build().expect("must build GlobalVersion cell")
}

fn mocked_config_boc64(version: u32, capabilities: u64) -> String {
    let mut config = DEFAULT_CONFIG_DICT.as_ref().clone();
    config
        .set(8, build_global_version_cell(version, capabilities))
        .expect("must update global version config param");
    let root = config
        .root()
        .clone()
        .expect("default blockchain config must have a root");
    Boc::encode_base64(root)
}

fn toncenter_v2_get_config_all_ok_response(config_boc64: &str) -> ToncenterV2MockResponse {
    ToncenterV2MockResponse {
        status: 200,
        body: serde_json::json!({
            "ok": true,
            "result": {
                "@type": "configInfo",
                "config": {
                    "@type": "tvm.cell",
                    "bytes": config_boc64
                },
                "@extra": "mocked-live-shape"
            }
        })
        .to_string(),
    }
}

fn build_broadcast_wallet_error_project(project_name: &str) -> Project {
    let project = ProjectBuilder::new(project_name)
        .script_file(
            "deploy",
            r#"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"
            import "../../lib/emulation/scripts"

            fun main() {
                val wallet = scripts.wallet("deployer");
                net.send(wallet.address, createMessage({
                    bounce: false,
                    value: ton("0.05"),
                    dest: address("EQBvDB_H7FFBs0nF4ap_DBdcOrwY_rMIpNVVOR6SWYFHByMJ"),
                }));
            }
        "#,
        )
        .build();

    fs::write(project.path().join("mnemonic.txt"), DEPLOYER_MNEMONIC)
        .expect("failed to write mnemonic");
    fs::write(
        project.path().join("wallets.toml"),
        r#"[wallets.deployer]
kind = "v4r2"
workchain = 0
keys = { mnemonic-file = "mnemonic.txt" }
"#,
    )
    .expect("failed to write wallets.toml");

    project
}

fn script_body_project(project_name: &str) -> ProjectBuilder {
    ProjectBuilder::new(project_name)
        .file(
            "contracts/script_body_messages",
            r"
struct (0xF8000001) ScriptBodyMsg {
    queryId: uint64
    recipient: address
    amount: coins
}
",
        )
        .contract(
            "script_body_sink",
            r#"
import "script_body_messages"

contract ScriptBodySink {
    incomingMessages: ScriptBodyMsg
}

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val _msg = lazy ScriptBodyMsg.fromSlice(in.body);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#,
        )
        .script_file(
            "print_txs",
            r#"
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/emulation/scripts"
import "../../lib/io"
import "../contracts/script_body_messages"

fun main() {
    val sender = testing.treasury("sender");
    val init = ContractState {
        code: build("script_body_sink"),
        data: createEmptyCell(),
    };
    val sinkAddress = AutoDeployAddress { stateInit: init }.calculateAddress();

    net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: init,
        },
    }));

    val txs = net.send(sender.address, createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: sinkAddress,
        body: ScriptBodyMsg {
            queryId: 11,
            recipient: sender.address,
            amount: ton("0.02"),
        },
    }));

    println(txs);
}
"#,
        )
}

fn write_localnet_wallet_config(project: &Project, wallet_name: &str) {
    fs::write(project.path().join("mnemonic.txt"), DEPLOYER_MNEMONIC)
        .expect("failed to write mnemonic");
    fs::write(
        project.path().join("wallets.toml"),
        format!(
            r#"[wallets.{wallet_name}]
kind = "v4r2"
workchain = 0
keys = {{ mnemonic-file = "mnemonic.txt" }}
"#
        ),
    )
    .expect("failed to write wallets.toml");
}

fn extract_marker_value(output: &str, marker: &str) -> String {
    output
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix(marker).map(ToOwned::to_owned))
        .unwrap_or_else(|| panic!("Marker `{marker}` not found in output:\n{output}"))
}

fn wait_until_address_state_active(
    node: &crate::support::localnet::LocalnetHandle,
    address: &str,
    timeout: Duration,
) {
    let query = format!("/api/v2/getAddressState?address={address}");
    let deadline = Instant::now() + timeout;
    loop {
        let response = node.get_json(&query);
        if response["ok"].as_bool() == Some(true) && response["result"].as_str() == Some("active") {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "Timed out waiting for address `{address}` to become active:\n{}",
            serde_json::to_string_pretty(&response).unwrap_or_default()
        );
        thread::sleep(Duration::from_millis(200));
    }
}

#[test]
fn test_script_simple_execution() {
    let project = ProjectBuilder::new("script-simple")
        .script_file(
            "hello",
            r#"
            import "../../lib/io"

            fun main() {
                println("Hello from script!");
            }
        "#,
        )
        .build();

    let output = project.acton().script("scripts/hello.tolk").run().code(0);

    output.assert_contains("Hello from script!");
}

#[test]
fn test_script_debug_logs_are_hidden_without_verbose_flag() {
    let project = ProjectBuilder::new("script-debug-logs-default-off")
        .script_file(
            "debug_logs",
            r"
            fun main() {
                debug.dumpStack();
            }
        ",
        )
        .build();

    project
        .acton()
        .script("scripts/debug_logs.tolk")
        .run()
        .success()
        .assert_not_contains("stack(0 values)")
        .assert_snapshot_matches(
            "integration/snapshots/test_script_debug_logs_are_hidden_without_verbose_flag.stdout.txt",
        );
}

#[test]
fn test_script_verbose_flag_is_accepted() {
    let project = ProjectBuilder::new("script-debug-logs-verbose")
        .script_file(
            "debug_logs",
            r"
            fun main() {
                debug.dumpStack();
            }
        ",
        )
        .build();

    project
        .acton()
        .script("scripts/debug_logs.tolk")
        .arg("--verbose")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_script_verbose_flag_is_accepted.stdout.txt",
        );
}

#[test]
fn test_script_rejects_verbose_level_above_one() {
    let project = ProjectBuilder::new("script-debug-logs-verbose-level")
        .script_file(
            "debug_logs",
            r"
            fun main() {
                debug.dumpStack();
            }
        ",
        )
        .build();

    project
        .acton()
        .script("scripts/debug_logs.tolk")
        .arg("-vv")
        .run()
        .failure()
        .assert_stderr_contains("Verbosity levels above 1 are not supported yet")
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_script_rejects_verbose_level_above_one.stderr.txt",
        );
}

#[test]
fn test_script_ensure_latest_uses_project_root_from_nested_directory() {
    let project = ProjectBuilder::new("script-ensure-latest-project-root")
        .script_file(
            "hello",
            r#"
            import "../../lib/io"

            fun main() {
                println("Hello from nested script!");
            }
        "#,
        )
        .build();

    let nested_dir = project.path().join("nested");
    fs::create_dir_all(&nested_dir).expect("Failed to create nested test directory");

    let root_stdlib = project.path().join(".acton/tolk-stdlib");
    let nested_stdlib = nested_dir.join(".acton/tolk-stdlib");
    let script_path = project.path().join("scripts/hello.tolk");
    assert!(
        !root_stdlib.exists(),
        "stdlib must not exist before script command"
    );
    assert!(
        !nested_stdlib.exists(),
        "stdlib must not exist in nested cwd before script command"
    );

    project
        .acton()
        .arg("--project-root")
        .arg("..")
        .script(script_path.to_string_lossy().as_ref())
        .current_dir(&nested_dir)
        .run()
        .success()
        .assert_contains("Hello from nested script!");

    assert!(
        root_stdlib.exists(),
        "stdlib should be installed in project root"
    );
    assert!(
        !nested_stdlib.exists(),
        "stdlib must not be installed in nested cwd"
    );
}

#[test]
fn test_script_with_calculations() {
    let project = ProjectBuilder::new("script-calc")
        .script_file(
            "calc",
            r#"
            import "../../lib/io"

            fun main() {
                val result = 2 + 2 * 2;
                println("Result: ");
                println(result);
            }
        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/calc.tolk")
        .run()
        .code(0)
        .assert_contains("Result:")
        .assert_contains("6");
}

#[test]
fn test_script_hides_transaction_bodies_without_show_bodies_flag() {
    let project = script_body_project("script-hides-transaction-bodies").build();

    project
        .acton()
        .script("scripts/print_txs.tolk")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_script_hides_transaction_bodies_without_show_bodies_flag.stdout.txt",
        );
}

#[test]
fn test_script_shows_transaction_bodies_with_show_bodies_flag() {
    let project = script_body_project("script-shows-transaction-bodies").build();

    project
        .acton()
        .script("scripts/print_txs.tolk")
        .show_bodies()
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_script_shows_transaction_bodies_with_show_bodies_flag.stdout.txt",
        );
}

#[test]
fn test_script_file_not_found() {
    let project = ProjectBuilder::new("script-not-found").build();

    project
        .acton()
        .script("scripts/nonexistent.tolk")
        .run()
        .failure()
        .assert_stderr_contains("Cannot find file or directory");
}

#[test]
fn test_script_not_a_file() {
    let project = ProjectBuilder::new("script-dir").build();

    fs::create_dir_all(project.path().join("scripts")).unwrap();

    project
        .acton()
        .script("scripts")
        .run()
        .failure()
        .assert_stderr_contains("is not a file");
}

#[test]
fn test_script_wrong_extension() {
    let project = ProjectBuilder::new("script-wrong-ext").build();

    fs::create_dir_all(project.path().join("scripts")).unwrap();
    fs::write(project.path().join("scripts/test.txt"), "some content").unwrap();

    project
        .acton()
        .script("scripts/test.txt")
        .run()
        .failure()
        .assert_stderr_contains("must end with .tolk");
}

#[test]
fn test_script_with_args() {
    let project = ProjectBuilder::new("script-args")
        .script_file(
            "args",
            r#"
            import "../../lib/io"

            fun main(a: int, b: int) {
                println("Arg A:");
                println(a);
                println("Arg B:");
                println(b);
                println("Sum:");
                println(a + b);
            }
        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/args.tolk")
        .arg("10")
        .arg("20")
        .run()
        .success()
        .assert_contains("Arg A:")
        .assert_contains("10")
        .assert_contains("Arg B:")
        .assert_contains("20")
        .assert_contains("Sum:")
        .assert_contains("30");
}

#[test]
fn test_script_missing_args_uses_main_definition_for_backtrace() {
    let project = ProjectBuilder::new("script-missing-args")
        .script_file(
            "args",
            r#"
            import "../../lib/io"

            fun main(a: int, b: int) {
                println("Arg A:");
                println(a);
                println("Arg B:");
                println(b);
            }
        "#,
        )
        .build();

    let output = project
        .acton()
        .script("scripts/args.tolk")
        .with_backtrace("full")
        .run()
        .failure();

    output
        .assert_contains("Script finished with exit code 2")
        .assert_contains("at scripts/args.tolk:")
        .assert_contains("Backtrace:")
        .assert_contains("main")
        .assert_not_contains("unknown-file");
}

#[test]
fn test_script_with_tuple_args() {
    let project = ProjectBuilder::new("script-tuple-args")
        .script_file(
            "tuple",
            r#"
            import "../../lib/io"

            fun main(t: tuple) {
                val a = t.get(0) as int;
                val b = t.get(1) as int;
                println("Tuple A:");
                println(a);
                println("Tuple B:");
                println(b);
            }
        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/tuple.tolk")
        .arg("[(10 20)]")
        .run()
        .success()
        .assert_contains("Tuple A:")
        .assert_contains("10")
        .assert_contains("Tuple B:")
        .assert_contains("20");
}

#[test]
fn test_script_with_tensor_args_and_struct() {
    let project = ProjectBuilder::new("script-tensor-args")
        .script_file(
            "tensor",
            r#"
            import "../../lib/io"

            struct Abc {
                a: int,
                b: int,
                c: int,
            }

            fun main(a: Abc) {
                println("a: {}", a.a);
                println("b: {}", a.b);
                println("c: {}", a.c);
            }

        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/tensor.tolk")
        .arg("[ 10 20 30 ]")
        .run()
        .success()
        .assert_contains("a: 10")
        .assert_contains("b: 20")
        .assert_contains("c: 30");
}

#[test]
fn test_script_with_args_and_struct() {
    let project = ProjectBuilder::new("script-tensor-args")
        .script_file(
            "tensor",
            r#"
            import "../../lib/io"

            struct Abc {
                a: int,
                b: int,
                c: int,
            }

            fun main(a: Abc) {
                println("a: {}", a.a);
                println("b: {}", a.b);
                println("c: {}", a.c);
            }

        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/tensor.tolk")
        .arg("10")
        .arg("20")
        .arg("30")
        .run()
        .success()
        .assert_contains("a: 10")
        .assert_contains("b: 20")
        .assert_contains("c: 30");
}

#[test]
fn test_script_with_null_arg() {
    let project = ProjectBuilder::new("script-tuple-args")
        .script_file(
            "tuple",
            r#"
            import "../../lib/io"

            fun main(a: int?) {
                println("a: {}", a);
            }

        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/tuple.tolk")
        .arg("null")
        .run()
        .success()
        .assert_contains("a: null");

    project
        .acton()
        .script("scripts/tuple.tolk")
        .arg("10")
        .run()
        .success()
        .assert_contains("a: 10");
}

#[test]
fn test_script_with_cell_arg() {
    let project = ProjectBuilder::new("script-cell-args")
        .script_file(
            "cell",
            r#"
            import "../../lib/io"

            fun main(a: cell) {
                var slice = a.beginParse();
                println("a: {}", slice.loadUint(32));
            }

        "#,
        )
        .build();

    let mut builder = CellBuilder::new();
    builder.store_uint(999, 32).ok();
    let cell = builder.build().ok().unwrap_or_default();
    let cell_hex = Boc::encode_hex(cell);

    project
        .acton()
        .script("scripts/cell.tolk")
        .arg(&format!("C{{{cell_hex}}}"))
        .run()
        .success()
        .assert_contains("a: 999");
}

#[test]
fn test_script_with_slice_arg() {
    let project = ProjectBuilder::new("script-cell-args")
        .script_file(
            "cell",
            r#"
            import "../../lib/io"

            fun main(a: slice) {
                println("a: {}", a.loadUint(32));
            }

        "#,
        )
        .build();

    let mut builder = CellBuilder::new();
    builder.store_uint(999, 32).ok();
    let cell = builder.build().ok().unwrap_or_default();
    let cell_hex = Boc::encode_hex(cell);

    project
        .acton()
        .script("scripts/cell.tolk")
        .arg(&format!("CS{{{cell_hex}}}"))
        .run()
        .success()
        .assert_contains("a: 999");
}

#[test]
#[ignore]
fn test_script_with_string_arg() {
    let project = ProjectBuilder::new("script-string-args")
        .script_file(
            "string",
            r#"
            import "../../lib/io"

            fun main(a: slice) {
                println("a: {}", a);
            }

        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/string.tolk")
        .arg(r#""hello world""#)
        .run()
        .success()
        .assert_contains("a: hello world");
}

#[test]
#[ignore]
fn test_script_with_long_string_arg() {
    let project = ProjectBuilder::new("script-string-args")
        .script_file(
            "string",
            r#"
            import "../../lib/io"

            fun main(a: slice) {
                println("a: {}", a);
            }

        "#,
        )
        .build();

    let string = "hello world ".repeat(1000);
    project
        .acton()
        .script("scripts/string.tolk")
        .arg(&format!("\"{string}\""))
        .run()
        .success()
        .assert_contains(&format!("a: {string}"));
}

#[test]
fn test_script_with_invalid_arg() {
    let project = ProjectBuilder::new("script-cell-args")
        .script_file(
            "cell",
            r#"
            import "../../lib/io"

            fun main(a: int) {
                println("a: {}", a);
            }

        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/cell.tolk")
        .arg("[ 10")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_script_with_invalid_arg.stderr.txt",
        );
}

#[test]
fn test_script_to_calculate_storage_fee() {
    let project = ProjectBuilder::new("script-cell-args")
        .script_file(
            "cell",
            r#"
            import "../../lib/io"

            import "@stdlib/gas-payments"

            fun main(libraryCode: cell, duration: int) {
                val gasConsumedBeforeCalculation = getGasConsumedAtTheMoment();
                val (libraryRefs, libraryBits, _) = libraryCode.calculateSizeStrict(2048);
                val gasConsumedForCalculation = getGasConsumedAtTheMoment() - gasConsumedBeforeCalculation;

                val toReserve = calculateGasFeeWithoutFlatPrice(MASTERCHAIN, gasConsumedForCalculation)
                    + calculateStorageFee(MASTERCHAIN, duration, libraryBits, libraryRefs);
                println("{:ton}", toReserve);
            }
        "#,
        )
        .build();

    let mut builder = CellBuilder::new();
    builder.store_uint(999, 32).ok();
    let cell = builder.build().ok().unwrap_or_default();
    let cell_hex = Boc::encode_hex(cell);

    project
        .acton()
        .script("scripts/cell.tolk")
        .arg(&format!("C{{{cell_hex}}}"))
        .arg(&(60 * 60 * 24 * 365).to_string())
        .run()
        .success()
        .assert_contains("0.258139024 TON");
}

// ========================================
// Script Compilation Tests
// ========================================

#[test]
fn test_script_compilation_error() {
    let project = ProjectBuilder::new("script-compile-error")
        .script_file(
            "broken",
            r"
            fun main() {
                val x = nonexistent_function();
            }
        ",
        )
        .build();

    project
        .acton()
        .script("scripts/broken.tolk")
        .run()
        .failure()
        .assert_stderr_contains("undefined symbol")
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_script_compilation_error.stderr.txt",
        );
}

#[test]
fn test_script_syntax_error() {
    let project = ProjectBuilder::new("script-syntax")
        .script_file(
            "syntax",
            r"
            val x = {{{;
        ",
        )
        .build();

    project
        .acton()
        .script("scripts/syntax.tolk")
        .run()
        .failure();
}

// ========================================
// Script with Libraries Tests
// ========================================

#[test]
fn test_script_with_multiple_operations() {
    let project = ProjectBuilder::new("script-multi")
        .script_file(
            "multi",
            r#"
            import "../../lib/io"

            fun main() {
                println("Step 1");
                val a = 10;
                println("Step 2");
                val b = 20;
                println("Step 3");
                val sum = a + b;
                println("Sum: ");
                println(sum);
            }
        "#,
        )
        .build();

    let output = project.acton().script("scripts/multi.tolk").run().code(0);

    output
        .assert_contains("Step 1")
        .assert_contains("Step 2")
        .assert_contains("Step 3")
        .assert_contains("Sum:")
        .assert_contains("30");
}

// ========================================
// Clear Cache Tests
// ========================================

#[test]
fn test_script_with_clear_cache() {
    let project = ProjectBuilder::new("script-cache")
        .script_file(
            "test",
            r#"
            import "../../lib/io"

            fun main() {
                println("Running with cache clear");
            }
        "#,
        )
        .build();

    project.acton().script("scripts/test.tolk").run().code(0);

    project
        .acton()
        .script("scripts/test.tolk")
        .clear_cache()
        .run()
        .code(0)
        .assert_contains("Cache cleared");
}

// ========================================
// Exit Code Tests
// ========================================

#[test]
fn test_script_custom_exit_code() {
    let project = ProjectBuilder::new("script-exit")
        .script_file(
            "exit_777",
            r#"
            import "../../lib/io"

            fun main() {
                println("Exiting with code 777");
                throw 777
            }
        "#,
        )
        .build();

    let output = project
        .acton()
        .script("scripts/exit_777.tolk")
        .run()
        .code(1);

    output.assert_snapshot_matches("integration/snapshots/test_script_custom_exit_code.stdout.txt");
}

#[test]
fn test_script_custom_exit_code_from_abi_shows_single_name() {
    let project = ProjectBuilder::new("script-exit-abi")
        .script_file(
            "exit_abi",
            r#"
            import "../../lib/io"

            enum Errors {
                AbiFailure = 709
            }

            fun main() {
                println("Exiting with code 709");
                throw Errors.AbiFailure
            }
        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/exit_abi.tolk")
        .run()
        .code(1)
        .assert_not_contains("Error: Errors.AbiFailure")
        .assert_snapshot_matches(
            "integration/snapshots/test_script_custom_exit_code_from_abi_shows_single_name.stdout.txt",
        );
}

#[test]
fn test_script_success_exit_code() {
    let project = ProjectBuilder::new("script-success")
        .script_file(
            "success",
            r#"
            import "../../lib/io"

            fun main() {
                println("Success!");
            }
        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/success.tolk")
        .run()
        .code(0)
        .assert_snapshot_matches("integration/snapshots/test_script_success_exit_code.stdout.txt");
}

#[test]
fn test_script_known_exit_code_shows_description_and_phase() {
    let project = ProjectBuilder::new("script-known-exit")
        .script_file(
            "exit_2",
            r#"
            import "../../lib/io"

            fun main() {
                println("Exiting with code 2");
                throw 2
            }
        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/exit_2.tolk")
        .run()
        .code(1)
        .assert_snapshot_matches(
            "integration/snapshots/test_script_known_exit_code_shows_description_and_phase.stdout.txt",
        );
}

#[test]
fn test_script_known_exit_code_shows_backtrace_with_full_mode() {
    let project = ProjectBuilder::new("script-known-exit-backtrace")
        .script_file(
            "exit_2_backtrace",
            r#"
            import "../../lib/io"

            fun explode() {
                throw 2
            }

            fun nested() {
                explode();
            }

            fun main() {
                nested();
            }
        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/exit_2_backtrace.tolk")
        .with_backtrace("full")
        .run()
        .code(1)
        .assert_snapshot_matches(
            "integration/snapshots/test_script_known_exit_code_shows_backtrace_with_full_mode.stdout.txt",
        );
}

#[test]
fn test_script_custom_exit_code_from_abi_with_backtrace_full_shows_single_name() {
    let project = ProjectBuilder::new("script-exit-abi-backtrace")
        .script_file(
            "exit_abi_backtrace",
            r#"
            import "../../lib/io"

            enum Errors {
                AbiFailure = 709
            }

            fun explode() {
                throw Errors.AbiFailure
            }

            fun nested() {
                explode();
            }

            fun main() {
                nested();
            }
        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/exit_abi_backtrace.tolk")
        .with_backtrace("full")
        .run()
        .code(1)
        .assert_not_contains("Error: Errors.AbiFailure")
        .assert_snapshot_matches(
            "integration/snapshots/test_script_custom_exit_code_from_abi_with_backtrace_full_shows_single_name.stdout.txt",
        );
}

#[test]
fn test_script_invalid_message_exit_code_shows_description_and_phase() {
    let project = ProjectBuilder::new("script-invalid-message-exit")
        .script_file(
            "exit_65535",
            r#"
            import "../../lib/io"

            fun main() {
                println("Exiting with code 65535");
                throw 0xFF_FF
            }
        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/exit_65535.tolk")
        .run()
        .code(1)
        .assert_snapshot_matches(
            "integration/snapshots/test_script_invalid_message_exit_code_shows_description_and_phase.stdout.txt",
        );
}

// ========================================
// Snapshot Tests
// ========================================

#[test]
fn test_script_assert_failure_formats_detailed_output() {
    let project = ProjectBuilder::new("script-assert-format")
        .script_file(
            "assert_failure",
            r#"
            import "../../lib/testing/assert"

            fun main() {
                Assert.equal(
                    (42, 41),
                    (42, 42),
                    "script assert diagnostics",
                    "scripts/assert_failure.tolk:8:21"
                );
            }
        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/assert_failure.tolk")
        .run()
        .failure()
        .assert_snapshot_matches(
            "integration/snapshots/test_script_assert_failure_formats_detailed_output.stdout.txt",
        );
}

#[test]
fn test_script_to_have_tx_not_found_shows_transaction_search_details() {
    let project = ProjectBuilder::new("script-tx-not-found")
        .contract(
            "simple",
            r"
            fun onInternalMessage(_: InMessage) {}
            fun onBouncedMessage(_: InMessageBounced) {}
        ",
        )
        .script_file(
            "tx_not_found",
            r#"
            import "../../lib/build"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"
            import "../../lib/emulation/scripts"
            import "../../lib/testing/expect"

            fun main() {
                val sender = testing.treasury("sender");
                val init = ContractState {
                    code: build("simple"),
                    data: createEmptyCell(),
                };
                val target = AutoDeployAddress { stateInit: init }.calculateAddress();

                val txs = net.send(sender.address, createMessage({
                    bounce: false,
                    value: ton("1"),
                    dest: { stateInit: init },
                    body: beginCell().storeUint(0x01020304, 32).endCell(),
                }));

                expect(txs).toHaveTx({
                    from: sender.address,
                    to: target,
                    opcode: 0xDEADBEEF,
                });
            }
        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/tx_not_found.tolk")
        .run()
        .failure()
        .assert_snapshot_matches(
            "integration/snapshots/test_script_to_have_tx_not_found_shows_transaction_search_details.stdout.txt",
        );
}

#[test]
fn test_script_run_get_method_on_undeployed_contract_shows_actionable_error() {
    let project = ProjectBuilder::new("script-get-method-undeployed")
        .script_file(
            "get_undeployed",
            r#"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"
            import "../../lib/emulation/scripts"

            fun main() {
                val target = randomAddress("target");
                val _: int = net.runGetMethod(target, "seqno");
            }
        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/get_undeployed.tolk")
        .run()
        .code(1)
        .assert_snapshot_matches(
            "integration/snapshots/test_script_run_get_method_on_undeployed_contract_shows_actionable_error.stdout.txt",
        );
}

#[test]
fn test_script_run_get_method_on_contract_without_code_shows_actionable_error() {
    let project = ProjectBuilder::new("script-get-method-null-code")
        .script_file(
            "get_null_code",
            r#"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"
            import "../../lib/emulation/scripts"

            fun main() {
                val deployer = testing.treasury("deployer");
                val address = AutoDeployAddress {
                    stateInit: beginCell()
                        .storeBool(false) // fixed_prefix_length:(Maybe (## 5))
                        .storeBool(false) // special:(Maybe TickTock)
                        .storeBool(false) // code:(Maybe ^Cell)
                        .storeBool(false) // data:(Maybe ^Cell)
                        .storeBool(false) // library:(Maybe ^Cell)
                        .endCell(),
                };

                val outMsg = createMessage({
                    bounce: BounceMode.NoBounce,
                    value: ton("0.1"),
                    dest: address,
                });
                net.send(deployer.address, outMsg);

                val _: int = net.runGetMethod(address.calculateAddress(), "counter");
            }
        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/get_null_code.tolk")
        .run()
        .code(1)
        .assert_snapshot_matches(
            "integration/snapshots/test_script_run_get_method_on_contract_without_code_shows_actionable_error.stdout.txt",
        );
}

#[test]
fn test_script_output_snapshot() {
    let project = ProjectBuilder::new("script-snapshot")
        .script_file(
            "output",
            r#"
            import "../../lib/io"

            fun main() {
                println("Line 1");
                println("Line 2");
                println("Line 3");
            }
        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/output.tolk")
        .run()
        .code(0)
        .assert_snapshot_matches("integration/snapshots/test_script_output_snapshot.stdout.txt");
}

#[test]
fn test_script_multi_arg_println_helpers_snapshot() {
    let project = ProjectBuilder::new("script-println-multiarg-snapshot")
        .script_file(
            "output",
            r#"
            import "../../lib/io"

            fun main() {
                println("{} + {}", "left", "right");
                println("hex={:x} ton={:ton} label={}", 255, 2500000000, "ok");
                println("{} {} {} {}", "a", "b", "c", "d");
                println("{} {} {} {} {}", 1, 2, 3, 4, 5);
            }
        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/output.tolk")
        .run()
        .code(0)
        .assert_snapshot_matches(
            "integration/snapshots/test_script_multi_arg_println_helpers_snapshot.stdout.txt",
        );
}

// ========================================
// Additional Error Handling Tests
// ========================================

#[test]
fn test_script_invalid_network() {
    let project = ProjectBuilder::new("script-invalid-net")
        .script_file(
            "test",
            r#"
            import "../../lib/io"

            fun main() {
                println("Test");
            }
        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/test.tolk")
        .with_net("invalid-network-name")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_script_invalid_network.stderr.txt",
        );
}

#[test]
fn test_script_empty_script_file() {
    let project = ProjectBuilder::new("script-empty")
        .script_file("empty", "")
        .build();

    project
        .acton()
        .script("scripts/empty.tolk")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_script_empty_script_file.stderr.txt",
        );
}

#[test]
fn test_script_no_main_function() {
    let project = ProjectBuilder::new("script-no-main")
        .script_file(
            "no_main",
            r#"
            import "../../lib/io"

            fun not_main() {
                println("This is not main!");
            }
        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/no_main.tolk")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_script_no_main_function.stderr.txt",
        );
}

#[test]
fn test_script_empty_path() {
    let project = ProjectBuilder::new("script-empty-path").build();

    project
        .acton()
        .script("")
        .run()
        .failure()
        .assert_stderr_snapshot_matches("integration/snapshots/test_script_empty_path.stderr.txt");
}

#[test]
fn test_script_file_without_read_permission() {
    let project = ProjectBuilder::new("script-no-read")
        .script_file(
            "secret",
            r#"
            import "../../lib/io"

            fun main() {
                println("Secret script");
            }
        "#,
        )
        .build();

    // Make the file unreadable (on Unix systems)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let file_path = project.path().join("scripts/secret.tolk");
        let mut perms = fs::metadata(&file_path).unwrap().permissions();
        perms.set_mode(0o000); // no permissions
        fs::set_permissions(&file_path, perms).unwrap();
    }

    project
        .acton()
        .script("scripts/secret.tolk")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_script_file_without_read_permission.stderr.txt",
        );
}

#[test]
fn test_script_broadcast_with_nonexistent_wallet_with_wallets() {
    let project = ProjectBuilder::new("script-broadcast-wallet-no-config")
        .script_file(
            "deploy",
            r#"
            import "../../lib/io"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"
            import "../../lib/emulation/scripts"

            fun main() {
                println("Attempting to deploy with nonexistent wallet");
                // This should fail because wallet "nonexistent" is not defined
                val wallet = scripts.wallet("nonexistent");
                println("Wallet found: {}", wallet.address);
            }
        "#,
        )
        .build();

    let mnemonic = "cupboard match uphold miracle fog balance unknown region share hand trophy million toy narrow ability exchange first toast fresh maid report cram strong later";
    fs::write(project.path().join("mnemonic.txt"), mnemonic).unwrap();

    let toml_content = r#"
[package]
name = "script-broadcast-wallet-no-config"
description = ""
version = "0.1.0"
"#;
    fs::write(project.path().join("Acton.toml"), toml_content).unwrap();

    let wallets_toml = r#"
[wallets.deployer]
kind = "v5r1"
workchain = 0
keys = { mnemonic-file = "mnemonic.txt" }
"#;
    fs::write(project.path().join("wallets.toml"), wallets_toml).unwrap();

    let home_temp = tempfile::TempDir::new().unwrap();
    let home_path = home_temp.path();

    project
        .acton()
        .env("HOME", home_path.to_str().unwrap())
        .script("scripts/deploy.tolk")
        .verify_network("testnet")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_script_broadcast_with_nonexistent_wallet_with_wallets.stderr.txt",
        );
}

#[test]
fn test_script_broadcast_with_nonexistent_wallet_no_config() {
    let project = ProjectBuilder::new("script-broadcast-wallet-no-config")
        .script_file(
            "deploy",
            r#"
            import "../../lib/io"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"
            import "../../lib/emulation/scripts"

            fun main() {
                println("Attempting to deploy with nonexistent wallet");
                // This should fail because wallet "nonexistent" is not defined
                val wallet = scripts.wallet("nonexistent");
                println("Wallet found: {}", wallet.address);
            }
        "#,
        )
        .build();

    let toml_content = r#"
[package]
name = "script-broadcast-wallet-no-config"
description = ""
version = "0.1.0"

"#;
    fs::write(project.path().join("Acton.toml"), toml_content).unwrap();

    let home_temp = tempfile::TempDir::new().unwrap();
    let home_path = home_temp.path();

    project
        .acton()
        .env("HOME", home_path.to_str().unwrap())
        .script("scripts/deploy.tolk")
        .verify_network("testnet")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_script_broadcast_with_nonexistent_wallet.stderr.txt",
        );
}

#[test]
fn test_script_broadcast_with_nonexistent_wallet_empty_config() {
    let project = ProjectBuilder::new("script-broadcast-wallet-empty-config")
        .script_file(
            "deploy",
            r#"
            import "../../lib/io"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"
            import "../../lib/emulation/scripts"

            fun main() {
                println("Attempting to deploy with nonexistent wallet");
                // This should fail because wallet "nonexistent" is not defined
                val wallet = scripts.wallet("nonexistent");
                println("Wallet found: {}", wallet.address);
            }
        "#,
        )
        .build();

    let toml_content = r#"
[package]
name = "script-broadcast-wallet-empty-config"
description = ""
version = "0.1.0"

[wallets]
"#;
    fs::write(project.path().join("Acton.toml"), toml_content).unwrap();

    let home_temp = tempfile::TempDir::new().unwrap();
    let home_path = home_temp.path();

    project
        .acton()
        .env("HOME", home_path.to_str().unwrap())
        .script("scripts/deploy.tolk")
        .verify_network("testnet")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_script_broadcast_with_nonexistent_wallet_empty_config.stderr.txt",
        );
}

#[test]
fn test_script_broadcast_wallet_exposes_key_helpers_for_v5() {
    let project = ProjectBuilder::new("script-broadcast-wallet-key-helpers-v5")
        .script_file(
            "wallet_keys",
            r#"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"
            import "../../lib/emulation/scripts"
            import "../../lib/io"

            fun main() {
                val wallet = scripts.wallet("deployer");
                val keyPair = wallet.toKeyPair();

                if (keyPair.privateKey == 0) {
                    throw 101;
                }
                if (keyPair.publicKey == 0) {
                    throw 102;
                }
                if (wallet.privateKey() != keyPair.privateKey) {
                    throw 103;
                }
                if (wallet.publicKey() != keyPair.publicKey) {
                    throw 104;
                }

                val payload = beginCell().storeUint(0xA17, 12).endCell();
                if (!isSignatureValid(payload.hash(), wallet.sign(payload), keyPair.publicKey)) {
                    throw 105;
                }
                if (!isSignatureValid(payload.hash(), wallet.rawSign(payload.hash()), wallet.publicKey())) {
                    throw 106;
                }
                if (wallet.walletId() == 0) {
                    throw 107;
                }

                println("wallet key helpers ok");
            }
        "#,
        )
        .build();

    fs::write(project.path().join("mnemonic.txt"), DEPLOYER_MNEMONIC)
        .expect("failed to write mnemonic");
    fs::write(
        project.path().join("wallets.toml"),
        r#"[wallets.deployer]
kind = "v5r1"
workchain = 0
keys = { mnemonic-file = "mnemonic.txt" }
"#,
    )
    .expect("failed to write wallets.toml");

    project
        .acton()
        .script("scripts/wallet_keys.tolk")
        .verify_network("testnet")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_script_broadcast_wallet_exposes_key_helpers_for_v5.stdout.txt",
        );
}

#[test]
fn test_script_wallet_key_pair_requires_open_broadcast_wallet() {
    let project = ProjectBuilder::new("script-wallet-key-pair-requires-open-broadcast-wallet")
        .script_file(
            "wallet_keys_error",
            r#"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"
            import "../../lib/emulation/scripts"

            fun main() {
                val wallet = scripts.wallet("deployer");
                wallet.toKeyPair();
            }
        "#,
        )
        .build();

    let output = project
        .acton()
        .script("scripts/wallet_keys_error.tolk")
        .run()
        .failure();

    output.assert_snapshot_matches(
        "integration/snapshots/test_script_wallet_key_pair_requires_open_broadcast_wallet.stdout.txt",
    );
}

#[test]
fn test_script_wallet_id_requires_open_broadcast_wallet() {
    let project = ProjectBuilder::new("script-wallet-id-requires-open-broadcast-wallet")
        .script_file(
            "wallet_id_error",
            r#"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"
            import "../../lib/emulation/scripts"

            fun main() {
                val wallet = scripts.wallet("deployer");
                wallet.walletId();
            }
        "#,
        )
        .build();

    let output = project
        .acton()
        .script("scripts/wallet_id_error.tolk")
        .run()
        .failure();

    output.assert_snapshot_matches(
        "integration/snapshots/test_script_wallet_id_requires_open_broadcast_wallet.stdout.txt",
    );
}

#[test]
fn test_script_broadcast_treasury_recommends_wallet_api() {
    let project = ProjectBuilder::new("script-broadcast-treasury-recommends-wallet")
        .script_file(
            "deploy",
            r#"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"
            import "../../lib/emulation/scripts"

            fun main() {
                testing.treasury("deployer");
            }
        "#,
        )
        .build();

    let output = project
        .acton()
        .script("scripts/deploy.tolk")
        .verify_network("testnet")
        .run()
        .failure();

    output.assert_snapshot_matches(
        "integration/snapshots/test_script_broadcast_treasury_recommends_wallet_api.stdout.txt",
    );
}

#[test]
fn test_script_broadcast_wallet_rejection_shows_actionable_toncenter_hint() {
    let project = build_broadcast_wallet_error_project("script-broadcast-wallet-rejection");

    let (mock_url, mock_handle) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_seqno_ok_response(),
        toncenter_v2_error_response(
            400,
            "LITE_SERVER_UNKNOWN: cannot apply external message to current state : External message was not accepted: cannot run message on account: inbound external message rejected by account 3029B3EAEDA86A5381D86100F2A8B761C38DE45642EDB6E4BB1CCA2E6DD7FFED before smart-contract execution",
        ),
    ]);
    append_custom_network(project.path(), "mock-v2", &mock_url);

    let output = project
        .acton()
        .env("ACTON_DISABLE_SYSTEM_PROXY", "1")
        .script("scripts/deploy.tolk")
        .verify_network("custom:mock-v2")
        .run()
        .failure();

    output.assert_snapshot_matches(
        "integration/snapshots/test_script_broadcast_wallet_rejection_shows_actionable_toncenter_hint.stdout.txt",
    );

    mock_handle.join().expect("mock toncenter v2 must finish");
}

#[test]
fn test_script_broadcast_missing_account_state_without_state_init_shows_wallet_setup_hint() {
    let project =
        build_broadcast_wallet_error_project("script-broadcast-wallet-missing-account-state");

    let (mock_url, mock_handle) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_seqno_ok_response(),
        toncenter_v2_error_response(
            400,
            "LITE_SERVER_UNKNOWN: cannot apply external message to current state : Failed to unpack account state",
        ),
    ]);
    append_custom_network(project.path(), "mock-v2-missing-account", &mock_url);

    let output = project
        .acton()
        .env("ACTON_DISABLE_SYSTEM_PROXY", "1")
        .script("scripts/deploy.tolk")
        .verify_network("custom:mock-v2-missing-account")
        .run()
        .failure();

    output.assert_snapshot_matches(
        "integration/snapshots/test_script_broadcast_missing_account_state_without_state_init_shows_wallet_setup_hint.stdout.txt",
    );

    mock_handle.join().expect("mock toncenter v2 must finish");
}

#[test]
fn test_script_broadcast_missing_account_state_on_localnet_shows_localnet_airdrop_hint() {
    let project =
        build_broadcast_wallet_error_project("script-broadcast-wallet-missing-account-localnet");

    let (mock_url, mock_handle) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_seqno_ok_response(),
        toncenter_v2_error_response(
            400,
            "LITE_SERVER_UNKNOWN: cannot apply external message to current state : Failed to unpack account state",
        ),
    ]);
    append_localnet_network(project.path(), &format!("{mock_url}/api/v2"));

    let output = project
        .acton()
        .env("ACTON_DISABLE_SYSTEM_PROXY", "1")
        .script("scripts/deploy.tolk")
        .verify_network("localnet")
        .run()
        .failure();

    output.assert_snapshot_matches(
        "integration/snapshots/test_script_broadcast_missing_account_state_on_localnet_shows_localnet_airdrop_hint.stdout.txt",
    );

    mock_handle.join().expect("mock toncenter v2 must finish");
}

#[test]
fn test_script_broadcast_defaults_fork_net_to_broadcast_network() {
    let project = ProjectBuilder::new("script-broadcast-defaults-fork-net")
        .file(
            "contracts/types",
            r"
enum Errors {
    InvalidMessage = 0xFFFF
}

struct Storage {
    id: uint32
    counter: uint32
}

fun Storage.load(): Storage {
    return Storage.fromCell(contract.getData());
}

fun Storage.save(self) {
    contract.setData(self.toCell());
}

struct (0x7e8764ef) IncreaseCounter {
    increaseBy: uint32
}

struct (0x3a752f06) ResetCounter {}
",
        )
        .contract(
            "counter",
            r#"
import "types"

contract Counter {
    storage: Storage
    incomingMessages: AllowedMessage
}

type AllowedMessage = IncreaseCounter | ResetCounter

fun onInternalMessage(in: InMessage) {
    val msg = lazy AllowedMessage.fromSlice(in.body);

    match (msg) {
        IncreaseCounter => {
            var storage = lazy Storage.load();
            storage.counter += msg.increaseBy;
            storage.save();
        }
        ResetCounter => {
            var storage = lazy Storage.load();
            storage.counter = 0;
            storage.save();
        }
        else => {
            assert (in.body.isEmpty()) throw Errors.InvalidMessage;
        }
    }
}

fun onBouncedMessage(_in: InMessageBounced) {}

get fun currentCounter(): int {
    val storage = lazy Storage.load();
    return storage.counter;
}
"#,
        )
        .script_file(
            "deploy_counter",
            r#"
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/emulation/scripts"
import "../../lib/io"
import "../contracts/types"

fun main() {
    val deployer = scripts.wallet("deployer");
    val init = ContractState {
        code: build("counter"),
        data: Storage {
            id: 0,
            counter: 7,
        }.toCell(),
    };
    val counterAddress = AutoDeployAddress { stateInit: init }.calculateAddress();
    val res = net.send(deployer.address, createMessage({
        bounce: false,
        value: ton("0.05"),
        dest: { stateInit: init },
    }));
    if (res.waitForFirstTransaction() == null) {
        return;
    }

    println("COUNTER_ADDRESS={}", counterAddress);
}
"#,
        )
        .build();

    write_localnet_wallet_config(&project, "deployer");

    let node = project.localnet().args(["--accounts", "deployer"]).start();
    append_localnet_network(project.path(), &format!("{}/api/v2", node.base_url()));

    let deploy_output = project
        .acton()
        .script("scripts/deploy_counter.tolk")
        .verify_network("localnet")
        .run()
        .success();
    let counter_address = extract_marker_value(&deploy_output.get_stdout(), "COUNTER_ADDRESS=");
    wait_until_address_state_active(&node, &counter_address, Duration::from_secs(12));

    fs::write(
        project.path().join("scripts/query_counter.tolk"),
        format!(
            r#"
import "../../lib/emulation/network"
import "../../lib/io"

fun main() {{
    val counter: int = net.runGetMethod(address("{counter_address}"), "currentCounter");
    println("On-chain counter: {{}}", counter);
}}
"#
        ),
    )
    .expect("failed to write query script");

    project
        .acton()
        .script("scripts/query_counter.tolk")
        .verify_network("localnet")
        .run()
        .success()
        .assert_contains("On-chain counter: 7");

    node.stop();
}

#[test]
fn test_script_broadcast_rejects_conflicting_net_and_fork_net() {
    let project = ProjectBuilder::new("script-broadcast-net-conflict")
        .script_file(
            "hello",
            r#"
            import "../../lib/io"

            fun main() {
                println("hello");
            }
        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/hello.tolk")
        .with_net("testnet")
        .fork_net("mainnet")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_script_broadcast_rejects_conflicting_net_and_fork_net.stderr.txt",
        );
}

#[test]
fn test_script_broadcast_missing_account_state_with_state_init_shows_deploy_hint() {
    let project =
        build_broadcast_wallet_error_project("script-broadcast-wallet-missing-account-with-init");

    let (mock_url, mock_handle) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_error_response(400, "account is not active"),
        toncenter_v2_error_response(
            400,
            "LITE_SERVER_UNKNOWN: cannot apply external message to current state : Failed to unpack account state",
        ),
    ]);
    append_custom_network(
        project.path(),
        "mock-v2-missing-account-with-init",
        &mock_url,
    );

    let output = project
        .acton()
        .env("ACTON_DISABLE_SYSTEM_PROXY", "1")
        .script("scripts/deploy.tolk")
        .verify_network("custom:mock-v2-missing-account-with-init")
        .run()
        .failure();

    output.assert_snapshot_matches(
        "integration/snapshots/test_script_broadcast_missing_account_state_with_state_init_shows_deploy_hint.stdout.txt",
    );

    mock_handle.join().expect("mock toncenter v2 must finish");
}

#[test]
fn test_script_broadcast_wallet_rejection_with_state_init_shows_deploy_hint() {
    let project =
        build_broadcast_wallet_error_project("script-broadcast-wallet-rejection-with-init");

    let (mock_url, mock_handle) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_error_response(400, "account is not active"),
        toncenter_v2_error_response(
            400,
            "LITE_SERVER_UNKNOWN: cannot apply external message to current state : External message was not accepted: cannot run message on account: inbound external message rejected by account 3029B3EAEDA86A5381D86100F2A8B761C38DE45642EDB6E4BB1CCA2E6DD7FFED before smart-contract execution",
        ),
    ]);
    append_custom_network(
        project.path(),
        "mock-v2-wallet-rejection-with-init",
        &mock_url,
    );

    let output = project
        .acton()
        .env("ACTON_DISABLE_SYSTEM_PROXY", "1")
        .script("scripts/deploy.tolk")
        .verify_network("custom:mock-v2-wallet-rejection-with-init")
        .run()
        .failure();

    output.assert_snapshot_matches(
        "integration/snapshots/test_script_broadcast_wallet_rejection_with_state_init_shows_deploy_hint.stdout.txt",
    );

    mock_handle.join().expect("mock toncenter v2 must finish");
}

#[test]
fn test_script_address_print_default() {
    let project = ProjectBuilder::new("script-simple")
        .script_file(
            "hello",
            r#"
            import "../../lib/io"

            fun main() {
                println(address("EQBvDB_H7FFBs0nF4ap_DBdcOrwY_rMIpNVVOR6SWYFHByMJ"));
            }
        "#,
        )
        .build();

    let output = project.acton().script("scripts/hello.tolk").run().code(0);

    output.assert_contains("kQBvDB_H7FFBs0nF4ap_DBdcOrwY_rMIpNVVOR6SWYFHB5iD");
}

#[test]
fn test_script_address_print_fork_testnet() {
    let project = ProjectBuilder::new("script-simple")
        .script_file(
            "hello",
            r#"
            import "../../lib/io"

            fun main() {
                println(address("EQBvDB_H7FFBs0nF4ap_DBdcOrwY_rMIpNVVOR6SWYFHByMJ"));
            }
        "#,
        )
        .build();

    let output = project
        .acton()
        .script("scripts/hello.tolk")
        .fork_net("testnet")
        .run()
        .success();

    output.assert_contains("kQBvDB_H7FFBs0nF4ap_DBdcOrwY_rMIpNVVOR6SWYFHB5iD");
}

#[test]
fn test_script_address_print_fork_mainnet() {
    let project = ProjectBuilder::new("script-simple")
        .script_file(
            "hello",
            r#"
            import "../../lib/io"

            fun main() {
                println(address("EQBvDB_H7FFBs0nF4ap_DBdcOrwY_rMIpNVVOR6SWYFHByMJ"));
            }
        "#,
        )
        .build();

    let output = project
        .acton()
        .script("scripts/hello.tolk")
        .fork_net("mainnet")
        .run()
        .success();

    output.assert_contains("EQBvDB_H7FFBs0nF4ap_DBdcOrwY_rMIpNVVOR6SWYFHByMJ");
}

#[test]
fn test_script_address_print_broadcast_net_testnet() {
    let project = ProjectBuilder::new("script-simple")
        .script_file(
            "hello",
            r#"
            import "../../lib/io"

            fun main() {
                println(address("EQBvDB_H7FFBs0nF4ap_DBdcOrwY_rMIpNVVOR6SWYFHByMJ"));
            }
        "#,
        )
        .build();

    let output = project
        .acton()
        .script("scripts/hello.tolk")
        .with_net("testnet")
        .run()
        .success();

    output.assert_contains("kQBvDB_H7FFBs0nF4ap_DBdcOrwY_rMIpNVVOR6SWYFHB5iD");
}

#[test]
fn test_script_address_print_broadcast_net_mainnet() {
    let project = ProjectBuilder::new("script-simple")
        .script_file(
            "hello",
            r#"
            import "../../lib/io"

            fun main() {
                println(address("EQBvDB_H7FFBs0nF4ap_DBdcOrwY_rMIpNVVOR6SWYFHByMJ"));
            }
        "#,
        )
        .build();

    let output = project
        .acton()
        .script("scripts/hello.tolk")
        .with_net("mainnet")
        .run()
        .success();

    output.assert_contains("EQBvDB_H7FFBs0nF4ap_DBdcOrwY_rMIpNVVOR6SWYFHByMJ");
}

#[test]
fn test_script_env_vars() {
    let project = ProjectBuilder::new("script-env-vars")
        .script_file(
            "env",
            r#"
            import "../../lib/io"
            import "../../lib/env"

            fun main() {
                val i = env<int>("TEST_INT");
                if (i != null) {
                    println("int: {}", i);
                }

                val b = env<bool>("TEST_BOOL");
                if (b != null) {
                    println("bool: {}", b);
                }

                val s = env<string>("TEST_SLICE");
                if (s != null) {
                    println("slice: {}", s);
                }

                val a = env<address>("TEST_ADDRESS");
                if (a != null) {
                    println("address: {}", a);
                }

                val c = env<cell>("TEST_CELL");
                if (c != null) {
                    var slice = c.beginParse();
                    println("cell: {}", slice.loadUint(32));
                }
            }
        "#,
        )
        .build();

    let mut builder = CellBuilder::new();
    builder.store_uint(123, 32).ok();
    let cell = builder.build().ok().unwrap_or_default();
    let cell_hex = Boc::encode_hex(cell);

    project
        .acton()
        .script("scripts/env.tolk")
        .env("TEST_INT", "123")
        .env("TEST_BOOL", "true")
        .env("TEST_SLICE", "hello")
        .env(
            "TEST_ADDRESS",
            "EQBvDB_H7FFBs0nF4ap_DBdcOrwY_rMIpNVVOR6SWYFHByMJ",
        )
        .env("TEST_CELL", &cell_hex)
        .run()
        .success()
        .assert_contains("int: 123")
        .assert_contains("bool: true")
        .assert_contains("slice: hello")
        .assert_contains("address: kQBvDB_H7FFBs0nF4ap_DBdcOrwY_rMIpNVVOR6SWYFHB5iD")
        .assert_contains("cell: 123");
}

#[test]
fn test_script_env_vars_extended() {
    let project = ProjectBuilder::new("script-env-vars-extended")
        .script_file(
            "env",
            r#"
            import "../../lib/io"
            import "../../lib/env"

            fun main() {
                val i_hex = env<int>("TEST_INT_HEX");
                if (i_hex != null) {
                    println("int_hex: {}", i_hex);
                }

                val b_1 = env<bool>("TEST_BOOL_1");
                if (b_1 != null) {
                    println("bool_1: {}", b_1);
                }

                val b_false = env<bool>("TEST_BOOL_FALSE");
                if (b_false != null) {
                    println("bool_false: {}", b_false);
                }

                val b_0 = env<bool>("TEST_BOOL_0");
                if (b_0 != null) {
                    println("bool_0: {}", b_0);
                }

                val a_raw = env<address>("TEST_ADDRESS_RAW");
                if (a_raw != null) {
                    println("address_raw: {}", a_raw);
                }

                val c_b64 = env<cell>("TEST_CELL_B64");
                if (c_b64 != null) {
                    var slice = c_b64.beginParse();
                    println("cell_b64: {}", slice.loadUint(32));
                }
            }
        "#,
        )
        .build();

    let mut builder = CellBuilder::new();
    builder.store_uint(456, 32).ok();
    let cell = builder.build().ok().unwrap_or_default();
    let cell_b64 = Boc::encode_base64(cell);

    project
        .acton()
        .script("scripts/env.tolk")
        .env("TEST_INT_HEX", "0x1a") // 26
        .env("TEST_BOOL_1", "1")
        .env("TEST_BOOL_FALSE", "FALSE")
        .env("TEST_BOOL_0", "0")
        .env(
            "TEST_ADDRESS_RAW",
            "0:8356d05f87ec5141b349c5e1aa7f0c175c3abc18feb308a4d555391e92598147",
        )
        .env("TEST_CELL_B64", &cell_b64)
        .run()
        .success()
        .assert_contains("int_hex: 26")
        .assert_contains("bool_1: true")
        .assert_contains("bool_false: false")
        .assert_contains("bool_0: false")
        .assert_contains("address_raw: kQCDVtBfh-xRQbNJxeGqfwwXXDq8GP6zCKTVVTkeklmBRxCZ")
        .assert_contains("cell_b64: 456");
}

#[test]
fn test_script_env_vars_support_coins() {
    let project = ProjectBuilder::new("script-env-vars-coins")
        .script_file(
            "env_coins",
            r#"
            import "../../lib/io"
            import "../../lib/env"

            fun main() {
                val amount = env<coins>("TEST_COINS");
                if (amount != null) {
                    println("coins: {:ton}", amount);
                }

                val fallback = env<coins>("TEST_COINS_MISSING") ?? ton("0.75");
                println("coins_default: {:ton}", fallback);
            }
        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/env_coins.tolk")
        .env("TEST_COINS", "1500000000")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_script_env_vars_support_coins.stdout.txt",
        );
}

#[test]
fn test_script_env_or_vars() {
    let project = ProjectBuilder::new("script-env-or-vars")
        .script_file(
            "env",
            r#"
            import "../../lib/io"
            import "../../lib/env"

            fun main() {
                val i = env<int>("TEST_INT") ?? 42;
                println("int: {}", i);

                val b = env<bool>("TEST_BOOL") ?? false;
                println("bool: {}", b);

                val s = env<string>("TEST_SLICE") ?? "default";
                println("string: {}", s);

                val a = env<address>("TEST_ADDRESS") ?? address("EQBvDB_H7FFBs0nF4ap_DBdcOrwY_rMIpNVVOR6SWYFHByMJ");
                println("address: {}", a);
            }
        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/env.tolk")
        .run()
        .success()
        .assert_contains("int: 42")
        .assert_contains("bool: false")
        .assert_contains("string: default")
        .assert_contains("address: kQBvDB_H7FFBs0nF4ap_DBdcOrwY_rMIpNVVOR6SWYFHB5iD");
}

#[test]
fn test_println_nullable_values() {
    let project = ProjectBuilder::new("script-nullable-values")
        .script_file(
            "env",
            r#"
            import "../../lib/io"

            struct Foo {
                a: int,
                b: int,
            }

            fun print_option<T>(a: T?) {
                println(a);
            }

            fun main() {
                // primitive types
                print_option(10);
                print_option(null as int?);
                print_option("slice");
                print_option(null as slice?);

                // complex types
                print_option(Foo {
                    a: 0,
                    b: 1,
                });
                print_option(null as Foo?);
                print_option(null as [int, int]?);
                print_option(null as (int, int)?);
                print_option(null as ()?);

                // empty map
                print_option(createEmptyMap<int32, int32>() as map<int32, int32>?);
                print_option(createEmptyMap<int32, int34>() as map<int32, int34>?);
            }
        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/env.tolk")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/test_println_nullable_values.stderr.txt");
}

#[test]
fn test_println_non_empty_map_values() {
    let project = ProjectBuilder::new("script-println-map-values")
        .script_file(
            "map_values",
            r#"
            import "../../lib/io"

            fun main() {
                var balances = createEmptyMap<int32, int32>();
                balances.set(1, 10);
                balances.set(2, 20);
                println(balances);
            }
        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/map_values.tolk")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_println_non_empty_map_values.stderr.txt",
        );
}

#[test]
fn test_println_empty_map_values() {
    let project = ProjectBuilder::new("script-println-empty-map-values")
        .script_file(
            "map_empty_values",
            r#"
            import "../../lib/io"

            fun main() {
                val emptyInts = createEmptyMap<int32, int32>();
                println(emptyInts);

                val emptyStrings = createEmptyMap<int32, string>();
                println(emptyStrings);
            }
        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/map_empty_values.tolk")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/test_println_empty_map_values.stderr.txt");
}

#[test]
fn test_println_map_supported_key_types() {
    let project = ProjectBuilder::new("script-println-map-key-types")
        .script_file(
            "map_key_types",
            r#"
            import "../../lib/io"

            fun main() {
                val ownerRaw = address("0:8356d05f87ec5141b349c5e1aa7f0c175c3abc18feb308a4d555391e92598147");

                var byBool = createEmptyMap<bool, int32>();
                byBool.set(false, 10);
                println(byBool);

                var byAddress = createEmptyMap<address, int32>();
                byAddress.set(ownerRaw, 20);
                println(byAddress);

                var byInt8 = createEmptyMap<int8, int32>();
                byInt8.set(-1, 30);
                println(byInt8);

                var byUint16 = createEmptyMap<uint16, int32>();
                byUint16.set(65535, 40);
                println(byUint16);

                var byInt257 = createEmptyMap<int257, int32>();
                byInt257.set(-1, 50);
                println(byInt257);

                var byUint256 = createEmptyMap<uint256, int32>();
                byUint256.set(1, 60);
                println(byUint256);
            }
        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/map_key_types.tolk")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_println_map_supported_key_types.stderr.txt",
        );
}

#[test]
fn test_println_map_supported_value_types() {
    let project = ProjectBuilder::new("script-println-map-value-types")
        .script_file(
            "map_value_types",
            r#"
            import "../../lib/io"

            fun main() {
                val ownerRaw = address("0:8356d05f87ec5141b349c5e1aa7f0c175c3abc18feb308a4d555391e92598147");
                val ownerAny = ownerRaw as any_address;

                var withBool = createEmptyMap<int32, bool>();
                withBool.set(1, true);
                println(withBool);

                var withAddress = createEmptyMap<int32, address>();
                withAddress.set(2, ownerRaw);
                println(withAddress);

                var withAnyAddress = createEmptyMap<int32, any_address>();
                withAnyAddress.set(3, ownerAny);
                println(withAnyAddress);

                var withCell = createEmptyMap<int32, cell>();
                withCell.set(11, beginCell().storeUint(42, 8).endCell());
                println(withCell);

                var withString = createEmptyMap<int32, string>();
                withString.set(12, "hello");
                println(withString);

                var withInt257 = createEmptyMap<int32, int257>();
                withInt257.set(4, -123);
                println(withInt257);

                var withUint32 = createEmptyMap<int32, uint32>();
                withUint32.set(5, 123);
                println(withUint32);

                var withCoins = createEmptyMap<int32, coins>();
                withCoins.set(6, ton("1.5"));
                println(withCoins);

                var withVarInt16 = createEmptyMap<int32, varint16>();
                withVarInt16.set(7, -77);
                println(withVarInt16);

                var withVarInt32 = createEmptyMap<int32, varint32>();
                withVarInt32.set(8, -888888888);
                println(withVarInt32);

                var withVarUInt16 = createEmptyMap<int32, varuint16>();
                withVarUInt16.set(9, 65535);
                println(withVarUInt16);

                var withVarUInt32 = createEmptyMap<int32, varuint32>();
                withVarUInt32.set(10, 4294967296);
                println(withVarUInt32);
            }
        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/map_value_types.tolk")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_println_map_supported_value_types.stderr.txt",
        );
}

#[test]
fn test_println_map_fallback_for_unformattable_types() {
    let project = ProjectBuilder::new("script-println-map-fallback-types")
        .script_file(
            "map_fallback_types",
            r#"
            import "../../lib/io"

            struct Key {
                id: int32,
            }

            fun main() {
                var byStructKey = createEmptyMap<Key, int32>();
                byStructKey.set(Key { id: 1 }, 10);
                println(byStructKey);

                var nested = createEmptyMap<int32, int32>();
                nested.set(7, 70);
                var withMapValue = createEmptyMap<int32, map<int32, int32>>();
                withMapValue.set(3, nested);
                println(withMapValue);
            }
        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/map_fallback_types.tolk")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_println_map_fallback_for_unformattable_types.stderr.txt",
        );
}

#[test]
fn test_println_map_retyped_from_low_level_dict_parse_failures() {
    let project = ProjectBuilder::new("script-println-map-retyped-from-dict")
        .script_file(
            "map_retyped_from_dict",
            r#"
            import "../../lib/io"

            fun main() {
                var source = createEmptyMap<int32, int32>();
                source.set(1, 10);
                source.set(2, 20);

                val lowLevel = source.toLowLevelDict();

                val asAddress = createMapFromLowLevelDict<int32, address>(lowLevel);
                println(asAddress);

                val asCell = createMapFromLowLevelDict<int32, cell>(lowLevel);
                println(asCell);
            }
        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/map_retyped_from_dict.tolk")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_println_map_retyped_from_low_level_dict_parse_failures.stderr.txt",
        );
}

#[test]
fn test_println_map_struct_value_falls_back_to_raw_hex() {
    let project = ProjectBuilder::new("script-println-map-struct-value-raw-hex")
        .script_file(
            "map_struct_value_raw_hex",
            r#"
            import "../../lib/io"

            struct Payload {
                a: int32,
                b: bool,
            }

            fun main() {
                var byStructValue = createEmptyMap<int32, Payload>();
                byStructValue.set(1, Payload {
                    a: 7,
                    b: true,
                });
                println(byStructValue);
            }
        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/map_struct_value_raw_hex.tolk")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_println_map_struct_value_falls_back_to_raw_hex.stderr.txt",
        );
}

#[test]
fn test_script_allows_predicate_based_transaction_matchers() {
    // `expect(...).toHaveTx({ ... })` builds predicate continuations that the matcher
    // runtime evaluates against the compiled code cell of the currently running script.
    // Previously `acton script` left `test_code` unset, so this matcher bailed with an
    // "only available under `acton test`" error. This test pins down that the matcher
    // now works in script mode for both the positive and the negative path.
    let project = ProjectBuilder::new("script-predicate-matchers")
        .file(
            "contracts/types",
            r"
struct Storage {
    id: uint32
    counter: uint32
}

fun Storage.load(): Storage {
    return Storage.fromCell(contract.getData());
}

fun Storage.save(self) {
    contract.setData(self.toCell());
}

struct (0x7e8764ef) IncreaseCounter {
    increaseBy: uint32
}
",
        )
        .contract(
            "counter",
            r#"
import "types"

contract Counter {
    storage: Storage
    incomingMessages: IncreaseCounter
}

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }
    val msg = lazy IncreaseCounter.fromSlice(in.body);
    var storage = lazy Storage.load();
    storage.counter += msg.increaseBy;
    storage.save();
}

fun onBouncedMessage(_in: InMessageBounced) {}
"#,
        )
        .script_file(
            "expect_in_script",
            r#"
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/emulation/scripts"
import "../../lib/io"
import "../../lib/testing/expect"
import "../contracts/types"

fun main() {
    val deployer = testing.treasury("deployer");
    val init = ContractState {
        code: build("counter"),
        data: Storage { id: 0, counter: 0 }.toCell(),
    };
    val counterAddress = AutoDeployAddress { stateInit: init }.calculateAddress();

    val deployRes = net.send(deployer.address, createMessage({
        bounce: false,
        value: ton("1.0"),
        dest: { stateInit: init },
    }));
    expect(deployRes).toHaveSuccessfulDeploy({ to: counterAddress });

    val increaseRes = net.send(deployer.address, createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: counterAddress,
        body: IncreaseCounter { increaseBy: 42 },
    }));
    expect(increaseRes).toHaveTx({ to: counterAddress });
    expect(increaseRes).toHaveSuccessfulTx({
        from: deployer.address,
        to: counterAddress,
    });

    println("EXPECT_IN_SCRIPT_OK");
}
"#,
        )
        .build();

    project
        .acton()
        .script("scripts/expect_in_script.tolk")
        .run()
        .success()
        .assert_contains("EXPECT_IN_SCRIPT_OK");
}

#[test]
fn test_script_predicate_transaction_matchers_snapshot() {
    let project = ProjectBuilder::new("script-predicate-matchers-snapshot")
        .file(
            "contracts/types",
            r"
struct Storage {
    id: uint32
    counter: uint32
}

fun Storage.load(): Storage {
    return Storage.fromCell(contract.getData());
}

fun Storage.save(self) {
    contract.setData(self.toCell());
}

struct (0x7e8764ef) IncreaseCounter {
    increaseBy: uint32
}
",
        )
        .contract(
            "counter",
            r#"
import "types"

contract Counter {
    storage: Storage
    incomingMessages: IncreaseCounter
}

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }
    val msg = lazy IncreaseCounter.fromSlice(in.body);
    var storage = lazy Storage.load();
    storage.counter += msg.increaseBy;
    storage.save();
}

fun onBouncedMessage(_in: InMessageBounced) {}
"#,
        )
        .script_file(
            "predicate_snapshot",
            r#"
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/io"
import "../../lib/testing/expect"
import "../contracts/types"

fun main() {
    val deployer = testing.treasury("deployer");
    val init = ContractState {
        code: build("counter"),
        data: Storage { id: 0, counter: 0 }.toCell(),
    };
    val counterAddress = AutoDeployAddress { stateInit: init }.calculateAddress();

    val deployRes = net.send(deployer.address, createMessage({
        bounce: false,
        value: ton("1.0"),
        dest: { stateInit: init },
    }));
    expect(deployRes).toHaveSuccessfulDeploy({ to: counterAddress });

    val expectedBody = IncreaseCounter { increaseBy: 42 }.toCell();
    val increaseRes = net.send(deployer.address, createMessage({
        bounce: true,
        value: ton("0.1"),
        dest: counterAddress,
        body: IncreaseCounter { increaseBy: 42 },
    }));

    val found = increaseRes.findTransaction({
        from: fun(addr: address): bool {
            println("script.from={}", addr);
            return addr == deployer.address;
        },
        to: fun(addr: address): bool {
            println("script.to={}", addr);
            return addr == counterAddress;
        },
        value: fun(value: coins): bool {
            println("script.value={}", value);
            return value == ton("0.1");
        },
        bounce: fun(flag: bool): bool {
            println("script.bounce={}", flag);
            return flag;
        },
        opcode: fun(op: uint32): bool {
            println("script.opcode=0x{:x}", op);
            return op == IncreaseCounter.__getDeclaredPackPrefix();
        },
        body: fun(body: cell): bool {
            println("script.bodyHash=0x{:x}", body.hash());
            return body.hash() == expectedBody.hash();
        },
    });
    expect(found).toBeNotNull();

    expect(increaseRes).toNotHaveTx({
        to: fun(addr: address): bool {
            println("script.negated.to={}", addr);
            return false;
        },
    });

    println("SCRIPT_PREDICATES_OK");
}
"#,
        )
        .build();

    project
        .acton()
        .script("scripts/predicate_snapshot.tolk")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_script_predicate_transaction_matchers_snapshot.stdout.txt",
        );
}

#[test]
fn test_script_predicate_transaction_matchers_vm_exit_snapshot() {
    let project = ProjectBuilder::new("script-predicate-matchers-vm-exit")
        .file(
            "contracts/types",
            r"
struct Storage {
    id: uint32
    counter: uint32
}

fun Storage.load(): Storage {
    return Storage.fromCell(contract.getData());
}

fun Storage.save(self) {
    contract.setData(self.toCell());
}

struct (0x7e8764ef) IncreaseCounter {
    increaseBy: uint32
}
",
        )
        .contract(
            "counter",
            r#"
import "types"

contract Counter {
    storage: Storage
    incomingMessages: IncreaseCounter
}

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }
    val msg = lazy IncreaseCounter.fromSlice(in.body);
    var storage = lazy Storage.load();
    storage.counter += msg.increaseBy;
    storage.save();
}

fun onBouncedMessage(_in: InMessageBounced) {}
"#,
        )
        .script_file(
            "predicate_vm_exit",
            r#"
	import "../../lib/build"
	import "../../lib/emulation/network"
	import "../../lib/emulation/testing"
	import "../../lib/io"
	import "../../lib/testing/expect"
	import "../contracts/types"

fun main() {
    val deployer = testing.treasury("deployer");
    val init = ContractState {
        code: build("counter"),
        data: Storage { id: 0, counter: 0 }.toCell(),
    };
    val counterAddress = AutoDeployAddress { stateInit: init }.calculateAddress();

    val deployRes = net.send(deployer.address, createMessage({
        bounce: false,
        value: ton("1.0"),
        dest: { stateInit: init },
    }));
    expect(deployRes).toHaveSuccessfulDeploy({ to: counterAddress });

    val increaseRes = net.send(deployer.address, createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: counterAddress,
        body: IncreaseCounter { increaseBy: 42 },
    }));

    expect(increaseRes).toHaveTx({
        to: fun(addr: address): bool {
            println("script.vm-exit.to={}", addr);
            throw 777;
        },
    });
}
"#,
        )
        .build();

    let output = project
        .acton()
        .script("scripts/predicate_vm_exit.tolk")
        .run()
        .failure();

    output
        .assert_snapshot_matches(
            "integration/snapshots/test_script_predicate_transaction_matchers_vm_exit_snapshot.stdout.txt",
        )
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_script_predicate_transaction_matchers_vm_exit_snapshot.stderr.txt",
        );
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn script_broadcast_get_config_uses_remote_network() {
    let config_boc64 = mocked_config_boc64(REMOTE_GLOBAL_VERSION, REMOTE_GLOBAL_CAPABILITIES);
    let (mock_url, mock_handle, captured) =
        spawn_toncenter_v2_mock_with_capture(vec![toncenter_v2_get_config_all_ok_response(
            &config_boc64,
        )]);

    let project = ProjectBuilder::new("script-broadcast-get-config-uses-remote-network")
        .script_file(
            "show_config",
            r#"
import "../../lib/emulation/config"
import "../../lib/emulation/testing"
import "../../lib/io"

fun main() {
    val version = testing.getConfig().getGlobalVersion();
    println("remote-version={}, remote-capabilities={}", version.version, version.capabilities);
}
"#,
        )
        .build();

    append_custom_network(project.path(), "mock-v2-config", &mock_url);

    let output = project
        .acton()
        .env("ACTON_DISABLE_SYSTEM_PROXY", "1")
        .script("scripts/show_config.tolk")
        .verify_network("custom:mock-v2-config")
        .run()
        .success();

    output.assert_contains(&format!(
        "remote-version={REMOTE_GLOBAL_VERSION}, remote-capabilities={REMOTE_GLOBAL_CAPABILITIES}"
    ));

    mock_handle.join().expect("mock toncenter v2 must finish");

    let captured = captured
        .lock()
        .expect("captured toncenter requests mutex poisoned");
    assert_eq!(captured.len(), 1, "expected exactly one TonCenter request");
    assert_eq!(captured[0].method, "GET");
    assert!(
        captured[0].path.starts_with("/getConfigAll"),
        "expected getConfigAll request, got {}",
        captured[0].path
    );
}
