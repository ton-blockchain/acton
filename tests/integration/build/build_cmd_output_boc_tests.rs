use crate::support::TestOutputExt;
use crate::support::compilation::extract_compiled_contracts;
use crate::support::project::ProjectBuilder;
use serde_json::Value;
use std::fs;
use std::path::Path;
use tycho_types::boc::Boc;

fn artifact_boc_bytes(project_path: &Path, contract_key: &str) -> Vec<u8> {
    let artifact_path = project_path
        .join("build")
        .join(format!("{contract_key}.json"));
    let artifact = fs::read_to_string(&artifact_path)
        .unwrap_or_else(|e| panic!("read {:?}: {}", artifact_path, e));
    let json: Value = serde_json::from_str(&artifact)
        .unwrap_or_else(|e| panic!("parse {:?}: {}", artifact_path, e));
    let boc64 = json["code_boc64"]
        .as_str()
        .expect("build artifact must contain `code_boc64` string");

    Boc::encode(
        Boc::decode_base64(boc64).expect("build artifact `code_boc64` must be valid base64 boc"),
    )
}

fn assert_output_matches_artifact(project_path: &Path, output_path: &str, contract_key: &str) {
    let full_output_path = project_path.join(output_path);
    let output = fs::read(&full_output_path)
        .unwrap_or_else(|e| panic!("read {:?}: {}", full_output_path, e));
    Boc::decode(output.clone()).expect("output file must contain valid boc");

    let expected = artifact_boc_bytes(project_path, contract_key);
    assert_eq!(
        output, expected,
        "output file `{}` must match build artifact `build/{}.json` code",
        output_path, contract_key
    );
}

#[test]
fn build_writes_boc_output_file_and_matches_artifact() {
    let project = ProjectBuilder::new("build-cmd-boc-output-success")
        .contract_with_output(
            "simple",
            r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#,
            "artifacts/simple.boc",
        )
        .build();

    project
        .acton()
        .build()
        .run()
        .success()
        .assert_contains("Compiling contracts")
        .assert_contains("Finished");

    assert_output_matches_artifact(project.path(), "artifacts/simple.boc", "simple");
}

#[test]
fn build_creates_missing_parent_directories_for_boc_output() {
    let project = ProjectBuilder::new("build-cmd-boc-output-nested")
        .contract_with_output(
            "simple",
            r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#,
            "nested/dir/simple.boc",
        )
        .build();

    project
        .acton()
        .build()
        .run()
        .success()
        .assert_contains("Finished");

    assert_output_matches_artifact(project.path(), "nested/dir/simple.boc", "simple");
}

#[test]
fn build_writes_boc_output_files_for_multiple_contracts() {
    let project = ProjectBuilder::new("build-cmd-boc-output-multi")
        .contract_with_output(
            "alpha",
            r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#,
            "outputs/alpha.boc",
        )
        .contract_with_output(
            "beta",
            r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#,
            "outputs/nested/beta.boc",
        )
        .contract(
            "gamma",
            r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#,
        )
        .build();

    project
        .acton()
        .build()
        .run()
        .success()
        .assert_contains("Finished");

    assert_output_matches_artifact(project.path(), "outputs/alpha.boc", "alpha");
    assert_output_matches_artifact(project.path(), "outputs/nested/beta.boc", "beta");

    assert!(
        !project.path().join("outputs/gamma.boc").exists(),
        "contracts without `output` should not emit standalone boc files"
    );
}

#[test]
fn build_overwrites_existing_boc_output_file() {
    let project = ProjectBuilder::new("build-cmd-boc-output-overwrite")
        .contract_with_output(
            "simple",
            r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#,
            "simple.boc",
        )
        .build();

    project.acton().build().run().success();
    assert_output_matches_artifact(project.path(), "simple.boc", "simple");

    let stale_bytes = b"stale-output";
    fs::write(project.path().join("simple.boc"), stale_bytes).expect("write stale output");

    project
        .acton()
        .build()
        .run()
        .success()
        .assert_contains("Finished");

    let current = fs::read(project.path().join("simple.boc")).expect("read rewritten output");
    assert_ne!(
        current, stale_bytes,
        "build should overwrite stale output file content"
    );
    assert_output_matches_artifact(project.path(), "simple.boc", "simple");
}

#[test]
fn build_overwrites_shared_boc_output_with_last_compiled_contract() {
    let project = ProjectBuilder::new("build-cmd-boc-output-shared-overwrite")
        .contract_with_output(
            "alpha",
            r#"
fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 100;
}
fun onBouncedMessage(_: InMessageBounced) {}
"#,
            "shared/output.boc",
        )
        .contract_with_output(
            "beta",
            r#"
fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 101;
}
fun onBouncedMessage(_: InMessageBounced) {}
"#,
            "shared/output.boc",
        )
        .build();

    let first_output = project.acton().build().clear_cache().run().success();
    let first_compiled = extract_compiled_contracts(&first_output.get_normalized_stdout());
    assert_eq!(
        first_compiled.len(),
        2,
        "expected two contracts compiled on first clear-cache build"
    );
    let first_last = first_compiled
        .last()
        .expect("expected last compiled contract on first run");

    let alpha_boc = artifact_boc_bytes(project.path(), "alpha");
    let beta_boc = artifact_boc_bytes(project.path(), "beta");
    assert_ne!(
        alpha_boc, beta_boc,
        "test setup expects distinct artifacts so shared output overwrite is observable"
    );
    assert_output_matches_artifact(project.path(), "shared/output.boc", first_last);

    let stale_bytes = b"stale-shared-output";
    fs::write(project.path().join("shared/output.boc"), stale_bytes)
        .expect("write stale shared output");

    let second_output = project.acton().build().clear_cache().run().success();
    let second_compiled = extract_compiled_contracts(&second_output.get_normalized_stdout());
    assert_eq!(
        second_compiled.len(),
        2,
        "expected two contracts compiled on second clear-cache build"
    );
    let second_last = second_compiled
        .last()
        .expect("expected last compiled contract on second run");

    let current = fs::read(project.path().join("shared/output.boc")).expect("read shared output");
    assert_ne!(
        current, stale_bytes,
        "build should overwrite stale shared output file content"
    );
    assert_output_matches_artifact(project.path(), "shared/output.boc", second_last);
}

#[test]
fn build_handles_file_vs_directory_boc_output_collision_with_mixed_results() {
    let project = ProjectBuilder::new("build-cmd-boc-output-file-vs-directory-collision")
        .contract_with_output(
            "flat",
            r#"
fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 100;
}
fun onBouncedMessage(_: InMessageBounced) {}
"#,
            "collision_target",
        )
        .contract_with_output(
            "nested",
            r#"
fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 101;
}
fun onBouncedMessage(_: InMessageBounced) {}
"#,
            "collision_target/nested.boc",
        )
        .contract_with_output(
            "independent",
            r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#,
            "artifacts/independent.boc",
        )
        .build();

    let output = project.acton().build().clear_cache().run().success();
    output.assert_contains("Finished");

    let compiled = extract_compiled_contracts(&output.get_normalized_stdout());
    assert_eq!(
        compiled.len(),
        3,
        "expected three contracts to compile when clear-cache is enabled"
    );

    let flat_idx = compiled
        .iter()
        .position(|contract| contract == "flat")
        .expect("flat contract should be compiled");
    let nested_idx = compiled
        .iter()
        .position(|contract| contract == "nested")
        .expect("nested contract should be compiled");

    let stderr = output.get_normalized_stderr();
    let boc_warning_count = stderr
        .matches("Warning: Failed to save cached BoC file for")
        .count();
    assert_eq!(
        boc_warning_count, 1,
        "exactly one contract output should fail for file-vs-directory collision"
    );

    assert_output_matches_artifact(project.path(), "artifacts/independent.boc", "independent");

    if flat_idx < nested_idx {
        output.assert_stderr_contains("Warning: Failed to save cached BoC file for nested");
        output.assert_stderr_contains("Failed to create directory for BoC file collision_target");
        assert_output_matches_artifact(project.path(), "collision_target", "flat");
        assert!(
            !project.path().join("collision_target/nested.boc").exists(),
            "nested output should not exist when its parent path is occupied by a file"
        );
    } else {
        output.assert_stderr_contains("Warning: Failed to save cached BoC file for flat");
        output.assert_stderr_contains("Is a directory");
        assert_output_matches_artifact(project.path(), "collision_target/nested.boc", "nested");
        assert!(
            project.path().join("collision_target").is_dir(),
            "collision target should remain a directory when nested output wins the collision"
        );
    }
}

#[test]
fn build_warns_for_multiple_contracts_sharing_blocked_boc_output_but_writes_other_outputs() {
    let project = ProjectBuilder::new("build-cmd-boc-output-shared-path-blocked")
        .contract_with_output(
            "alpha",
            r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#,
            "blocked_output",
        )
        .contract_with_output(
            "beta",
            r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#,
            "blocked_output",
        )
        .contract_with_output(
            "gamma",
            r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#,
            "artifacts/gamma.boc",
        )
        .build();
    fs::create_dir(project.path().join("blocked_output")).expect("create blocked output directory");

    let output = project.acton().build().run().success();
    output.assert_contains("Finished");
    output.assert_stderr_contains("Warning: Failed to save cached BoC file for alpha");
    output.assert_stderr_contains("Warning: Failed to save cached BoC file for beta");
    output.assert_stderr_contains("Is a directory");

    let stderr = output.get_normalized_stderr();
    let boc_warning_count = stderr
        .matches("Warning: Failed to save cached BoC file for")
        .count();
    assert_eq!(
        boc_warning_count, 2,
        "exactly two contract outputs should fail when both target a blocked directory path"
    );

    assert!(
        project.path().join("blocked_output").is_dir(),
        "blocked output path should remain a directory"
    );
    assert_output_matches_artifact(project.path(), "artifacts/gamma.boc", "gamma");
}

#[test]
fn build_warns_but_succeeds_when_boc_output_path_is_directory() {
    let project = ProjectBuilder::new("build-cmd-boc-output-directory-conflict")
        .contract_with_output(
            "simple",
            r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#,
            "blocked_output",
        )
        .build();
    fs::create_dir(project.path().join("blocked_output")).expect("create conflicting directory");

    let output = project.acton().build().run().success();
    output.assert_contains("Finished");
    output.assert_stderr_contains("Warning: Failed to save cached BoC file for simple");
    output.assert_stderr_contains("Is a directory");

    assert!(
        project.path().join("blocked_output").is_dir(),
        "conflicting path should remain a directory"
    );
}

#[test]
fn build_warns_for_dot_directory_output_path_but_writes_other_contract_outputs() {
    let project = ProjectBuilder::new("build-cmd-boc-output-dot-directory")
        .contract_with_output(
            "valid",
            r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#,
            "artifacts/valid.boc",
        )
        .contract_with_output(
            "invalid",
            r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#,
            "./",
        )
        .build();

    let output = project.acton().build().run().success();
    output.assert_contains("Finished");
    output.assert_stderr_contains("Warning: Failed to save cached BoC file for invalid");

    assert_output_matches_artifact(project.path(), "artifacts/valid.boc", "valid");
}

#[test]
fn build_warns_but_succeeds_when_boc_output_parent_is_a_file() {
    let project = ProjectBuilder::new("build-cmd-boc-output-parent-file-conflict")
        .contract_with_output(
            "simple",
            r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#,
            "blocked_parent/simple.boc",
        )
        .build();
    fs::write(project.path().join("blocked_parent"), "not-a-directory")
        .expect("create conflicting parent file");

    let output = project.acton().build().run().success();
    output.assert_contains("Finished");
    output.assert_stderr_contains("Warning: Failed to save cached BoC file for simple");
    output.assert_stderr_contains("Failed to create directory for BoC file blocked_parent");

    assert!(
        !project.path().join("blocked_parent/simple.boc").exists(),
        "boc file should not be created when output parent is a file"
    );
}

#[test]
fn build_rejects_boc_flag() {
    let project = ProjectBuilder::new("build-cmd-boc-flag-rejected")
        .contract(
            "simple",
            r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#,
        )
        .build();

    project
        .acton()
        .build()
        .arg("--boc")
        .arg("out.boc")
        .run()
        .failure()
        .assert_stderr_contains("unexpected argument '--boc'")
        .assert_stderr_contains("Usage: acton build");
}
