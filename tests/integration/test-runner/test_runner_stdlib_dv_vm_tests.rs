//! Reserved integration test module for subagent DV.
//!
//! Ownership boundary for agent DV:
//! - tests/integration/test-runner/test_runner_stdlib_dv_vm_tests.rs
//! - tests/integration/snapshots/test-runner/test_runner_stdlib_dv_vm_tests/**
//! - tests/integration/testdata/test_std_agent_dv/** (optional)
//! - tests/support/test_std_agent_dv/** (optional)
//!
//! Required test name prefix:
//! - dv_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const DV_SIMPLE_CONTRACT: &str = r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#;

const DV_IMPORTS: &str = r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/testing/transaction_expect"
import "../../lib/vm/vm"
"#;

fn run_dv_stdlib_success_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{DV_IMPORTS}\n{test_body}\n");

    ProjectBuilder::new(project_name)
        .contract("simple", DV_SIMPLE_CONTRACT)
        .test_file("dv_vm_register_library", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn vm_register_library_is_idempotent_under_repeated_same_cell_registration() {
    run_dv_stdlib_success_case(
        "dv-stdlib-vm-register-library-repeated-idempotent",
        r#"
get fun `test-dv-stdlib-vm-register-library-repeated-idempotent`() {
    val libraryCode = build("simple");
    val c7Before = vm.getC7();
    val c5Before = vm.getC5();

    vm.registerLibrary(libraryCode);
    vm.registerLibrary(libraryCode);
    vm.registerLibrary(libraryCode);

    val c7After = vm.getC7();
    val c5After = vm.getC5();
    expect(c7After).toEqual(c7Before);
    expect(c5After).toEqual(c5Before);

    val deployer = net.treasury("dv_register_library_idempotent_deployer");
    val init = ContractState {
        code: build("simple"),
        data: createEmptyCell(),
    };
    val contractAddress = AutoDeployAddress { stateInit: init }.calculateAddress();

    val deployTxs = net.send(
        deployer.address,
        createMessage({
            bounce: false,
            value: ton("0.2"),
            dest: {
                stateInit: init,
            },
        }),
    );
    expect(deployTxs).toHaveLength(1);
    expect(deployTxs).toHaveSuccessfulDeploy({ to: contractAddress });

    val followUpTxs = net.send(
        deployer.address,
        createMessage({
            bounce: false,
            value: ton("0.1"),
            dest: contractAddress,
        }),
    );
    expect(followUpTxs).toHaveLength(1);
    expect(followUpTxs).toHaveSuccessfulTx({ to: contractAddress });
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_dv_vm_tests/dv_stdlib_vm_register_library_is_idempotent_under_repeated_same_cell_registration.stdout.txt",
    );
}
