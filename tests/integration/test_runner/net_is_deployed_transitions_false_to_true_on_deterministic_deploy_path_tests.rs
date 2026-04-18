use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const CP_SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}

get fun ping(): int {
    return 42;
}
";

const CP_IMPORTS: &str = r#"
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/testing/expect"
import "../../lib/types/transaction"

fun deployedCodeOrNull(addr: address): cell? {
    val state = testing.getAccountState(addr);
    if (state == null) {
        return null;
    }
    if (state.storage.state is TlbAccountStateActive) {
        return state.storage.state.stateInit.code;
    }
    return null;
}
"#;

fn with_cp_imports(body: &str) -> String {
    format!("{CP_IMPORTS}\n{body}\n")
}

#[test]
fn net_is_deployed_transitions_false_to_true_on_deterministic_deploy_path() {
    let snapshot_path = "integration/snapshots/test-runner/net_is_deployed_transitions_false_to_true_on_deterministic_deploy_path/net_is_deployed_transitions_false_to_true_on_deterministic_deploy_path.stdout.txt";
    ProjectBuilder::new("cp-stdlib-net-is-deployed-transition")
        .contract("probe", CP_SIMPLE_CONTRACT)
        .test_file(
            "is_deployed_boundary",
            &with_cp_imports(
                r#"
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

            get fun `test cp net is deployed deterministic transition`() {
                val probe = Probe.withSeed(17);
                expect(testing.isDeployed(probe.address)).toBeFalse();

                val deployer = testing.treasury("cp_deployer");
                val deployMsg = createMessage({
                    bounce: false,
                    value: ton("1"),
                    dest: {
                        stateInit: probe.init,
                    },
                });
                val deployTxs = net.send(deployer.address, deployMsg);

                expect(deployTxs).toHaveSuccessfulDeploy({ to: probe.address });
                expect(testing.isDeployed(probe.address)).toBeTrue();

                val other = Probe.withSeed(18);
                expect(testing.isDeployed(other.address)).toBeFalse();
            }
        "#,
            ),
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
fn net_is_deployed_stays_false_after_read_only_lookups_on_missing_address() {
    let snapshot_path = "integration/snapshots/test-runner/net_is_deployed_transitions_false_to_true_on_deterministic_deploy_path/net_is_deployed_stays_false_after_read_only_lookups_on_missing_address.stdout.txt";
    ProjectBuilder::new("cp-stdlib-net-is-deployed-read-only-cache-miss")
        .contract("probe", CP_SIMPLE_CONTRACT)
        .test_file(
            "is_deployed_read_only_cache_miss",
            &with_cp_imports(
                r#"
            get fun `test cp net is deployed read only cache miss`() {
                val target = randomAddress("cp_is_deployed_cache_only_target");

                expect(testing.isDeployed(target)).toBeFalse();

                expect(testing.getAccountBalance(target)).toEqual(0);
                expect(testing.isDeployed(target)).toBeFalse();

                expect(testing.getAccountState(target)).toBeNull();
                expect(testing.isDeployed(target)).toBeFalse();

                expect(testing.getShardAccount(target)).toBeNotNull();
                expect(testing.isDeployed(target)).toBeFalse();

                expect(deployedCodeOrNull(target)).toBeNull();
                expect(testing.isDeployed(target)).toBeFalse();
            }
        "#,
            ),
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
fn net_is_deployed_stays_false_for_explicit_null_shard_account() {
    let snapshot_path = "integration/snapshots/test-runner/net_is_deployed_transitions_false_to_true_on_deterministic_deploy_path/net_is_deployed_stays_false_for_explicit_null_shard_account.stdout.txt";
    ProjectBuilder::new("cp-stdlib-net-is-deployed-explicit-null-shard-account")
        .contract("probe", CP_SIMPLE_CONTRACT)
        .test_file(
            "is_deployed_null_shard_account",
            &with_cp_imports(
                r#"
            get fun `test cp net is deployed explicit null shard account`() {
                val target = randomAddress("cp_is_deployed_null_shard_target");

                testing.setShardAccount(target, null);

                expect(testing.getShardAccount(target)).toBeNotNull();
                expect(testing.getAccountState(target)).toBeNull();
                expect(testing.getAccountBalance(target)).toEqual(0);
                expect(deployedCodeOrNull(target)).toBeNull();
                expect(testing.isDeployed(target)).toBeFalse();
            }
        "#,
            ),
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
        with_cp_imports(
            r#"
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

        get fun `test cp net is deployed fixture transition`() {
            val counter = Counter.fromStorage({ id: 77, counter: 0 });
            expect(testing.isDeployed(counter.address)).toBeFalse();

            val deployer = testing.treasury("cp_fixture_deployer");
            val deployMsg = createMessage({
                bounce: false,
                value: ton("1"),
                dest: {
                    stateInit: counter.init,
                },
            });

            val deployTxs = net.send(deployer.address, deployMsg);
            expect(deployTxs).toHaveSuccessfulDeploy({ to: counter.address });
            expect(testing.isDeployed(counter.address)).toBeTrue();
        }
    "#,
        ),
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
            "integration/snapshots/test-runner/net_is_deployed_transitions_false_to_true_on_deterministic_deploy_path/net_is_deployed_transitions_false_to_true_in_fixture_project.stdout.txt",
        );
}
