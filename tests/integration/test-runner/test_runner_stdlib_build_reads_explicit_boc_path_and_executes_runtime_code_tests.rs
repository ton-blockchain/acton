use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;

const SIMPLE_RUNTIME_CONTRACT: &str = r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}

get fun ping(): int {
    return 7;
}
"#;

#[test]
fn build_reads_explicit_boc_path_and_executes_runtime_code() {
    let project = ProjectBuilder::new("aw-stdlib-build-boc-path-runtime")
        .contract_with_output("simple", SIMPLE_RUNTIME_CONTRACT, "contracts/simple.boc")
        .test_file(
            "build_boc_path_runtime",
            r#"
            import "../../lib/build/build"
            import "../../lib/emulation/network"
            import "../../lib/testing/expect"
            import "../../lib/testing/transaction_expect"

            get fun `test-aw-build-boc-path-runtime`() {
                val fromSource = build("simple");
                val fromBocPath = build("simple", "contracts/simple.boc");
                expect(fromBocPath).toEqual(fromSource);

                val sender = net.treasury("deployer");
                val init = ContractState {
                    code: fromBocPath,
                    data: createEmptyCell(),
                };
                val address = AutoDeployAddress { stateInit: init }.calculateAddress();

                val deployMsg = createMessage({
                    bounce: false,
                    value: ton("1"),
                    dest: {
                        stateInit: init,
                    },
                });
                expect(net.send(sender.address, deployMsg)).toHaveSuccessfulDeploy({ to: address });
                expect(net.runGetMethod<int>(address, "ping")).toEqual(7);
            }
        "#,
        )
        .build();

    project.acton().build().run().success();

    project
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_stdlib_build_reads_explicit_boc_path_and_executes_runtime_code_tests/build_reads_explicit_boc_path_and_executes_runtime_code.stdout.txt",
        );
}

#[test]
fn build_name_based_code_executes_in_fixture_runtime() {
    FixtureProject::load("basic")
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(2)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_stdlib_build_reads_explicit_boc_path_and_executes_runtime_code_tests/build_name_based_code_executes_in_fixture_runtime.stdout.txt",
        );
}
