use crate::support::{ProjectBuilder, TestOutputExt};
use std::fs;

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
        .network("testnet")
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
        .network("testnet")
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
        .network("testnet")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_script_broadcast_with_nonexistent_wallet_empty_config.stderr.txt",
        );
}
