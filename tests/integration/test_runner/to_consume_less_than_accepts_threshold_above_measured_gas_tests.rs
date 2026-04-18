use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const CF_IMPORTS: &str = r#"
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/testing/expect"
"#;

const CF_RECEIVER_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

fn run_success_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{CF_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .contract("receiver", CF_RECEIVER_CONTRACT)
        .test_file("tx_expect_to_consume_less_than", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn to_consume_less_than_accepts_threshold_above_measured_gas() {
    run_success_case(
        "cf-stdlib-to-consume-less-than-pass",
        r#"
fun deployReceiver() {
    val sender = testing.treasury("sender");

    val stateInit = ContractState {
        code: build("receiver"),
        data: createEmptyCell(),
    };
    val receiverAddress = AutoDeployAddress { stateInit }.calculateAddress();

    val deployMsg = createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit,
        },
    });
    val deployRes = net.send(sender.address, deployMsg);
    expect(deployRes).toHaveSuccessfulDeploy({ to: receiverAddress });

    return (sender, receiverAddress);
}

get fun `test cf stdlib to consume less than pass`() {
    val (sender, receiverAddress) = deployReceiver();

    val payload = beginCell().storeUint(0xCF, 8).storeUint(1, 8).endCell();
    val txs = net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.2"),
            dest: receiverAddress,
            body: payload,
        }),
    );

    expect(txs).toHaveSuccessfulTx({
        from: sender.address,
        to: receiverAddress,
    });

    val rootTx = txs.at(0);
    expect(rootTx).toConsumeLessThan(rootTx.gasUsed + 1);
}
"#,
        "integration/snapshots/test-runner/to_consume_less_than_accepts_threshold_above_measured_gas/to_consume_less_than_accepts_threshold_above_measured_gas.stdout.txt",
    );
}

#[test]
fn to_consume_less_than_fails_on_equal_threshold_boundary() {
    let fixture = FixtureProject::load("basic");
    let test_path = "tests/cf_to_consume_less_than_equal_threshold.test.tolk";
    fs::write(
        fixture.path().join(test_path),
        r#"
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/testing/expect"
import "../contracts/counter_messages"

get fun `test cf stdlib to consume less than equal threshold`() {
    val deployer = testing.treasury("deployer");

    val stateInit = ContractState {
        code: build("counter"),
        data: Storage { id: 1, counter: 0 }.toCell(),
    };
    val counterAddress = AutoDeployAddress { stateInit }.calculateAddress();

    val deployMsg = createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit,
        },
    });
    val deployRes = net.send(deployer.address, deployMsg);
    expect(deployRes).toHaveSuccessfulDeploy({
        from: deployer.address,
        to: counterAddress,
    });

    val increaseMsg = createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: counterAddress,
        body: IncreaseCounter {
            queryId: 7,
            increaseBy: 1,
        },
    });
    val txs = net.send(deployer.address, increaseMsg);
    expect(txs).toHaveSuccessfulTx({
        from: deployer.address,
        to: counterAddress,
    });

    val tx = txs.at(0);
    expect(tx).toConsumeLessThan(tx.gasUsed);
}
"#,
    )
    .expect("failed to write cf fixture test");

    fixture
        .acton()
        .test()
        .path(test_path)
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/to_consume_less_than_accepts_threshold_above_measured_gas/to_consume_less_than_fails_on_equal_threshold_boundary.stdout.txt",
        );
}
