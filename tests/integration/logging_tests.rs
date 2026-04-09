use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use tempfile::TempDir;

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

#[test]
fn test_debug_log_uses_custom_dir_from_env() {
    let project = ProjectBuilder::new("logging-custom-dir")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    let custom_logs = project.path().join("custom-logs");
    let custom_logs_str = custom_logs
        .to_str()
        .expect("custom logs path must be valid UTF-8");

    project
        .acton()
        .env("ACTON_LOG_DIR", custom_logs_str)
        .build()
        .run()
        .success();

    assert!(
        custom_logs.join("debug.log").exists(),
        "debug.log should be created in ACTON_LOG_DIR"
    );
    assert!(
        !project.path().join(".acton").join("debug.log").exists(),
        "debug.log should not be created in project .acton directory"
    );
}

#[test]
fn test_logging_setup_failure_is_non_fatal() {
    let project = ProjectBuilder::new("logging-setup-failure-is-non-fatal")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    let occupied_path = project.path().join("occupied-log-path");
    std::fs::write(&occupied_path, "occupied").expect("failed to create occupied log path file");
    let occupied_path_str = occupied_path
        .to_str()
        .expect("occupied path must be valid UTF-8");

    let output = project
        .acton()
        .env("ACTON_LOG_DIR", occupied_path_str)
        .build()
        .run()
        .success();

    assert!(
        output
            .get_normalized_stderr()
            .contains("Warning: failed to initialize debug logging"),
        "expected warning about logging setup failure, got:\n{}",
        output.get_normalized_stderr()
    );
    assert!(
        output.get_normalized_stderr().contains("ACTON_LOG_DIR"),
        "expected ACTON_LOG_DIR hint in warning, got:\n{}",
        output.get_normalized_stderr()
    );
}

#[cfg(not(target_os = "windows"))]
#[test]
fn test_debug_log_defaults_to_home_dot_acton_logs() {
    let project = ProjectBuilder::new("logging-default-home")
        .contract("simple", SIMPLE_CONTRACT)
        .build();
    let home = TempDir::new().expect("failed to create HOME tempdir");
    let home_str = home.path().to_str().expect("HOME path must be valid UTF-8");

    project
        .acton()
        .env_remove("ACTON_LOG_DIR")
        .env("HOME", home_str)
        .build()
        .run()
        .success();

    let default_log = home.path().join(".acton").join("logs").join("debug.log");
    assert!(
        default_log.exists(),
        "debug.log should be created in HOME/.acton/logs"
    );
    assert!(
        !project.path().join(".acton").join("debug.log").exists(),
        "debug.log should not be created in project .acton directory"
    );
}

#[cfg(not(target_os = "windows"))]
#[test]
fn test_debug_log_falls_back_to_project_root_when_home_missing() {
    let project = ProjectBuilder::new("logging-fallback-project-root")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .env_remove("ACTON_LOG_DIR")
        .env_remove("HOME")
        .build()
        .run()
        .success();

    let fallback_log = project.path().join("build").join("logs").join("debug.log");
    assert!(
        fallback_log.exists(),
        "debug.log should be created in PROJECT_ROOT/build/logs when HOME is unavailable"
    );
}

#[cfg(windows)]
#[test]
fn test_debug_log_defaults_to_user_profile_dot_acton_logs() {
    let project = ProjectBuilder::new("logging-default-windows")
        .contract("simple", SIMPLE_CONTRACT)
        .build();
    let user_profile = TempDir::new().expect("failed to create USERPROFILE tempdir");
    let user_profile_str = user_profile
        .path()
        .to_str()
        .expect("USERPROFILE path must be valid UTF-8");

    project
        .acton()
        .env_remove("ACTON_LOG_DIR")
        .env("USERPROFILE", user_profile_str)
        .build()
        .run()
        .success();

    let default_log = user_profile
        .path()
        .join(".acton")
        .join("logs")
        .join("debug.log");
    assert!(
        default_log.exists(),
        "debug.log should be created in USERPROFILE/.acton/logs"
    );
    assert!(
        !project.path().join(".acton").join("debug.log").exists(),
        "debug.log should not be created in project .acton directory"
    );
}
