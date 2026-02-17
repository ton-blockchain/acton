//! Reserved integration test module for subagent AS.
//!
//! Ownership boundary for agent AS:
//! - tests/integration/test_std_agent_as_tests.rs
//! - tests/integration/snapshots/test_std_agent_as/**
//! - tests/integration/testdata/test_std_agent_as/**
//! - tests/support/test_std_agent_as/** (optional)
//!
//! Required test name prefix:
//! - as_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const TEST_IMPORTS: &str = r#"
import "../../lib/io"
"#;

fn run_stdlib_io_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let test_code = format!(
        r#"
{TEST_IMPORTS}

{test_body}
"#
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
fn as_stdlib_println1_formats_negative_hex_and_ton_values() {
    run_stdlib_io_case(
        "as-stdlib-println1-negative-formatters",
        r#"
get fun `test-as-stdlib-println1-negative-formatters`() {
    println1("hex_neg_one={:x}", -1);
    println1("hex_neg_255={:x}", -255);
    println1("ton_neg_nano={:ton}", -1);
    println1("ton_neg_one_and_half={:ton}", -1500000000);
}
"#,
        "integration/snapshots/test_std_agent_as/as_stdlib_println1_formats_negative_hex_and_ton_values.stdout.txt",
    );
}

#[test]
fn as_stdlib_println1_formats_edge_hex_and_ton_values() {
    run_stdlib_io_case(
        "as-stdlib-println1-edge-formatters",
        r#"
get fun `test-as-stdlib-println1-edge-formatters`() {
    println1("hex_zero={:x}", 0);
    println1("hex_i64_max={:x}", 9223372036854775807);
    println1("ton_zero={:ton}", 0);
    println1("ton_i64_max={:ton}", 9223372036854775807);
}
"#,
        "integration/snapshots/test_std_agent_as/as_stdlib_println1_formats_edge_hex_and_ton_values.stdout.txt",
    );
}
