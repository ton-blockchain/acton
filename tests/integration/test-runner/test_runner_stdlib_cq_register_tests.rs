//! Reserved integration test module for subagent CQ.
//!
//! Ownership boundary for agent CQ:
//! - tests/integration/test-runner/test_runner_stdlib_cq_register_tests.rs
//! - tests/integration/snapshots/test-runner/test_runner_stdlib_cq_register_tests/**
//! - tests/integration/testdata/test_std_agent_cq/**
//! - tests/support/test_std_agent_cq/** (optional)
//!
//! Required test name prefix:
//! - cq_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const NETWORK_IMPORTS: &str = r#"
import "../../lib/emulation/network"
import "../../lib/io"
"#;

fn run_cq_network_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{NETWORK_IMPORTS}\n{test_body}\n");
    let output = ProjectBuilder::new(project_name)
        .test_file("cq_register_address_rebind", &source)
        .build()
        .acton()
        .test()
        .run()
        .success();

    output
        .assert_passed(1)
        .assert_contains("0.2 TON -> cq_duplicate_name")
        .assert_contains("0.3 TON -> cq_duplicate_name")
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn register_address_duplicate_symbolic_name_rebind_visibility_in_output() {
    run_cq_network_case(
        "cq-stdlib-register-address-duplicate-symbolic-name",
        r#"
get fun `test-cq-register-address-duplicate-symbolic-name`() {
    val sender = net.treasury("cq_register_sender");
    val first = address("0:0000000000000000000000000000000000000000000000000000000000000011");
    val second = address("0:0000000000000000000000000000000000000000000000000000000000000022");

    net.registerAddress(first, "cq_duplicate_name");
    println(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("0.2"),
        dest: first,
    })));

    net.registerAddress(second, "cq_duplicate_name");
    println(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("0.3"),
        dest: second,
    })));

    println(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("0.4"),
        dest: first,
    })));
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_cq_register_tests/cq_stdlib_register_address_duplicate_symbolic_name_rebind_visibility_in_output.stdout.txt",
    );
}
