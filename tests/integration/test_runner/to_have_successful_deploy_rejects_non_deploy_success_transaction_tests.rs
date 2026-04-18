use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const DEPLOY_EXPECT_IMPORTS: &str = r#"
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/testing/expect"
"#;

const RECEIVER_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

fn run_deploy_expect_failure(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{DEPLOY_EXPECT_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .contract("receiver", RECEIVER_CONTRACT)
        .test_file("deploy_expect", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("expect(actual).toHaveSuccessfulDeploy(expected)")
        .assert_contains("deploy=true")
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn to_have_successful_deploy_rejects_non_deploy_success_transaction() {
    run_deploy_expect_failure(
        "cg-stdlib-to-have-successful-deploy-rejects-non-deploy-success-transaction",
        r#"
get fun `test cg to have successful deploy rejects non deploy success transaction`() {
    val sender = testing.treasury("sender");

    val deployState = ContractState {
        code: build("receiver"),
        data: createEmptyCell(),
    };
    val receiverAddress = AutoDeployAddress { stateInit: deployState }.calculateAddress();

    val deployResult = net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("1"),
            dest: { stateInit: deployState },
        }),
    );
    expect(deployResult).toHaveSuccessfulDeploy({
        from: sender.address,
        to: receiverAddress,
    });

    val regularResult = net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.1"),
            dest: receiverAddress,
            body: beginCell().storeUint(0xCAFE, 16).endCell(),
        }),
    );
    expect(regularResult).toHaveSuccessfulDeploy({
        from: sender.address,
        to: receiverAddress,
    });
}
"#,
        "integration/snapshots/test-runner/to_have_successful_deploy_rejects_non_deploy_success_transaction/to_have_successful_deploy_rejects_non_deploy_success_transaction.stdout.txt",
    );
}

#[test]
fn to_have_successful_deploy_failure_format_includes_expected_search_params() {
    run_deploy_expect_failure(
        "cg-stdlib-to-have-successful-deploy-formatting-expected-search-params",
        r#"
get fun `test cg to have successful deploy formatting expected search params`() {
    val sender = testing.treasury("sender");

    val deployState = ContractState {
        code: build("receiver"),
        data: createEmptyCell(),
    };

    val wrongAddress = AutoDeployAddress {
        stateInit: {
            code: build("receiver"),
            data: beginCell().storeUint(1, 1).endCell(),
        },
    }.calculateAddress();

    val deployResult = net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("1"),
            dest: { stateInit: deployState },
        }),
    );
    expect(deployResult).toHaveSuccessfulDeploy({
        from: sender.address,
        to: wrongAddress,
    });
}
"#,
        "integration/snapshots/test-runner/to_have_successful_deploy_rejects_non_deploy_success_transaction/to_have_successful_deploy_failure_format_includes_expected_search_params.stdout.txt",
    );
}
