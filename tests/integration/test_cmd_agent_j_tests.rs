use crate::support::TestOutputExt;
use crate::support::project::{Project, ProjectBuilder};

const MUTATION_CONTRACT: &str = r#"
fun onInternalMessage(in: InMessage) {
    assert (in.valueCoins > 0) throw 5;
}

fun onBouncedMessage(_: InMessageBounced) {}

get fun addOne(x: int): int {
    return x + 1;
}
"#;

const PASSING_TEST: &str = r#"
import "../../lib/testing/expect"

get fun `test-always-pass`() {
    expect(1).toEqual(1);
}
"#;

fn mutation_project(name: &str) -> Project {
    ProjectBuilder::new(name)
        .contract("simple", MUTATION_CONTRACT)
        .test_file("mutation", PASSING_TEST)
        .build()
}

#[test]
fn j_test_cmd_mutate_requires_mutate_contract() {
    mutation_project("j-mutate-requires-contract")
        .acton()
        .test()
        .arg("--mutate")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_cmd_agent_j/j_test_cmd_mutate_requires_mutate_contract.stderr.txt",
        );
}

#[test]
fn j_test_cmd_mutate_fails_for_unknown_contract() {
    mutation_project("j-mutate-unknown-contract")
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("missing")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_cmd_agent_j/j_test_cmd_mutate_fails_for_unknown_contract.stderr.txt",
        );
}

#[test]
fn j_test_cmd_mutate_reports_summary() {
    mutation_project("j-mutate-summary")
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_cmd_agent_j/j_test_cmd_mutate_reports_summary.stdout.txt",
        );
}

#[test]
fn j_test_cmd_mutate_disable_rule_filters_mutants() {
    mutation_project("j-mutate-disable-rule")
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--disable-rule")
        .arg("remove_assert")
        .arg("--disable-rule")
        .arg("flip_plus")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_cmd_agent_j/j_test_cmd_mutate_disable_rule_filters_mutants.stdout.txt",
        );
}

#[test]
fn j_test_cmd_mutate_uses_disable_rules_from_config() {
    ProjectBuilder::new("j-mutate-config-disable-rules")
        .without_acton_toml()
        .contract("simple", MUTATION_CONTRACT)
        .test_file("mutation", PASSING_TEST)
        .raw_file(
            "Acton.toml",
            r#"[package]
name = "j-mutate-config-disable-rules"
description = "A test project"
version = "0.1.0"

[contracts.simple]
name = "simple"
src = "contracts/simple.tolk"

[test.mutation]
disable-rules = ["remove_assert", "flip_plus", "flip_gt_ge"]
"#,
        )
        .build()
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_cmd_agent_j/j_test_cmd_mutate_uses_disable_rules_from_config.stdout.txt",
        );
}
