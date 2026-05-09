use crate::support::TestOutputExt;
use crate::support::project::{Project, ProjectBuilder};
use crate::support::toncenter::{
    append_custom_network, spawn_toncenter_v2_mock, toncenter_v2_error_response,
    toncenter_v2_seqno_ok_response,
};
use std::fs;
use std::path::Path;

#[cfg(unix)]
use std::time::Duration;

const DEPLOYER_MNEMONIC: &str = "cupboard match uphold miracle fog balance unknown region share hand trophy million toy narrow ability exchange first toast fresh maid report cram strong later";

fn create_empty_template_project(project: &Project, project_dir: &Path) {
    project
        .acton()
        .arg("new")
        .arg(&project_dir.display().to_string())
        .arg("--name")
        .arg("test-project")
        .arg("--description")
        .arg("test description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .run()
        .success();
}

fn write_deployer_wallet_config(project_dir: &Path) {
    fs::write(project_dir.join("mnemonic.txt"), DEPLOYER_MNEMONIC)
        .expect("failed to write mnemonic.txt");
    fs::write(
        project_dir.join("wallets.toml"),
        r#"[wallets.deployer]
kind = "v4r2"
workchain = 0
keys = { mnemonic-file = "mnemonic.txt" }
"#,
    )
    .expect("failed to write wallets.toml");
}

// Source scenario: tests/scenarios/common_user_flows.yaml
// Scenario id: create_project_and_run_tests
#[cfg(unix)]
#[test]
fn create_project_and_run_tests() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("scenario-create-project-and-run-tests")
        .without_acton_toml()
        .build();
    let project_dir = project.path().join("foobar");

    let mut session = project
        .acton()
        .arg("new")
        .arg(&project_dir.display().to_string())
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(20)));

    session.expect("Project name:");
    session.send_line("foobar", "failed to enter project name");
    session.expect("Template:");
    session.send_line("", "failed to accept default template");
    session.expect("Include the TypeScript dApp?");
    session.send_line("", "failed to keep default no-app choice");
    session.expect("Do you want to configure advanced options (Git hooks, license, etc.)?");
    session.send_line("", "failed to keep default no-advanced choice");
    session.expect("Created new Acton project");
    session.expect("Project name: foobar");
    session.expect("Description: A TON blockchain project");
    session.expect("Template: empty");
    session.expect("License: MIT");
    session.expect("acton build");
    session.expect("acton test");
    session.expect(Eof);

    project
        .acton()
        .current_dir(&project_dir)
        .arg("test")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/scenarios/common_user_flows/scenario_common_user_flows_create_project_and_run_tests.stdout.txt",
        )
        .assert_file_exists("foobar/Acton.toml")
        .assert_file_exists("foobar/contracts/Empty.tolk")
        .assert_file_exists("foobar/contracts/types.tolk")
        .assert_file_exists("foobar/tests/contract.test.tolk")
        .assert_file_exists("foobar/wrappers/Empty.gen.tolk")
        .assert_file_exists("foobar/scripts/deploy.tolk")
        .assert_file_exists("foobar/README.md")
        .assert_file_exists("foobar/.github/workflows/ci.yml");
}

// Source scenario: tests/scenarios/common_user_flows.yaml
// Scenario id: deploy_script_fails_when_toncenter_is_unavailable
#[test]
fn deploy_script_fails_when_toncenter_is_unavailable() {
    let project = ProjectBuilder::new("scenario-toncenter-unavailable")
        .without_acton_toml()
        .build();
    let project_dir = project.path().join("remote-project");

    create_empty_template_project(&project, &project_dir);
    write_deployer_wallet_config(&project_dir);

    let (mock_url, mock_handle) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_seqno_ok_response(),
        toncenter_v2_error_response(500, "temporary server error"),
    ]);
    append_custom_network(&project_dir, "toncenter-down", &mock_url);

    let output = project
        .acton()
        .script("scripts/deploy.tolk")
        .current_dir(&project_dir)
        .verify_network("custom:toncenter-down")
        .run()
        .failure();

    mock_handle.join().expect("mock toncenter v2 must finish");
    output.assert_snapshot_matches(
        "integration/snapshots/scenarios/common_user_flows/scenario_common_user_flows_deploy_script_fails_when_toncenter_is_unavailable.stdout.txt",
    );
}
