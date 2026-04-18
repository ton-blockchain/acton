use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const NETWORK_IMPORTS: &str = r#"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/testing/expect"
"#;

const NOOP_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

fn run_network_success_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{NETWORK_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .contract("noop", NOOP_CONTRACT)
        .test_file("di_set_now_overwrite", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn net_set_now_overwrites_timestamp_across_multiple_updates_in_project_builder() {
    run_network_success_case(
        "di-stdlib-net-set-now-overwrite-project-builder",
        r"
get fun `test di net set now overwrite project builder`() {
    testing.setNow(1700023001);
    expect(testing.getNow()).toEqual(1700023001);

    testing.setNow(1700023333);
    expect(testing.getNow()).toEqual(1700023333);

    testing.setNow(1700023111);
    expect(testing.getNow()).toEqual(1700023111);
}
",
        "integration/snapshots/test-runner/net_set_now_overwrites_timestamp_across_multiple_updates_in_project_builder/net_set_now_overwrites_timestamp_across_multiple_updates_in_project_builder.stdout.txt",
    );
}

#[test]
fn net_set_now_handles_boundary_and_followup_overwrite_in_fixture_project() {
    let fixture = FixtureProject::load("basic");
    let test_path = "tests/di_net_set_now_boundary_overwrite.test.tolk";
    let source = format!(
        r"
{NETWORK_IMPORTS}
get fun `test di net set now boundary overwrite`() {{
    testing.setNow(1);
    expect(testing.getNow()).toEqual(1);

    testing.setNow(4294967295);
    expect(testing.getNow()).toEqual(4294967295);

    testing.setNow(77);
    expect(testing.getNow()).toEqual(77);
}}
"
    );

    fs::write(fixture.path().join(test_path), source).expect("failed to write di fixture test");

    fixture
        .acton()
        .test()
        .path(test_path)
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/net_set_now_overwrites_timestamp_across_multiple_updates_in_project_builder/net_set_now_handles_boundary_and_followup_overwrite_in_fixture_project.stdout.txt",
        );
}
