use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

use std::fs;
use tycho_types::boc::Boc;
use tycho_types::cell::CellBuilder;

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
fn test_script_with_tuple_args() {
    let project = ProjectBuilder::new("script-tuple-args")
        .script_file(
            "tuple",
            r#"
            import "../../lib/io"

            fun main(t: tuple) {
                val a: int = t.get(0);
                val b: int = t.get(1);
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
                println1("a: {}", a.a);
                println1("b: {}", a.b);
                println1("c: {}", a.c);
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
                println1("a: {}", a.a);
                println1("b: {}", a.b);
                println1("c: {}", a.c);
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
                println1("a: {}", a);
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
                println1("a: {}", slice.loadUint(32));
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
                println1("a: {}", a.loadUint(32));
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
fn test_script_with_string_arg() {
    let project = ProjectBuilder::new("script-string-args")
        .script_file(
            "string",
            r#"
            import "../../lib/io"

            fun main(a: slice) {
                println1("a: {}", a);
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
fn test_script_with_long_string_arg() {
    let project = ProjectBuilder::new("script-string-args")
        .script_file(
            "string",
            r#"
            import "../../lib/io"

            fun main(a: slice) {
                println1("a: {}", a);
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
                println1("a: {}", a);
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
                println1("{:ton}", toReserve);
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
            r#"
            fun main() {
                val x = nonexistent_function();
            }
        "#,
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
            r#"
            val x = {{{;
        "#,
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
            "exit_42",
            r#"
            import "../../lib/io"

            fun main() {
                println("Exiting with code 42");
                throw 42
            }
        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/exit_42.tolk")
        .run()
        .code(42);
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

    project.acton().script("scripts/success.tolk").run().code(0);
}

// ========================================
// Snapshot Tests
// ========================================

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

            fun main() {
                println("Attempting to deploy with nonexistent wallet");
                // This should fail because wallet "nonexistent" is not defined
                val wallet = net.wallet("nonexistent");
                println1("Wallet found: {}", wallet.address);
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

[wallets.deployer]
kind = "v5r1"
workchain = 0
keys = { mnemonic-file = "mnemonic.txt" }
"#;
    fs::write(project.path().join("Acton.toml"), toml_content).unwrap();

    project
        .acton()
        .script("scripts/deploy.tolk")
        .broadcast()
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

            fun main() {
                println("Attempting to deploy with nonexistent wallet");
                // This should fail because wallet "nonexistent" is not defined
                val wallet = net.wallet("nonexistent");
                println1("Wallet found: {}", wallet.address);
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

    project
        .acton()
        .script("scripts/deploy.tolk")
        .broadcast()
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

            fun main() {
                println("Attempting to deploy with nonexistent wallet");
                // This should fail because wallet "nonexistent" is not defined
                val wallet = net.wallet("nonexistent");
                println1("Wallet found: {}", wallet.address);
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

    project
        .acton()
        .script("scripts/deploy.tolk")
        .broadcast()
        .verify_network("testnet")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_script_broadcast_with_nonexistent_wallet_empty_config.stderr.txt",
        );
}

#[test]
fn test_script_address_print_default() {
    let project = ProjectBuilder::new("script-simple")
        .script_file(
            "hello",
            r#"
            import "../../lib/io"

            fun main() {
                println(address("EQBvDB/H7FFBs0nF4ap/DBdcOrwY/rMIpNVVOR6SWYFHByMJ"));
            }
        "#,
        )
        .build();

    let output = project.acton().script("scripts/hello.tolk").run().code(0);

    output.assert_contains("kQBvDB/H7FFBs0nF4ap/DBdcOrwY/rMIpNVVOR6SWYFHB5iD");
}

#[test]
fn test_script_address_print_fork_testnet() {
    let project = ProjectBuilder::new("script-simple")
        .script_file(
            "hello",
            r#"
            import "../../lib/io"

            fun main() {
                println(address("EQBvDB/H7FFBs0nF4ap/DBdcOrwY/rMIpNVVOR6SWYFHByMJ"));
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

    output.assert_contains("kQBvDB/H7FFBs0nF4ap/DBdcOrwY/rMIpNVVOR6SWYFHB5iD");
}

#[test]
fn test_script_address_print_fork_mainnet() {
    let project = ProjectBuilder::new("script-simple")
        .script_file(
            "hello",
            r#"
            import "../../lib/io"

            fun main() {
                println(address("EQBvDB/H7FFBs0nF4ap/DBdcOrwY/rMIpNVVOR6SWYFHByMJ"));
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

    output.assert_contains("EQBvDB/H7FFBs0nF4ap/DBdcOrwY/rMIpNVVOR6SWYFHByMJ");
}

#[test]
fn test_script_address_print_broadcast_net_testnet() {
    let project = ProjectBuilder::new("script-simple")
        .script_file(
            "hello",
            r#"
            import "../../lib/io"

            fun main() {
                println(address("EQBvDB/H7FFBs0nF4ap/DBdcOrwY/rMIpNVVOR6SWYFHByMJ"));
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

    output.assert_contains("kQBvDB/H7FFBs0nF4ap/DBdcOrwY/rMIpNVVOR6SWYFHB5iD");
}

#[test]
fn test_script_address_print_broadcast_net_mainnet() {
    let project = ProjectBuilder::new("script-simple")
        .script_file(
            "hello",
            r#"
            import "../../lib/io"

            fun main() {
                println(address("EQBvDB/H7FFBs0nF4ap/DBdcOrwY/rMIpNVVOR6SWYFHByMJ"));
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

    output.assert_contains("EQBvDB/H7FFBs0nF4ap/DBdcOrwY/rMIpNVVOR6SWYFHByMJ");
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
                    println1("int: {}", i);
                }

                val b = env<bool>("TEST_BOOL");
                if (b != null) {
                    println1("bool: {}", b);
                }

                val s = env<slice>("TEST_SLICE");
                if (s != null) {
                    println1("slice: {}", s);
                }

                val a = env<address>("TEST_ADDRESS");
                if (a != null) {
                    println1("address: {}", a);
                }

                val c = env<cell>("TEST_CELL");
                if (c != null) {
                    var slice = c.beginParse();
                    println1("cell: {}", slice.loadUint(32));
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
            "EQBvDB/H7FFBs0nF4ap/DBdcOrwY/rMIpNVVOR6SWYFHByMJ",
        )
        .env("TEST_CELL", &cell_hex)
        .run()
        .success()
        .assert_contains("int: 123")
        .assert_contains("bool: true")
        .assert_contains("slice: hello")
        .assert_contains("address: kQBvDB/H7FFBs0nF4ap/DBdcOrwY/rMIpNVVOR6SWYFHB5iD")
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
                    println1("int_hex: {}", i_hex);
                }

                val b_1 = env<bool>("TEST_BOOL_1");
                if (b_1 != null) {
                    println1("bool_1: {}", b_1);
                }

                val b_false = env<bool>("TEST_BOOL_FALSE");
                if (b_false != null) {
                    println1("bool_false: {}", b_false);
                }

                val b_0 = env<bool>("TEST_BOOL_0");
                if (b_0 != null) {
                    println1("bool_0: {}", b_0);
                }

                val a_raw = env<address>("TEST_ADDRESS_RAW");
                if (a_raw != null) {
                    println1("address_raw: {}", a_raw);
                }

                val c_b64 = env<cell>("TEST_CELL_B64");
                if (c_b64 != null) {
                    var slice = c_b64.beginParse();
                    println1("cell_b64: {}", slice.loadUint(32));
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
        .assert_contains("address_raw: kQCDVtBfh+xRQbNJxeGqfwwXXDq8GP6zCKTVVTkeklmBRxCZ")
        .assert_contains("cell_b64: 456");
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
                val i = envOr<int>("TEST_INT", 42);
                println1("int: {}", i);

                val b = envOr<bool>("TEST_BOOL", false);
                println1("bool: {}", b);

                val s = envOr<slice>("TEST_SLICE", "default");
                println1("slice: {}", s);

                val a = envOr<address>("TEST_ADDRESS", address("EQBvDB/H7FFBs0nF4ap/DBdcOrwY/rMIpNVVOR6SWYFHByMJ"));
                println1("address: {}", a);
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
        .assert_contains("slice: default")
        .assert_contains("address: kQBvDB/H7FFBs0nF4ap/DBdcOrwY/rMIpNVVOR6SWYFHB5iD");
}
