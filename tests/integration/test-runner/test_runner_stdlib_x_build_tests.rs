//! Reserved integration test module for subagent X.
//!
//! Ownership boundary for agent X:
//! - tests/integration/test-runner/test_runner_stdlib_x_build_tests.rs
//! - tests/integration/snapshots/test-runner/test_runner_stdlib_x_build_tests/**
//! - tests/integration/testdata/test_std_agent_x/**
//! - tests/support/test_std_agent_x/** (optional)
//!
//! Required test name prefix:
//! - x_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const SIMPLE_CONTRACT: &str = r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#;

#[test]
fn build_accepts_contract_name_and_explicit_path() {
    ProjectBuilder::new("x-stdlib-build-by-name-and-path")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "build_paths",
            r#"
            import "../../lib/build/build"
            import "../../lib/testing/expect"

            get fun `test-build-by-name-and-path`() {
                val byName = build("simple");
                val byPath = build("simple", "contracts/simple.tolk");

                expect(byName).toEqual(byPath);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_stdlib_x_build_tests/x_stdlib_build_accepts_contract_name_and_explicit_path.stdout.txt",
        );
}

#[test]
fn build_reports_missing_contract_when_path_is_omitted() {
    ProjectBuilder::new("x-stdlib-build-contract-not-found")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "build_missing",
            r#"
            import "../../lib/build/build"

            get fun `test-build-contract-not-found`() {
                val _ = build("missing");
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("Contract missing not found in Acton.toml")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_stdlib_x_build_tests/x_stdlib_build_reports_missing_contract_when_path_is_omitted.stdout.txt",
        );
}
