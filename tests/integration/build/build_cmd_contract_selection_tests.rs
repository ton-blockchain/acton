use crate::support::TestOutputExt;
use crate::support::compilation::extract_compiled_contracts;
use crate::support::project::ProjectBuilder;

#[test]
fn build_named_contract_selection_includes_dependencies_only() {
    let project = ProjectBuilder::new("build-cmd-contract-select-deps")
        .contract(
            "base",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract(
            "independent",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_deps(
            "target",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["base"],
        )
        .build();

    let output = project.acton().build().contract("target").run().success();
    let stdout = output.get_normalized_stdout();
    let compiled = extract_compiled_contracts(&stdout);

    assert_eq!(compiled, vec!["base".to_string(), "target".to_string()]);
    assert!(project.path().join("build/base.json").exists());
    assert!(project.path().join("build/target.json").exists());
    assert!(!project.path().join("build/independent.json").exists());

    output.assert_snapshot_matches(
        "integration/snapshots/build/build_cmd_contract_selection_tests/build_named_contract_selection_includes_dependencies_only.stdout.txt",
    );
}

#[test]
fn build_named_contract_selection_without_dependencies_builds_only_target() {
    let project = ProjectBuilder::new("build-cmd-contract-select-single")
        .contract(
            "alpha",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract(
            "beta",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract(
            "gamma",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .build();

    let output = project.acton().build().contract("beta").run().success();
    let stdout = output.get_normalized_stdout();
    let compiled = extract_compiled_contracts(&stdout);

    assert_eq!(compiled, vec!["beta".to_string()]);
    assert!(project.path().join("build/beta.json").exists());
    assert!(!project.path().join("build/alpha.json").exists());
    assert!(!project.path().join("build/gamma.json").exists());

    output.assert_snapshot_matches(
        "integration/snapshots/build/build_cmd_contract_selection_tests/build_named_contract_selection_without_dependencies_builds_only_target.stdout.txt",
    );
}

#[test]
fn build_named_contract_selection_nonexistent_contract_fails() {
    let project = ProjectBuilder::new("build-cmd-contract-select-missing")
        .contract(
            "existing",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .build();

    project
        .acton()
        .build()
        .contract("nonexistent")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/build/build_cmd_contract_selection_tests/build_named_contract_selection_nonexistent_contract_fails.stderr.txt",
        );
}

#[test]
fn build_named_contract_selection_accepts_normalized_name_for_mixed_case_and_hyphen_name() {
    let project = ProjectBuilder::new("build-cmd-contract-select-normalized-name")
        .contract(
            "My-Contract",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract(
            "Other",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .build();

    let output = project
        .acton()
        .build()
        .contract("my_contract")
        .run()
        .success();
    let stdout = output.get_normalized_stdout();
    let compiled = extract_compiled_contracts(&stdout);

    assert_eq!(compiled, vec!["My-Contract".to_string()]);
    assert!(project.path().join("build/my_contract.json").exists());
    assert!(!project.path().join("build/other.json").exists());

    output.assert_snapshot_matches(
        "integration/snapshots/build/build_cmd_contract_selection_tests/build_named_contract_selection_accepts_normalized_name_for_mixed_case_and_hyphen_name.stdout.txt",
    );
}

#[test]
fn build_named_contract_selection_nonexistent_contract_lists_normalized_available_names() {
    let project = ProjectBuilder::new("build-cmd-contract-select-missing-multi")
        .contract(
            "Zeta",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract(
            "Beta-Contract",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract(
            "alpha",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .build();

    project
        .acton()
        .build()
        .contract("missing_contract")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/build/build_cmd_contract_selection_tests/build_named_contract_selection_nonexistent_contract_lists_normalized_available_names.stderr.txt",
        );
}

#[test]
fn build_named_contract_selection_treats_option_like_value_after_double_dash_as_contract_name() {
    let project = ProjectBuilder::new("build-cmd-contract-select-double-dash-option-like")
        .contract(
            "existing",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .build();

    project
        .acton()
        .build()
        .arg("--")
        .arg("--contract")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/build/build_cmd_contract_selection_tests/build_named_contract_selection_treats_option_like_value_after_double_dash_as_contract_name.stderr.txt",
        );
}

#[test]
fn build_named_contract_selection_rejects_multiple_positional_contract_names() {
    let project = ProjectBuilder::new("build-cmd-contract-select-extra-positional")
        .contract(
            "alpha",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract(
            "beta",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .build();

    project
        .acton()
        .build()
        .arg("alpha")
        .arg("beta")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/build/build_cmd_contract_selection_tests/build_named_contract_selection_rejects_multiple_positional_contract_names.stderr.txt",
        );

    assert!(
        !project.path().join("build/alpha.json").exists(),
        "parse failure should happen before any build artifact is created"
    );
    assert!(
        !project.path().join("build/beta.json").exists(),
        "parse failure should happen before any build artifact is created"
    );
}

#[test]
fn build_contract_flag_contract_is_rejected() {
    let project = ProjectBuilder::new("build-cmd-contract-flag")
        .contract(
            "simple",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .build();

    project
        .acton()
        .build()
        .arg("--contract")
        .arg("simple")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/build/build_cmd_contract_selection_tests/build_contract_flag_contract_is_rejected.stderr.txt",
        );
}

#[test]
fn build_contract_flag_does_not_override_positional_contract_selection() {
    let project = ProjectBuilder::new("build-cmd-contract-flag-positional-precedence")
        .contract(
            "alpha",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract(
            "beta",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .build();

    project
        .acton()
        .build()
        .contract("alpha")
        .arg("--contract")
        .arg("beta")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/build/build_cmd_contract_selection_tests/build_contract_flag_does_not_override_positional_contract_selection.stderr.txt",
        );

    assert!(
        !project.path().join("build/alpha.json").exists(),
        "parse failure should happen before any build artifact is created"
    );
    assert!(
        !project.path().join("build/beta.json").exists(),
        "parse failure should happen before any build artifact is created"
    );
}

#[test]
fn build_named_contract_selection_stays_filtered_with_broad_build_flags() {
    let project = ProjectBuilder::new("build-cmd-contract-select-broad-flags")
        .contract(
            "base",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract(
            "independent",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_deps(
            "target",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["base"],
        )
        .build();

    let output = project
        .acton()
        .build()
        .clear_cache()
        .with_out_dir("custom-build")
        .with_info()
        .contract("target")
        .run()
        .success();

    let stdout = output.get_normalized_stdout();
    let compiled = extract_compiled_contracts(&stdout);

    assert_eq!(compiled, vec!["base".to_string(), "target".to_string()]);
    assert!(project.path().join("custom-build/base.json").exists());
    assert!(project.path().join("custom-build/target.json").exists());
    assert!(
        !project
            .path()
            .join("custom-build/independent.json")
            .exists()
    );

    assert!(stdout.contains("Artifacts of base"));
    assert!(stdout.contains("Artifacts of target"));
    assert!(!stdout.contains("Artifacts of independent"));

    output.assert_snapshot_matches(
        "integration/snapshots/build/build_cmd_contract_selection_tests/build_named_contract_selection_stays_filtered_with_broad_build_flags.stdout.txt",
    );
}
