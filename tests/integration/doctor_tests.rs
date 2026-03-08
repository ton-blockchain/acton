use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use serde_json::Value;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_doctor_plain_output_contains_resolved_paths() {
    let project = ProjectBuilder::new("doctor-plain").build();

    let output = project
        .acton()
        .arg("doctor")
        .current_dir(project.path())
        .run()
        .success();
    let stdout = output.get_stdout();

    output
        .assert_contains("Acton Doctor")
        .assert_contains("Versions")
        .assert_contains("Paths")
        .assert_contains("Acton.toml")
        .assert_contains("Stdlib")
        .assert_contains("Environment")
        .assert_contains("project_root:")
        .assert_contains("manifest_path:")
        .assert_contains("acton_dir:")
        .assert_contains("cache_dir:")
        .assert_contains("wallets:")
        .assert_contains("global_wallets:")
        .assert_contains("libraries:")
        .assert_contains("global_libraries:")
        .assert_contains("source=")
        .assert_contains("parse_ok:")
        .assert_contains("contracts:")
        .assert_contains("scripts:")
        .assert_contains("mappings:")
        .assert_contains("version:")
        .assert_contains("revision:")
        .assert_contains("HOME:")
        .assert_contains("USERPROFILE:")
        .assert_contains("CI:")
        .assert_contains("TERM:")
        .assert_contains("LANG:")
        .assert_contains("SHELL:")
        .assert_contains("NO_COLOR:")
        .assert_contains("current_dir:")
        .assert_contains("executable:");

    let expected_root = fs::canonicalize(project.path())
        .expect("failed to canonicalize project root")
        .display()
        .to_string();
    let expected_manifest = fs::canonicalize(project.path().join("Acton.toml"))
        .expect("failed to canonicalize Acton.toml path")
        .display()
        .to_string();
    assert!(
        stdout.contains(&expected_root),
        "doctor output must contain resolved project root path: {expected_root}\nstdout:\n{stdout}"
    );
    assert!(
        stdout.contains(&expected_manifest),
        "doctor output must contain resolved manifest path: {expected_manifest}\nstdout:\n{stdout}"
    );
}

#[test]
fn test_doctor_json_output_has_expected_shape() {
    let project = ProjectBuilder::new("doctor-json").build();

    let output = project
        .acton()
        .arg("doctor")
        .arg("--json")
        .current_dir(project.path())
        .run()
        .success();

    let stdout = output.get_stdout();
    let payload: Value = serde_json::from_str(stdout.trim()).expect("doctor --json must be JSON");

    let expected_root = fs::canonicalize(project.path())
        .expect("failed to canonicalize project root")
        .display()
        .to_string();
    let expected_manifest = fs::canonicalize(project.path().join("Acton.toml"))
        .expect("failed to canonicalize Acton.toml path")
        .display()
        .to_string();

    assert_eq!(
        payload["paths"]["project_root"]["path"].as_str(),
        Some(expected_root.as_str())
    );
    assert_eq!(
        payload["paths"]["manifest_path"]["path"].as_str(),
        Some(expected_manifest.as_str())
    );
    assert_eq!(
        payload["paths"]["project_root"]["exists"].as_bool(),
        Some(true)
    );
    assert_eq!(
        payload["paths"]["manifest_path"]["exists"].as_bool(),
        Some(true)
    );
    assert!(payload["paths"]["project_root"]["writable"].is_boolean());
    assert!(payload["paths"]["manifest_path"]["writable"].is_boolean());
    assert_eq!(
        payload["paths"]["project_root"]["resolution_source"].as_str(),
        Some("auto-detected")
    );
    assert_eq!(
        payload["paths"]["manifest_path"]["resolution_source"].as_str(),
        Some("auto-detected")
    );

    assert!(
        payload["versions"]["acton"].as_str().is_some(),
        "doctor --json must include versions.acton"
    );
    assert!(payload["versions"]["git_sha"].as_str().is_some());
    assert!(payload["versions"]["build_date"].as_str().is_some());
    assert!(payload["versions"]["target_triple"].as_str().is_some());
    assert!(payload["versions"]["profile"].as_str().is_some());
    assert!(payload["manifest"]["exists"].is_boolean());
    assert!(payload["manifest"]["parse_ok"].is_boolean());
    assert!(
        payload["manifest"]["contracts_count"].is_number()
            || payload["manifest"]["contracts_count"].is_null()
    );
    assert!(
        payload["manifest"]["scripts_count"].is_number()
            || payload["manifest"]["scripts_count"].is_null()
    );
    assert!(
        payload["manifest"]["mappings_count"].is_number()
            || payload["manifest"]["mappings_count"].is_null()
    );
    assert!(payload["stdlib"]["path"]["path"].as_str().is_some());
    assert!(payload["stdlib"]["path"]["exists"].is_boolean());
    assert_eq!(
        payload["stdlib"]["source"].as_str(),
        Some("embedded-bundle")
    );

    assert!(
        payload["environment"]["current_dir"].as_str().is_some(),
        "doctor --json must include environment.current_dir"
    );
    assert!(
        payload["environment"]["executable"].as_str().is_some(),
        "doctor --json must include environment.executable"
    );
    assert!(
        payload["environment"]["vars"]["home"].is_string()
            || payload["environment"]["vars"]["home"].is_null()
    );
    assert!(
        payload["environment"]["vars"]["userprofile"].is_string()
            || payload["environment"]["vars"]["userprofile"].is_null()
    );
    assert!(
        payload["environment"]["vars"]["ci"].is_string()
            || payload["environment"]["vars"]["ci"].is_null()
    );
    assert!(
        payload["environment"]["vars"]["term"].is_string()
            || payload["environment"]["vars"]["term"].is_null()
    );
    assert!(
        payload["environment"]["vars"]["lang"].is_string()
            || payload["environment"]["vars"]["lang"].is_null()
    );
    assert!(
        payload["environment"]["vars"]["shell"].is_string()
            || payload["environment"]["vars"]["shell"].is_null()
    );
    assert!(
        payload["environment"]["vars"]["no_color"].is_string()
            || payload["environment"]["vars"]["no_color"].is_null()
    );
}

#[test]
fn test_doctor_json_resolution_sources_with_project_root_flag() {
    let project = ProjectBuilder::new("doctor-sources-project-root").build();
    let runner_dir = project.path().join("runner");
    fs::create_dir_all(&runner_dir).expect("failed to create runner directory");
    let project_root_arg = project.path().display().to_string();

    let output = project
        .acton()
        .arg("--project-root")
        .arg(&project_root_arg)
        .arg("doctor")
        .arg("--json")
        .current_dir(&runner_dir)
        .run()
        .success();

    let payload: Value =
        serde_json::from_str(output.get_stdout().trim()).expect("doctor --json must be JSON");

    assert_eq!(
        payload["paths"]["project_root"]["resolution_source"].as_str(),
        Some("--project-root")
    );
    assert_eq!(
        payload["paths"]["manifest_path"]["resolution_source"].as_str(),
        Some("--project-root")
    );
}

#[test]
fn test_doctor_json_resolution_sources_with_manifest_path_flag() {
    let project = ProjectBuilder::new("doctor-sources-manifest-path").build();
    let runner_dir = tempdir().expect("failed to create runner directory");
    let manifest_path_arg = project.path().join("Acton.toml").display().to_string();

    let output = project
        .acton()
        .arg("--manifest-path")
        .arg(&manifest_path_arg)
        .arg("doctor")
        .arg("--json")
        .current_dir(runner_dir.path())
        .run()
        .success();

    let payload: Value =
        serde_json::from_str(output.get_stdout().trim()).expect("doctor --json must be JSON");

    assert_eq!(
        payload["paths"]["project_root"]["resolution_source"].as_str(),
        Some("fallback-cwd")
    );
    assert_eq!(
        payload["paths"]["manifest_path"]["resolution_source"].as_str(),
        Some("--manifest-path")
    );
}
