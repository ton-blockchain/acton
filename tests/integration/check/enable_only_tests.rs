use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use function_name::named;

const MULTIPLE_RULES_SAMPLE: &str = r"
            global result: int; // E022
            fun onInternalMessage(in: InMessage) { // E001
                var x = 1; // E002
                _ = x;
            }
        ";

#[test]
#[named]
fn check_enable_only_e002_rules() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("main", MULTIPLE_RULES_SAMPLE)
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .arg("--enable-only")
        .arg("E001")
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/enable_only/{}.txt",
            function_name!()
        ));
}

#[test]
#[named]
fn check_enable_only_e028_rules() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("main", MULTIPLE_RULES_SAMPLE)
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .arg("--enable-only")
        .arg("E022")
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/enable_only/{}.txt",
            function_name!()
        ));
}

#[test]
#[named]
fn check_enable_only_multiple_rules() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("main", MULTIPLE_RULES_SAMPLE)
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .arg("--enable-only")
        .arg("E022,E002")
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/enable_only/{}.txt",
            function_name!()
        ));
}

#[test]
#[named]
fn check_enable_only_rejects_invalid_rule_code() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("main", MULTIPLE_RULES_SAMPLE)
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .arg("--enable-only")
        .arg("E999")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/enable_only/{}.txt",
            function_name!()
        ));
}

#[test]
#[named]
fn check_enable_only_preserves_deny_for_selected_rule() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("main", MULTIPLE_RULES_SAMPLE)
        .with_lint_level("mutable-variable-can-be-immutable", "deny")
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .arg("--enable-only")
        .arg("E002")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/enable_only/{}.txt",
            function_name!()
        ));
}

#[test]
#[named]
fn check_enable_only_unmutes_allow_for_selected_rule() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("main", MULTIPLE_RULES_SAMPLE)
        .with_lint_level("no-global-variables", "allow")
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .arg("--enable-only")
        .arg("E022")
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/enable_only/{}.txt",
            function_name!()
        ));
}
