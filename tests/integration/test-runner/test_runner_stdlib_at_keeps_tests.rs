//! Reserved integration test module for subagent AT.
//!
//! Ownership boundary for agent AT:
//! - tests/integration/test-runner/test_runner_stdlib_at_keeps_tests.rs
//! - tests/integration/snapshots/test-runner/test_runner_stdlib_at_keeps_tests/**
//! - tests/integration/testdata/test_std_agent_at/**
//! - tests/support/test_std_agent_at/** (optional)
//!
//! Required test name prefix:
//! - at_stdlib_

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
fn keeps_stdout_and_stderr_separated_when_stderr_happens_first() {
    run_stdlib_io_case(
        "at-stdlib-stderr-first-separation",
        r#"
get fun `test-at-stdlib-stderr-first-separation`() {
    eprintln("stderr-1-first");
    println("stdout-1-after-first-stderr");
    eprintln("stderr-2-middle");
    println("stdout-2-after-middle-stderr");
    eprintln("stderr-3-last");
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_at_keeps_tests/at_stdlib_keeps_stdout_and_stderr_separated_when_stderr_happens_first.stdout.txt",
    );
}

#[test]
fn preserves_stdout_order_during_interleaved_println_and_eprintln() {
    run_stdlib_io_case(
        "at-stdlib-interleaved-stdout-order",
        r#"
get fun `test-at-stdlib-interleaved-stdout-order`() {
    println("stdout-1");
    eprintln("stderr-1");
    println("stdout-2");
    eprintln("stderr-2");
    println1("stdout-3={}", 333);
    eprintln("stderr-3");
    println("stdout-4");
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_at_keeps_tests/at_stdlib_preserves_stdout_order_during_interleaved_println_and_eprintln.stdout.txt",
    );
}
