use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}

get fun currentCounter(): int {
    return 0;
}
";

#[test]
fn run_get_method_by_id_fails_for_undeployed_contract() {
    let snapshot_path = "integration/snapshots/test-runner/run_get_method_by_id_fails_for_undeployed_contract/run_get_method_by_id_fails_for_undeployed_contract.stdout.txt";
    ProjectBuilder::new("bg-stdlib-run-get-method-by-id-undeployed")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "run_get_method_by_id_undeployed",
            r#"
            import "../../lib/emulation/network"
import "../../lib/emulation/testing"
            import "../../lib/io"

            get fun `test bg run get method by id undeployed`() {
                val undeployed = address("EQC2jeGorIAFh2LXwsDjHfRK-GSo9UzchdIEMh24A7T7AHot");
                val value: int = net.runGetMethod(undeployed, 0x10000);
                println(value);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn run_get_method_by_id_fails_for_invalid_method_id_on_deployed_contract() {
    let fixture = FixtureProject::load("basic");
    let test_path = "tests/bg_run_get_method_by_id_invalid_id.test.tolk";
    fs::write(
        fixture.path().join(test_path),
        r#"
        import "../../lib/build"
        import "../../lib/emulation/network"
import "../../lib/emulation/testing"
        import "../../lib/io"
        import "../../lib/testing/expect"
        import "../contracts/counter_messages"

        struct Counter {
            address: address
            init: ContractState
        }

        fun Counter.fromStorage(storage: Storage): Counter {
            val init = ContractState {
                code: build("counter"),
                data: storage.toCell(),
            };
            val address = AutoDeployAddress { stateInit: init }.calculateAddress();
            return Counter { address, init }
        }

        fun setupCounter() {
            val counter = Counter.fromStorage({ id: 0, counter: 0 });
            val deployer = testing.treasury("deployer");

            val deploy = createMessage({
                bounce: false,
                value: ton("1.0"),
                dest: {
                    stateInit: counter.init,
                },
            });
            net.send(deployer.address, deploy);
            expect(testing.isDeployed(counter.address)).toBeTrue();
            return counter
        }

        get fun `test bg run get method by id invalid id`() {
            val counter = setupCounter();
            val value: int = net.runGetMethod(counter.address, 777777);
            println(value);
        }
    "#,
    )
    .expect("failed to write bg fixture test file");

    fixture
        .acton()
        .test()
        .path(test_path)
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/run_get_method_by_id_fails_for_undeployed_contract/run_get_method_by_id_fails_for_invalid_method_id_on_deployed_contract.stdout.txt",
        );
}
