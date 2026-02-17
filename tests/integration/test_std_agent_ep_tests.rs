//! Reserved integration test module for subagent EP.
//!
//! Ownership boundary for agent EP:
//! - tests/integration/test_std_agent_ep_tests.rs
//! - tests/integration/snapshots/test_std_agent_ep/**
//! - tests/integration/testdata/test_std_agent_ep/**
//! - tests/support/test_std_agent_ep/** (optional)
//!
//! Required test name prefix:
//! - ep_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const EP_NOOP_CONTRACT: &str = r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#;

const EP_IMPORTS: &str = r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/testing/transaction_expect"

struct (0xE5000001) EpDeclaredPrefixBody {
    queryId: uint64
}

struct Harness {
    address: address
    init: ContractState
}

fun Harness.create() {
    val init = ContractState {
        code: build("ep_noop"),
        data: createEmptyCell(),
    };
    val address = AutoDeployAddress { stateInit: init }.calculateAddress();
    return Harness { address, init };
}

fun deployHarness() {
    val sender = net.treasury("sender");
    val harness = Harness.create();

    val deployMsg = createMessage({
        bounce: false,
        value: ton("1"),
        dest: { stateInit: harness.init },
    });
    val deployRes = net.send(sender.address, deployMsg);
    expect(deployRes).toHaveSuccessfulDeploy({ to: harness.address });

    return (sender, harness);
}
"#;

fn run_ep_success_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{EP_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .contract("ep_noop", EP_NOOP_CONTRACT)
        .test_file("ep_declared_pack_prefix", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn ep_stdlib_declared_pack_prefix_helpers_direct_calls_return_expected_values() {
    run_ep_success_case(
        "ep-stdlib-declared-pack-prefix-helpers-direct-calls",
        r#"
get fun `test-ep-declared-pack-prefix-helpers-direct-calls`() {
    expect(EpDeclaredPrefixBody.getDeclaredPackPrefix()).toEqual(0xE5000001);
    expect(never.getDeclaredPackPrefix()).toBeNull();
    expect(never.getDeclaredPackPrefixLen()).toBeNull();
}
"#,
        "integration/snapshots/test_std_agent_ep/ep_stdlib_declared_pack_prefix_helpers_direct_calls_return_expected_values.stdout.txt",
    );
}

#[test]
fn ep_stdlib_transaction_expect_matchers_use_default_and_typed_declared_pack_prefix_paths() {
    run_ep_success_case(
        "ep-stdlib-transaction-expect-matchers-declared-pack-prefix-paths",
        r#"
get fun `test-ep-transaction-expect-matchers-declared-pack-prefix-paths`() {
    val (sender, harness) = deployHarness();

    val msg = createMessage({
        bounce: false,
        value: ton("0.2"),
        dest: harness.address,
        body: EpDeclaredPrefixBody { queryId: 77 },
    });
    val txs = net.send(sender.address, msg);

    expect(txs).toHaveSuccessfulTx({
        from: sender.address,
        to: harness.address,
    });
    expect(txs).toHaveSuccessfulTx<EpDeclaredPrefixBody>({
        from: sender.address,
        to: harness.address,
    });
    expect(txs).toHaveTx<EpDeclaredPrefixBody>({
        from: sender.address,
        to: harness.address,
        success: true,
    });
}
"#,
        "integration/snapshots/test_std_agent_ep/ep_stdlib_transaction_expect_matchers_use_default_and_typed_declared_pack_prefix_paths.stdout.txt",
    );
}
