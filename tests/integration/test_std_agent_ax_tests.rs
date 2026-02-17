//! Reserved integration test module for subagent AX.
//!
//! Ownership boundary for agent AX:
//! - tests/integration/test_std_agent_ax_tests.rs
//! - tests/integration/snapshots/test_std_agent_ax/**
//! - tests/integration/testdata/test_std_agent_ax/**
//! - tests/support/test_std_agent_ax/** (optional)
//!
//! Required test name prefix:
//! - ax_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const SIMPLE_CONTRACT: &str = r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#;

#[test]
fn ax_stdlib_build_reports_missing_contract_when_name_is_unknown_and_path_is_empty() {
    ProjectBuilder::new("ax-stdlib-build-missing-contract-empty-path")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "build_missing_contract_empty_path",
            r#"
            import "../../lib/build/build"

            get fun `test-ax-build-missing-contract-empty-path`() {
                val _ = build("missing", "");
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
            "integration/snapshots/test_std_agent_ax/ax_stdlib_build_reports_missing_contract_when_name_is_unknown_and_path_is_empty.stdout.txt",
        );
}

#[test]
fn ax_stdlib_build_reports_missing_contract_when_name_and_path_are_empty() {
    ProjectBuilder::new("ax-stdlib-build-empty-contract-inputs")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "build_empty_contract_inputs",
            r#"
            import "../../lib/build/build"

            get fun `test-ax-build-empty-contract-inputs`() {
                val _ = build("", "");
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("not found in Acton.toml")
        .assert_contains("Available contracts:")
        .assert_contains("simple")
        .assert_snapshot_matches(
            "integration/snapshots/test_std_agent_ax/ax_stdlib_build_reports_missing_contract_when_name_and_path_are_empty.stdout.txt",
        );
}
