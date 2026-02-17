//! Reserved integration test module for subagent V.
//!
//! Ownership boundary for agent V:
//! - tests/integration/test_std_agent_v_tests.rs
//! - tests/integration/snapshots/test_std_agent_v/**
//! - tests/integration/testdata/test_std_agent_v/**
//! - tests/support/test_std_agent_v/** (optional)
//!
//! Required test name prefix:
//! - v_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const TEST_IMPORTS: &str = r#"
import "../../lib/io"
"#;

fn run_stdlib_io_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let test_code = format!(
        r#"
            {}

            {}
        "#,
        TEST_IMPORTS, test_body
    );

    ProjectBuilder::new(project_name)
        .test_file("stdlib_io", &test_code)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn v_stdlib_println_and_println1_support_hex_and_ton_formatters() {
    run_stdlib_io_case(
        "v-stdlib-println-and-println1-formatters",
        r#"
        get fun `test-println-and-println1-formatters`() {
            println(17);
            println1("hex={:x}", 48879);
            println1("ton={:ton}", 1000000000);
            println1("plain={}", "ok");
        }
        "#,
        "integration/snapshots/test_std_agent_v/v_stdlib_println_and_println1_support_hex_and_ton_formatters.stdout.txt",
    );
}

#[test]
fn v_stdlib_eprintln_reports_into_test_stderr_block() {
    run_stdlib_io_case(
        "v-stdlib-eprintln-stderr-path",
        r#"
        get fun `test-eprintln-stderr-path`() {
            println("stdout-before");
            eprintln("stderr-line-1");
            eprintln("stderr-line-2");
            println("stdout-after");
        }
        "#,
        "integration/snapshots/test_std_agent_v/v_stdlib_eprintln_reports_into_test_stderr_block.stdout.txt",
    );
}
