//! Reserved for agent-cw.
//! Prefix: cw_stdlib_
//! Ownership: this file and tests/integration/snapshots/test_std_agent_cw/**
//! Agent will add targeted stdlib integration tests here.

use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const CW_LOAD_BODY_IMPORTS: &str = r#"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/types/out_actions"
import "../../lib/vm/vm"

struct (0xC0DEAA01) CwLoadBodyActual {
    queryId: uint64
    amount: uint32
}

struct (0xC0DEAA02) CwLoadBodyExpected {
    queryId: uint64
    amount: uint32
}
"#;

fn run_cw_project_builder_load_body_mismatch_failure(
    project_name: &str,
    test_body: &str,
    snapshot_path: &str,
) {
    let source = format!("{CW_LOAD_BODY_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("cw_load_body_mismatch", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("exit_code=63")
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn cw_stdlib_out_action_send_message_load_body_mismatched_type_reports_exit_code63_diagnostic() {
    run_cw_project_builder_load_body_mismatch_failure(
        "cw-stdlib-out-action-load-body-mismatch-diagnostic",
        r#"
get fun `test-cw-out-action-load-body-mismatch-diagnostic`() {
    val dest = net.randomAddress("cw_load_body_mismatch_dest");
    createMessage({
        bounce: false,
        value: ton("1"),
        dest,
        body: CwLoadBodyActual {
            queryId: 77,
            amount: 33,
        },
    }).send(SEND_MODE_REGULAR);

    val outActions = vm.outActions();
    expect(outActions.size()).toEqual(1);
    val action = outActions.getSendMessageAt(0);
    expect(action).toBeNotNull();

    action!.loadBody<CwLoadBodyExpected>();
}
"#,
        "integration/snapshots/test_std_agent_cw/cw_stdlib_out_action_send_message_load_body_mismatched_type_reports_exit_code63_diagnostic.stdout.txt",
    );
}

#[test]
fn cw_stdlib_out_action_send_message_load_body_mismatched_type_reports_exit_code63_diagnostic_in_fixture_project(
) {
    let fixture = FixtureProject::load("basic");
    let test_path = "tests/cw_out_action_load_body_mismatch.test.tolk";
    let source = format!(
        r#"{CW_LOAD_BODY_IMPORTS}
get fun `test-cw-out-action-load-body-mismatch-diagnostic-fixture`() {{
    val dest = net.randomAddress("cw_load_body_mismatch_fixture_dest");
    createMessage({{
        bounce: false,
        value: ton("1.5"),
        dest,
        body: CwLoadBodyActual {{
            queryId: 88,
            amount: 44,
        }},
    }}).send(SEND_MODE_PAY_FEES_SEPARATELY);

    val outActions = vm.outActions();
    expect(outActions.size()).toEqual(1);
    val action = outActions.getSendMessageAt(0);
    expect(action).toBeNotNull();

    action!.loadBody<CwLoadBodyExpected>();
}}
"#
    );

    fs::write(fixture.path().join(test_path), source).expect("failed to write cw fixture test");

    fixture
        .acton()
        .test()
        .path(test_path)
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("exit_code=63")
        .assert_snapshot_matches(
            "integration/snapshots/test_std_agent_cw/cw_stdlib_out_action_send_message_load_body_mismatched_type_reports_exit_code63_diagnostic_in_fixture_project.stdout.txt",
        );
}
