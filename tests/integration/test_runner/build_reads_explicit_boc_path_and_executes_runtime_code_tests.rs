use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const SIMPLE_RUNTIME_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}

get fun ping(): int {
    return 7;
}
";

fn compiled_runtime_boc_bytes() -> Vec<u8> {
    let source_project = ProjectBuilder::new("aw-stdlib-build-precompiled-source")
        .contract_with_output("simple", SIMPLE_RUNTIME_CONTRACT, "contracts/simple.boc")
        .build();

    source_project.acton().build().run().success();

    fs::read(source_project.path().join("contracts/simple.boc"))
        .expect("must read compiled boc bytes")
}

#[test]
fn build_reads_explicit_boc_path_and_executes_runtime_code() {
    let project = ProjectBuilder::new("aw-stdlib-build-boc-path-runtime")
        .contract_with_output("simple", SIMPLE_RUNTIME_CONTRACT, "contracts/simple.boc")
        .test_file(
            "build_boc_path_runtime",
            r#"
            import "../../lib/build"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"
            import "../../lib/testing/expect"

            get fun `test aw build boc path runtime`() {
                val fromSource = build("simple");
                val fromBocPath = build("simple", "contracts/simple.boc");
                expect(fromBocPath).toEqual(fromSource);

                val sender = testing.treasury("deployer");
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
            "integration/snapshots/test-runner/build_reads_explicit_boc_path_and_executes_runtime_code/build_reads_explicit_boc_path_and_executes_runtime_code.stdout.txt",
        );
}

#[test]
fn build_reads_name_based_precompiled_boc_contract_and_executes_runtime_code() {
    let boc_bytes = compiled_runtime_boc_bytes();
    let project = ProjectBuilder::new("aw-stdlib-build-precompiled-boc-runtime")
        .contract_from_boc("precompiled", boc_bytes)
        .test_file(
            "build_precompiled_boc_runtime",
            r#"
            import "../../lib/build"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"
            import "../../lib/testing/expect"

            get fun `test aw build precompiled boc runtime`() {
                val fromName = build("precompiled");
                val fromBocPath = build("precompiled", "contracts/precompiled.boc");
                expect(fromBocPath).toEqual(fromName);

                val sender = testing.treasury("deployer");
                val init = ContractState {
                    code: fromName,
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

    project
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/build_reads_explicit_boc_path_and_executes_runtime_code/build_reads_name_based_precompiled_boc_contract_and_executes_runtime_code.stdout.txt",
        );
}

#[test]
fn build_reports_missing_explicit_boc_path() {
    ProjectBuilder::new("ax-stdlib-build-missing-boc-path")
        .contract("dummy", SIMPLE_RUNTIME_CONTRACT)
        .test_file(
            "build_missing_boc_path",
            r#"
            import "../../lib/build"

            get fun `test ax build missing boc path`() {
                val _ = build("missing_boc", "contracts/missing.boc");
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("Cannot read BoC file")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/build_reads_explicit_boc_path_and_executes_runtime_code/build_reports_missing_explicit_boc_path.stdout.txt",
        );
}

#[test]
fn build_reports_invalid_explicit_boc_path() {
    ProjectBuilder::new("ax-stdlib-build-invalid-boc-path")
        .contract("dummy", SIMPLE_RUNTIME_CONTRACT)
        .raw_file("contracts/invalid.boc", "not a boc payload")
        .test_file(
            "build_invalid_boc_path",
            r#"
            import "../../lib/build"

            get fun `test ax build invalid boc path`() {
                val _ = build("invalid_boc", "contracts/invalid.boc");
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("Failed to decode code BoC")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/build_reads_explicit_boc_path_and_executes_runtime_code/build_reports_invalid_explicit_boc_path.stdout.txt",
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
            "integration/snapshots/test-runner/build_reads_explicit_boc_path_and_executes_runtime_code/build_name_based_code_executes_in_fixture_runtime.stdout.txt",
        );
}
