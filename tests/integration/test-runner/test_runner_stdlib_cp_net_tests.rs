//! Reserved for agent-cp.
//! Prefix: cp_stdlib_
//! Ownership: this file and tests/integration/snapshots/test-runner/test_runner_stdlib_cp_net_tests/**
//! Agent will add targeted stdlib integration tests here.

use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const CP_SIMPLE_CONTRACT: &str = r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}

get fun ping(): int {
    return 42;
}
"#;

#[test]
fn net_is_deployed_transitions_false_to_true_on_deterministic_deploy_path() {
    let snapshot_path = "integration/snapshots/test-runner/test_runner_stdlib_cp_net_tests/cp_stdlib_net_is_deployed_transitions_false_to_true_on_deterministic_deploy_path.stdout.txt";
    ProjectBuilder::new("cp-stdlib-net-is-deployed-transition")
        .contract("probe", CP_SIMPLE_CONTRACT)
        .test_file(
            "is_deployed_boundary",
            r#"
            import "../../lib/build/build"
            import "../../lib/emulation/network"
            import "../../lib/testing/expect"
            import "../../lib/testing/transaction_expect"

            struct Probe {
                address: address
                init: ContractState
            }

            fun Probe.withSeed(seed: int) {
                val init = ContractState {
                    code: build("probe"),
                    data: beginCell()
                        .storeInt(seed, 32)
                        .endCell(),
                };
                val address = AutoDeployAddress { stateInit: init }.calculateAddress();
                return Probe { address, init }
            }

            get fun `test-cp-net-is-deployed-deterministic-transition`() {
                val probe = Probe.withSeed(17);
                expect(net.isDeployed(probe.address)).toBeFalse();

                val deployer = net.treasury("cp_deployer");
                val deployMsg = createMessage({
                    bounce: false,
                    value: ton("1"),
                    dest: {
                        stateInit: probe.init,
                    },
                });
                val deployTxs = net.send(deployer.address, deployMsg);

                expect(deployTxs).toHaveSuccessfulDeploy({ to: probe.address });
                expect(net.isDeployed(probe.address)).toBeTrue();

                val other = Probe.withSeed(18);
                expect(net.isDeployed(other.address)).toBeFalse();
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn net_is_deployed_transitions_false_to_true_in_fixture_project() {
    let fixture = FixtureProject::load("basic");
    let test_path = "tests/cp_net_is_deployed_boundary.test.tolk";
    fs::write(
        fixture.path().join(test_path),
        r#"
        import "../../lib/build/build"
        import "../../lib/emulation/network"
        import "../../lib/testing/expect"
        import "../../lib/testing/transaction_expect"
        import "../contracts/counter_messages"

        struct Counter {
            address: address
            init: ContractState
        }

        fun Counter.fromStorage(storage: Storage) {
            val init = ContractState {
                code: build("counter"),
                data: storage.toCell(),
            };
            val address = AutoDeployAddress { stateInit: init }.calculateAddress();
            return Counter { address, init }
        }

        get fun `test-cp-net-is-deployed-fixture-transition`() {
            val counter = Counter.fromStorage({ id: 77, counter: 0 });
            expect(net.isDeployed(counter.address)).toBeFalse();

            val deployer = net.treasury("cp_fixture_deployer");
            val deployMsg = createMessage({
                bounce: false,
                value: ton("1"),
                dest: {
                    stateInit: counter.init,
                },
            });

            val deployTxs = net.send(deployer.address, deployMsg);
            expect(deployTxs).toHaveSuccessfulDeploy({ to: counter.address });
            expect(net.isDeployed(counter.address)).toBeTrue();
        }
    "#,
    )
    .expect("failed to write cp fixture test");

    fixture
        .acton()
        .test()
        .path(test_path)
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_stdlib_cp_net_tests/cp_stdlib_net_is_deployed_transitions_false_to_true_in_fixture_project.stdout.txt",
        );
}
