use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const CR_IMPORTS: &str = r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/io"
"#;

const CR_SIMPLE_ALPHA: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

const CR_SIMPLE_BETA: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
get fun betaMarker(): int {
    return 7;
}
";

fn run_project_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{CR_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .contract("alpha", CR_SIMPLE_ALPHA)
        .contract("beta", CR_SIMPLE_BETA)
        .test_file("cr_register_code_duplicate_name", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn register_code_cell_duplicate_name_is_hash_specific() {
    run_project_case(
        "cr-stdlib-register-code-cell-duplicate-name-hash-specific",
        r#"
get fun `test-cr-register-code-cell-duplicate-name-hash-specific`() {
    val deployer = net.treasury("cr_duplicate_name_deployer");
    val alphaCode = build("alpha");
    val betaCode = build("beta");

    net.registerCodeCell(alphaCode, "cr_duplicate_name");
    net.registerCodeCell(betaCode, "cr_duplicate_name");

    val alphaInit = ContractState {
        code: alphaCode,
        data: createEmptyCell(),
    };
    val alphaRes = net.send(
        deployer.address,
        createMessage({
            bounce: false,
            value: ton("0.2"),
            dest: {
                stateInit: alphaInit,
            },
        }),
    );
    println(alphaRes);

    val betaInit = ContractState {
        code: betaCode,
        data: createEmptyCell(),
    };
    val betaRes = net.send(
        deployer.address,
        createMessage({
            bounce: false,
            value: ton("0.3"),
            dest: {
                stateInit: betaInit,
            },
        }),
    );
    println(betaRes);
}
"#,
        "integration/snapshots/test-runner/register_code_cell_duplicate_name_is_hash_specific/register_code_cell_duplicate_name_is_hash_specific.stdout.txt",
    );
}

#[test]
fn register_code_cell_duplicate_name_last_registration_wins_for_same_hash() {
    let fixture = FixtureProject::load("basic");
    let test_path = "tests/cr_register_code_duplicate_name_precedence.test.tolk";
    fs::write(
        fixture.path().join(test_path),
        r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/io"
import "../contracts/counter_messages"

get fun `test-cr-register-code-cell-duplicate-name-precedence`() {
    val deployer = net.treasury("cr_precedence_deployer");
    val code = build("counter");

    net.registerCodeCell(code, "cr_duplicate_name_before");
    net.registerCodeCell(code, "cr_duplicate_name_after");

    val init = ContractState {
        code: code,
        data: Storage {
            id: 19,
            counter: 0,
        }.toCell(),
    };
    val txs = net.send(
        deployer.address,
        createMessage({
            bounce: false,
            value: ton("1"),
            dest: {
                stateInit: init,
            },
        }),
    );
    println(txs);
}
"#,
    )
    .expect("failed to write cr fixture test");

    fixture
        .acton()
        .test()
        .path(test_path)
        .run()
        .success()
        .assert_passed(1)
        .assert_contains("1 TON -> cr_duplicate_name_after")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/register_code_cell_duplicate_name_is_hash_specific/register_code_cell_duplicate_name_last_registration_wins_for_same_hash.stdout.txt",
        );
}
