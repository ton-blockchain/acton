//! Reserved integration test module for subagent BO.
//!
//! Ownership boundary for agent BO:
//! - tests/integration/test_std_agent_bo_tests.rs
//! - tests/integration/snapshots/test_std_agent_bo/**
//! - tests/integration/testdata/test_std_agent_bo/**
//! - tests/support/test_std_agent_bo/** (optional)
//!
//! Required test name prefix:
//! - bo_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const NETWORK_IMPORTS: &str = r#"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
"#;

fn run_network_failure_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{NETWORK_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("bo_random_address_reuse", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn bo_stdlib_network_random_address_reuses_identical_symbolic_name_bug() {
    run_network_failure_case(
        "bo-stdlib-network-random-address-reuse",
        r#"
get fun `test-bo-stdlib-network-random-address-reuse`() {
    val first = net.randomAddress("bo_reused_symbolic_name");
    val second = net.randomAddress("bo_reused_symbolic_name");

    // BUG: net.randomAddress should reuse a deterministic address for an identical symbolic name; expected same address, got different addresses.
    expect(second).toEqual(first);
}
"#,
        "integration/snapshots/test_std_agent_bo/bo_stdlib_network_random_address_reuses_identical_symbolic_name_bug.stdout.txt",
    );
}
