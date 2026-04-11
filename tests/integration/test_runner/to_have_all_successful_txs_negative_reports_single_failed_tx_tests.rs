use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const TX_EXPECT_IMPORTS: &str = r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/testing/transaction_expect"
"#;

const FAILING_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {
    throw 77;
}

fun onBouncedMessage(_: InMessageBounced) {}
";

fn run_project_builder_failure(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{TX_EXPECT_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .contract("failing", FAILING_CONTRACT)
        .test_file("tx_expect_all_successful", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn to_have_all_successful_txs_negative_reports_single_failed_tx() {
    run_project_builder_failure(
        "ce-stdlib-all-successful-negative-single-fail",
        r#"
get fun `test ce all successful negative single failing tx`() {
    val init = ContractState {
        code: build("failing"),
        data: createEmptyCell(),
    };
    val address = AutoDeployAddress {
        stateInit: init,
    };

    val sender = net.treasury("sender");
    val msg = createMessage({
        bounce: false,
        value: ton("1"),
        dest: address,
        body: beginCell().storeUint(1, 32).endCell(),
    });

    val results = net.send(sender.address, msg);
    expect(results).toHaveAllSuccessfulTxs();
}
"#,
        "integration/snapshots/test-runner/to_have_all_successful_txs_negative_reports_single_failed_tx/to_have_all_successful_txs_negative_reports_single_failed_tx.stdout.txt",
    );
}

#[test]
fn to_have_all_successful_txs_negative_reports_single_failed_tx_in_fixture() {
    let fixture = FixtureProject::load("basic").with_contract_slot(1);
    let test_path = "tests/ce_to_have_all_successful_negative.test.tolk";
    let source = format!(
        r#"{TX_EXPECT_IMPORTS}
import "../contracts/counter_messages"

get fun `test ce all successful negative fixture single failing tx`() {{
    val init = ContractState {{
        code: build("counter"),
        data: Storage {{ id: 0, counter: 0 }}.toCell(),
    }};
    val counterAddress = AutoDeployAddress {{ stateInit: init }}.calculateAddress();

    val deployer = net.treasury("deployer");
    val deployMsg = createMessage({{
        bounce: false,
        value: ton("1"),
        dest: {{
            stateInit: init,
        }},
    }});
    val deployRes = net.send(deployer.address, deployMsg);
    expect(deployRes).toHaveSuccessfulDeploy({{ to: counterAddress }});

    val increaseMsg = createMessage({{
        bounce: false,
        value: ton("0.1"),
        dest: counterAddress,
        body: IncreaseCounter {{ queryId: 0, increaseBy: 1 }},
    }});
    val results = net.send(deployer.address, increaseMsg);
    expect(results).toHaveAllSuccessfulTxs();
}}
"#
    );

    fs::write(fixture.path().join(test_path), source)
        .expect("failed to write CE fixture toHaveAllSuccessfulTxs negative test");

    fixture
        .acton()
        .test()
        .path(test_path)
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/to_have_all_successful_txs_negative_reports_single_failed_tx/to_have_all_successful_txs_negative_reports_single_failed_tx_in_fixture.stdout.txt",
        );
}
