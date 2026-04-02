use crate::common::assertion;
use crate::support::TestOutputExt;
use crate::support::project::{Project, ProjectBuilder, TestConfig};
use crate::support::snapshots::normalize_output;
use std::fs;

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

const COUNTER_TEMPLATE_CONTRACT: &str =
    include_str!("../../src/commands/new/templates/counter/contracts/counter.tolk");
const COUNTER_TEMPLATE_TYPES: &str =
    include_str!("../../src/commands/new/templates/counter/contracts/types.tolk");
const COUNTER_TEMPLATE_WRAPPER: &str =
    include_str!("../../src/commands/new/templates/counter/tests/wrappers/Counter.tolk");
const COUNTER_TEMPLATE_TESTS: &str =
    include_str!("../../src/commands/new/templates/counter/tests/counter.test.tolk");

const COUNTER_TEMPLATE_SPLIT_UNKNOWN_MESSAGE_TESTS: &str = r#"
import "@acton/emulation/network"
import "@acton/testing/expect"
import "@acton/testing/transaction_expect"

import "@contracts/types"
import "@wrappers/Counter"

get fun `test unknown message reject`() {
    val (contract, deployer, _) = setupTest();

    val res = contract.sendAny(deployer.address, beginCell().storeInt(0x999, 32).endCell());
    expect(res).toHaveFailedTx({
        from: deployer.address,
        to: contract.address,
        exitCode: Errors.InvalidMessage as int,
    });
}

get fun `test unknown message accept`() {
    val (contract, deployer, _) = setupTest();

    val res = contract.sendAny(deployer.address, createEmptyCell());
    expect(res).toHaveSuccessfulTx({ from: deployer.address, to: contract.address });
}

fun setupTest(): (Counter, Treasury, Treasury) {
    val deployer = net.treasury("deployer");
    val notDeployer = net.treasury("not_deployer");

    val contract = Counter.fromStorage({ id: 0, counter: 0 });
    val res = contract.deploy(deployer.address, { value: ton("1") });
    expect(res).toHaveSuccessfulDeploy({ to: contract.address });

    return (contract, deployer, notDeployer);
}
"#;

fn build_counter_template_project(name: &str, test_source: &str) -> Project {
    let project = ProjectBuilder::new(name)
        .contract("counter", COUNTER_TEMPLATE_CONTRACT)
        .file("contracts/types", COUNTER_TEMPLATE_TYPES)
        .file("tests/wrappers/Counter", COUNTER_TEMPLATE_WRAPPER)
        .test_file("counter", test_source)
        .mapping("acton", "./.acton")
        .mapping("contracts", "contracts")
        .mapping("wrappers", "tests/wrappers")
        .build();
    project.acton().init().run().success();
    project
}

fn build_jetton_template_project(name: &str) -> Project {
    let project = ProjectBuilder::new(name)
        .contract_from_path(
            "jetton_minter",
            "src/commands/new/templates/jetton/contracts/jetton-minter-contract.tolk",
        )
        .contract_from_path(
            "jetton_wallet",
            "src/commands/new/templates/jetton/contracts/jetton-wallet-contract.tolk",
        )
        .file_from_path(
            "contracts/errors",
            "src/commands/new/templates/jetton/contracts/errors.tolk",
        )
        .file_from_path(
            "contracts/fees-management",
            "src/commands/new/templates/jetton/contracts/fees-management.tolk",
        )
        .file_from_path(
            "contracts/jetton-utils",
            "src/commands/new/templates/jetton/contracts/jetton-utils.tolk",
        )
        .file_from_path(
            "contracts/messages",
            "src/commands/new/templates/jetton/contracts/messages.tolk",
        )
        .file_from_path(
            "contracts/storage",
            "src/commands/new/templates/jetton/contracts/storage.tolk",
        )
        .file_from_path(
            "tests/wrappers/JettonMinter",
            "src/commands/new/templates/jetton/tests/wrappers/JettonMinter.tolk",
        )
        .file_from_path(
            "tests/wrappers/JettonWallet",
            "src/commands/new/templates/jetton/tests/wrappers/JettonWallet.tolk",
        )
        .test_file_from_path(
            "wallet",
            "src/commands/new/templates/jetton/tests/wallet.test.tolk",
        )
        .mapping("acton", "./.acton")
        .mapping("contracts", "contracts")
        .mapping("wrappers", "tests/wrappers")
        .build();
    project.acton().init().run().success();
    project
}

#[test]
fn test_coverage_basic_output() {
    let project = ProjectBuilder::new("coverage-basic")
        .contract("simple", SIMPLE_CONTRACT)
        .file(
            "code/math",
            r"
            fun add(a: int, b: int): int {
                return a + b;
            }

            fun isPositive(x: int): bool {
                if (x > 0) {
                    return true;
                } else {
                    return false;
                }
            }
        ",
        )
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"
            import "../code/math"

            get fun `test-coverage-example`() {
                val result = add(1, 2);
                expect(result).toEqual(3);

                val positive = isPositive(5);
                expect(positive).toEqual(true);

                val positive2 = isPositive(-10);
                expect(positive2).toEqual(false);
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .with_coverage()
        .with_coverage_format("text")
        .run()
        .success()
        .assert_passed(1)
        .assert_contains(" COVERAGE ")
        .assert_contains("math.tolk")
        .assert_snapshot_matches("integration/snapshots/test_coverage_basic_output.stdout.txt")
        .assert_file_snapshot_matches(
            "coverage.txt",
            "integration/snapshots/test_coverage_basic_output.txt",
        );
}

#[test]
fn test_coverage_multiple_tests() {
    let project = ProjectBuilder::new("coverage-multiple")
        .contract("simple", SIMPLE_CONTRACT)
        .file(
            "code/calculator",
            r"
            fun multiply(a: int, b: int): int {
                return a * b;
            }

            fun divide(a: int, b: int): int {
                if (b == 0) {
                    throw 100;
                }
                return a / b;
            }
        ",
        )
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"
            import "../code/calculator"

            get fun `test-multiply`() {
                val result = multiply(3, 4);
                expect(result).toEqual(12);
            }

            get fun `test-divide`() {
                val result = divide(10, 2);
                expect(result).toEqual(5);
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .with_coverage()
        .with_coverage_format("text")
        .run()
        .success()
        .assert_passed(2)
        .assert_contains(" COVERAGE ")
        .assert_contains("calculator.tolk")
        .assert_snapshot_matches("integration/snapshots/test_coverage_multiple_tests.stdout.txt")
        .assert_file_snapshot_matches(
            "coverage.txt",
            "integration/snapshots/test_coverage_multiple_tests.txt",
        );
}

#[test]
fn test_coverage_with_failing_tests() {
    let project = ProjectBuilder::new("coverage-with-failures")
        .contract("simple", SIMPLE_CONTRACT)
        .file(
            "code/validator",
            r"
            fun validate(value: int): bool {
                if (value > 0) {
                    return true;
                }
                return false;
            }
        ",
        )
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"
            import "../code/validator"

            get fun `test-passing`() {
                val result = validate(10);
                expect(result).toEqual(true);
            }

            get fun `test-failing`() {
                val result = validate(10);
                expect(result).toEqual(false); // This will fail
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .with_coverage()
        .with_coverage_format("text")
        .run()
        .failure()
        .assert_passed(1)
        .assert_failed(1)
        .assert_contains(" COVERAGE ")
        .assert_contains("validator.tolk")
        .assert_snapshot_matches(
            "integration/snapshots/test_coverage_with_failing_tests.stdout.txt",
        )
        .assert_file_snapshot_matches(
            "coverage.txt",
            "integration/snapshots/test_coverage_with_failing_tests.txt",
        );
}

#[test]
fn test_coverage_with_filter() {
    let project = ProjectBuilder::new("coverage-filtered")
        .contract("simple", SIMPLE_CONTRACT)
        .file(
            "code/helpers",
            r"
            fun double(x: int): int {
                return x * 2;
            }

            fun triple(x: int): int {
                return x * 3;
            }
        ",
        )
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"
            import "../code/helpers"

            get fun `test-unit-double`() {
                val result = double(5);
                expect(result).toEqual(10);
            }

            get fun `test-integration-triple`() {
                val result = triple(5);
                expect(result).toEqual(15);
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .with_coverage()
        .with_coverage_format("text")
        .run()
        .success()
        .assert_passed(2)
        .assert_contains(" COVERAGE ")
        .assert_contains("helpers.tolk")
        .assert_snapshot_matches("integration/snapshots/test_coverage_with_filter_all.stdout.txt")
        .assert_file_snapshot_matches(
            "coverage.txt",
            "integration/snapshots/test_coverage_with_filter_all.txt",
        );

    project
        .acton()
        .test()
        .filter("test-unit-.*")
        .with_coverage()
        .with_coverage_format("text")
        .run()
        .success()
        .assert_passed(1)
        .assert_contains(" COVERAGE ")
        .assert_contains("helpers.tolk")
        .assert_snapshot_matches("integration/snapshots/test_coverage_with_filter.stdout.txt")
        .assert_file_snapshot_matches(
            "coverage.txt",
            "integration/snapshots/test_coverage_with_filter.txt",
        );
}

#[test]
fn test_coverage_lcov_snapshot() {
    let project = ProjectBuilder::new("coverage-lcov-snapshot")
        .contract("simple", SIMPLE_CONTRACT)
        .file(
            "code/logic",
            r"
            fun and(a: bool, b: bool): bool {
                return a && b;
            }

            fun or(a: bool, b: bool): bool {
                return a || b;
            }
        ",
        )
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"
            import "../code/logic"

            get fun `test-lcov-snapshot`() {
                val result1 = and(true, true);
                expect(result1).toEqual(true);

                val result2 = or(false, true);
                expect(result2).toEqual(true);
            }
        "#,
        )
        .build();

    let lcov_path = project.path().join("lcov.info");

    let output = project
        .acton()
        .test()
        .with_coverage()
        .with_coverage_format("lcov")
        .run()
        .success();

    output
        .assert_passed(1)
        .assert_contains("LCOV file saved in lcov.info");

    let lcov_content = fs::read_to_string(&lcov_path).expect("Should read lcov.info");
    assertion().eq(
        normalize_output(lcov_content.as_str(), project.path()),
        snapbox::file!("snapshots/test_coverage_lcov_snapshot.lcov"),
    );
}

#[test]
fn test_counter_template_coverage_text_snapshots() {
    let project =
        build_counter_template_project("coverage-counter-template", COUNTER_TEMPLATE_TESTS);

    project
        .acton()
        .test()
        .with_coverage()
        .with_coverage_format("text")
        .with_coverage_file("counter-template-all.txt")
        .run()
        .success()
        .assert_passed(5)
        .assert_file_snapshot_matches(
            "counter-template-all.txt",
            "integration/snapshots/test_counter_template_coverage_all.txt",
        );

    project
        .acton()
        .test()
        .filter("test unknown message")
        .with_coverage()
        .with_coverage_format("text")
        .with_coverage_file("counter-template-unknown-only.txt")
        .run()
        .success()
        .assert_passed(1)
        .assert_file_snapshot_matches(
            "counter-template-unknown-only.txt",
            "integration/snapshots/test_counter_template_coverage_unknown_only.txt",
        );

    project
        .acton()
        .test()
        .filter("test deploy starts at zero|test increase counter|test any account can increase counter|test reset counter")
        .with_coverage()
        .with_coverage_format("text")
        .with_coverage_file("counter-template-non-branch.txt")
        .run()
        .success()
        .assert_passed(4)
        .assert_file_snapshot_matches(
            "counter-template-non-branch.txt",
            "integration/snapshots/test_counter_template_coverage_non_branch.txt",
        );
}

#[test]
fn test_counter_template_split_unknown_message_branch_text_snapshots() {
    let project = build_counter_template_project(
        "coverage-counter-template-split-unknown",
        COUNTER_TEMPLATE_SPLIT_UNKNOWN_MESSAGE_TESTS,
    );

    project
        .acton()
        .test()
        .filter("test unknown message reject")
        .with_coverage()
        .with_coverage_format("text")
        .with_coverage_file("counter-template-reject-only.txt")
        .run()
        .success()
        .assert_passed(1)
        .assert_file_snapshot_matches(
            "counter-template-reject-only.txt",
            "integration/snapshots/test_counter_template_coverage_reject_only.txt",
        );

    project
        .acton()
        .test()
        .filter("test unknown message accept")
        .with_coverage()
        .with_coverage_format("text")
        .with_coverage_file("counter-template-accept-only.txt")
        .run()
        .success()
        .assert_passed(1)
        .assert_file_snapshot_matches(
            "counter-template-accept-only.txt",
            "integration/snapshots/test_counter_template_coverage_accept_only.txt",
        );
}

#[test]
fn test_jetton_template_coverage_text_snapshots() {
    let project = build_jetton_template_project("coverage-jetton-template");

    project
        .acton()
        .test()
        .with_coverage()
        .with_coverage_format("text")
        .with_coverage_file("jetton-template-all.txt")
        .run()
        .success()
        .assert_passed(25)
        .assert_file_snapshot_matches(
            "jetton-template-all.txt",
            "integration/snapshots/test_jetton_template_coverage_all.txt",
        );

    project
        .acton()
        .test()
        .filter(
            "test minter admin should be able to mint jettons|test not a minter admin should not be able to mint jettons",
        )
        .with_coverage()
        .with_coverage_format("text")
        .with_coverage_file("jetton-template-mint-admin-pair.txt")
        .run()
        .success()
        .assert_passed(2)
        .assert_file_snapshot_matches(
            "jetton-template-mint-admin-pair.txt",
            "integration/snapshots/test_jetton_template_coverage_mint_admin_pair.txt",
        );

    project
        .acton()
        .test()
        .filter("test minter admin should be able to mint jettons")
        .with_coverage()
        .with_coverage_format("text")
        .with_coverage_file("jetton-template-mint-admin-accept-only.txt")
        .run()
        .success()
        .assert_passed(1)
        .assert_file_snapshot_matches(
            "jetton-template-mint-admin-accept-only.txt",
            "integration/snapshots/test_jetton_template_coverage_mint_admin_accept_only.txt",
        );

    project
        .acton()
        .test()
        .filter("test not a minter admin should not be able to mint jettons")
        .with_coverage()
        .with_coverage_format("text")
        .with_coverage_file("jetton-template-mint-admin-reject-only.txt")
        .run()
        .success()
        .assert_passed(1)
        .assert_file_snapshot_matches(
            "jetton-template-mint-admin-reject-only.txt",
            "integration/snapshots/test_jetton_template_coverage_mint_admin_reject_only.txt",
        );
}

#[test]
fn test_coverage_empty_no_tests() {
    let project = ProjectBuilder::new("coverage-empty")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            // No test functions
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .with_coverage()
        .run()
        .success()
        .assert_passed(0);
}

#[test]
fn test_coverage_text_custom_filename() {
    let project = ProjectBuilder::new("coverage-text-custom")
        .contract("simple", SIMPLE_CONTRACT)
        .file(
            "code/logic",
            r"
            fun and(a: bool, b: bool): bool {
                return a && b;
            }

            fun or(a: bool, b: bool): bool {
                return a || b;
            }
        ",
        )
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"
            import "../code/logic"

            get fun `test-custom-filename`() {
                val result1 = and(true, true);
                expect(result1).toEqual(true);

                val result2 = or(false, true);
                expect(result2).toEqual(true);
            }
        "#,
        )
        .build();

    let output = project
        .acton()
        .test()
        .with_coverage()
        .with_coverage_format("text")
        .with_coverage_file("my-custom-coverage.txt")
        .run()
        .success();

    output
        .assert_passed(1)
        .assert_contains("Text coverage file saved in my-custom-coverage.txt")
        .assert_file_exists("my-custom-coverage.txt")
        .assert_file_snapshot_matches(
            "my-custom-coverage.txt",
            "integration/snapshots/test_coverage_text_custom_filename.txt",
        );

    let default_path = project.path().join("coverage.txt");
    assert!(
        !default_path.exists(),
        "Default coverage.txt should not exist when custom filename is specified"
    );
}

#[test]
fn test_coverage_text_custom_filename_from_config() {
    let project = ProjectBuilder::new("coverage-text-custom")
        .contract("simple", SIMPLE_CONTRACT)
        .file(
            "code/logic",
            r"
            fun and(a: bool, b: bool): bool {
                return a && b;
            }

            fun or(a: bool, b: bool): bool {
                return a || b;
            }
        ",
        )
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"
            import "../code/logic"

            get fun `test-custom-filename`() {
                val result1 = and(true, true);
                expect(result1).toEqual(true);

                val result2 = or(false, true);
                expect(result2).toEqual(true);
            }
        "#,
        )
        .with_test_config(TestConfig {
            filter: None,
            exclude_patterns: None,
            include_patterns: None,
            reporters: None,
            debug: None,
            debug_port: None,
            backtrace: None,
            coverage: Some(true),
            coverage_format: Some("text".to_owned()),
            coverage_file: Some("my-custom-coverage.txt".to_owned()),
            junit_path: None,
            junit_merge: None,
            ..Default::default()
        })
        .build();

    let output = project.acton().test().run().success();

    output
        .assert_passed(1)
        .assert_contains("Text coverage file saved in my-custom-coverage.txt")
        .assert_file_exists("my-custom-coverage.txt")
        .assert_file_snapshot_matches(
            "my-custom-coverage.txt",
            "integration/snapshots/test_coverage_text_custom_filename.txt",
        );

    let default_path = project.path().join("coverage.txt");
    assert!(
        !default_path.exists(),
        "Default coverage.txt should not exist when custom filename is specified"
    );
}

#[test]
fn test_coverage_text_output_write_error_is_non_zero() {
    let project = ProjectBuilder::new("coverage-text-write-error")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            get fun `test-coverage-text-write-error`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .build();

    let readonly_dir = project.path().join("readonly");
    fs::create_dir(&readonly_dir).expect("Create readonly dir");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&readonly_dir).unwrap().permissions();
        perms.set_mode(0o444);
        fs::set_permissions(&readonly_dir, perms).unwrap();
    }

    project
        .acton()
        .test()
        .with_coverage()
        .with_coverage_format("text")
        .with_coverage_file("readonly/output.txt")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_coverage_text_output_write_error.stderr.txt",
        );
}

#[test]
fn test_coverage_lcov_output_write_error_is_non_zero() {
    let project = ProjectBuilder::new("coverage-lcov-write-error")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            get fun `test-coverage-lcov-write-error`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .build();

    let readonly_dir = project.path().join("readonly");
    fs::create_dir(&readonly_dir).expect("Create readonly dir");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&readonly_dir).unwrap().permissions();
        perms.set_mode(0o444);
        fs::set_permissions(&readonly_dir, perms).unwrap();
    }

    project
        .acton()
        .test()
        .with_coverage()
        .with_coverage_format("lcov")
        .with_coverage_file("readonly/output.lcov")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_coverage_lcov_output_write_error.stderr.txt",
        );
}
