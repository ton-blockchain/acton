use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use function_name::named;
use std::fs;

const UNAUTHORIZED_ACCESS_SAMPLE: &str = r"
            fun onInternalMessage(in: InMessage) {
                val _sender = in.senderAddress;
                contract.setData(contract.getData());
            }
        ";

#[test]
#[named]
fn check_lint_rules_warn_enables_rule_diagnostics() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("main", UNAUTHORIZED_ACCESS_SAMPLE)
        .with_lint_level("missing-contract-header", "allow")
        .with_lint_level("unauthorized-access", "warn")
        .with_lint_level("explicit-return-type", "allow")
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/lint_rules_config/{}.txt",
            function_name!()
        ));
}

#[test]
#[named]
fn check_lint_rules_allow_disables_rule_diagnostics() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("main", UNAUTHORIZED_ACCESS_SAMPLE)
        .with_lint_level("missing-contract-header", "allow")
        .with_lint_level("unauthorized-access", "allow")
        .with_lint_level("explicit-return-type", "allow")
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/lint_rules_config/{}.txt",
            function_name!()
        ));
}

#[test]
#[named]
fn check_lint_rules_contract_override_applies_to_single_contract() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("alpha", UNAUTHORIZED_ACCESS_SAMPLE)
        .contract("beta", UNAUTHORIZED_ACCESS_SAMPLE)
        .with_lint_level("missing-contract-header", "allow")
        .with_lint_level("unauthorized-access", "warn")
        .with_lint_level("explicit-return-type", "allow")
        .build();

    let acton_toml_path = project.path().join("Acton.toml");
    let mut acton_toml = fs::read_to_string(&acton_toml_path).expect("failed to read Acton.toml");
    acton_toml.push_str("\n[lint.rules.beta]\nunauthorized-access = \"allow\"\n");
    fs::write(&acton_toml_path, acton_toml).expect("failed to patch Acton.toml");

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/lint_rules_config/{}.txt",
            function_name!()
        ));
}
