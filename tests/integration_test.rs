#[cfg(test)]
mod common;
#[cfg(test)]
mod integration;
#[cfg(test)]
mod support;

use acton_config::schema::ACTON_SCHEMA_JSON;
use common::ActonCommandExt;
use std::{fs, process::Command};

const MANUAL_COMMANDS: &[&str] = &[
    "init",
    "new",
    "build",
    "help",
    "hooks",
    "compile",
    "wrapper",
    "disasm",
    "fmt",
    "retrace",
    "test",
    "check",
    "script",
    "run",
    "verify",
    "library",
    "wallet",
    "rpc",
    "localnet",
    "doc",
    "ls",
    "up",
    "doctor",
    "func2tolk",
    "completions",
];

#[test]
fn test_acton_help_long_flag() {
    snapbox::cmd::Command::acton_ui()
        .arg("--help")
        .assert()
        .success()
        .stdout_eq(snapbox::file!["snapshots/acton/stdout.txt"])
        .stderr_eq(snapbox::str![""]);
}

#[test]
fn test_acton_help_short_flag() {
    snapbox::cmd::Command::acton_ui()
        .arg("-h")
        .assert()
        .success()
        .stdout_eq(snapbox::file!["snapshots/acton/stdout.txt"])
        .stderr_eq(snapbox::str![""]);
}

#[test]
fn test_acton_help_without_flag() {
    snapbox::cmd::Command::acton_ui()
        .assert()
        .failure()
        .stdout_eq(snapbox::str![""])
        .stderr_eq(snapbox::file!["snapshots/acton/stderr_no_flag.txt"]);
}

#[test]
fn test_acton_lint_shows_check_replacement() {
    snapbox::cmd::Command::acton_ui()
        .arg("lint")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![""])
        .stderr_eq(snapbox::file!["snapshots/lint/stderr.txt"]);
}

#[test]
fn test_acton_lint_with_args_shows_check_replacement() {
    snapbox::cmd::Command::acton_ui()
        .args(["lint", "counter", "--fix"])
        .assert()
        .failure()
        .stdout_eq(snapbox::str![""])
        .stderr_eq(snapbox::file!["snapshots/lint/stderr_with_args.txt"]);
}

#[test]
fn test_acton_build_help() {
    snapbox::cmd::Command::acton_ui()
        .arg("build")
        .arg("--help")
        .assert()
        .success()
        .stdout_eq(snapbox::file!["snapshots/build/stdout.txt"])
        .stderr_eq(snapbox::str![""]);
}

#[test]
fn test_acton_help_build() {
    snapbox::cmd::Command::acton_ui()
        .arg("help")
        .arg("build")
        .assert()
        .success()
        .stdout_eq(snapbox::file!["snapshots/help_build/stdout.txt"])
        .stderr_eq(snapbox::str![""]);
}

#[test]
fn test_acton_new_help() {
    snapbox::cmd::Command::acton_ui()
        .arg("new")
        .arg("--help")
        .assert()
        .success()
        .stdout_eq(snapbox::file!["snapshots/new/stdout.txt"])
        .stderr_eq(snapbox::str![""]);
}

#[test]
fn test_acton_help_new() {
    snapbox::cmd::Command::acton_ui()
        .arg("help")
        .arg("new")
        .assert()
        .success()
        .stdout_eq(snapbox::file!["snapshots/help_new/stdout.txt"])
        .stderr_eq(snapbox::str![""]);
}

#[test]
fn test_acton_rpc_help() {
    snapbox::cmd::Command::acton_ui()
        .arg("rpc")
        .arg("--help")
        .assert()
        .success()
        .stdout_eq(snapbox::file!["snapshots/rpc/stdout.txt"])
        .stderr_eq(snapbox::str![""]);
}

#[test]
fn test_acton_rpc_info_help() {
    snapbox::cmd::Command::acton_ui()
        .args(["rpc", "info", "--help"])
        .assert()
        .success()
        .stdout_eq(snapbox::file!["snapshots/rpc_info/stdout.txt"])
        .stderr_eq(snapbox::str![""]);
}

#[test]
fn test_acton_retrace_help() {
    snapbox::cmd::Command::acton_ui()
        .args(["retrace", "--help"])
        .assert()
        .success()
        .stdout_eq(snapbox::file!["snapshots/retrace/stdout.txt"])
        .stderr_eq(snapbox::str![""]);
}

#[test]
fn test_acton_help_retrace() {
    snapbox::cmd::Command::acton_ui()
        .arg("help")
        .arg("retrace")
        .assert()
        .success()
        .stdout_eq(snapbox::file!["snapshots/help_retrace/stdout.txt"])
        .stderr_eq(snapbox::str![""]);
}

#[test]
fn test_acton_help_verify() {
    snapbox::cmd::Command::acton_ui()
        .arg("help")
        .arg("verify")
        .assert()
        .success()
        .stdout_eq(snapbox::file!["snapshots/help_verify/stdout.txt"])
        .stderr_eq(snapbox::str![""]);
}

#[test]
fn test_manual_commands_short_help_points_to_detailed_help() {
    for command in MANUAL_COMMANDS {
        let output = Command::new(common::acton_exe())
            .args(["--color", "never", command, "--help"])
            .output()
            .unwrap_or_else(|err| panic!("failed to run acton {command} --help: {err}"));

        assert!(
            output.status.success(),
            "acton {command} --help failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );

        let stdout = common::strip_ansi(&String::from_utf8_lossy(&output.stdout));
        let expected = format!("Run 'acton help {command}' for more detailed information.");
        assert!(
            stdout.contains(&expected),
            "acton {command} --help did not contain detailed help pointer.\nExpected: {expected}\nActual stdout:\n{stdout}",
        );
    }
}

#[test]
fn test_manual_commands_detailed_help_is_available() {
    for command in MANUAL_COMMANDS {
        let output = Command::new(common::acton_exe())
            .args(["--color", "never", "help", command])
            .output()
            .unwrap_or_else(|err| panic!("failed to run acton help {command}: {err}"));

        assert!(
            output.status.success(),
            "acton help {command} failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );

        let stdout = common::strip_ansi(&String::from_utf8_lossy(&output.stdout));
        let expected = format!("ACTON-{}(1)", command.to_ascii_uppercase());
        assert!(
            stdout.contains(&expected),
            "acton help {command} did not render the generated manual.\nExpected to find: {expected}\nActual stdout:\n{stdout}",
        );
    }
}

#[test]
fn test_commands_index_links_all_documented_command_pages() {
    #[derive(serde::Deserialize)]
    struct CommandsMeta {
        pages: Vec<String>,
    }

    let meta = fs::read_to_string("docs/content/docs/commands/meta.json")
        .expect("failed to read commands meta.json");
    let meta: CommandsMeta =
        serde_json::from_str(&meta).expect("failed to parse commands meta.json");
    let index = fs::read_to_string("docs/content/docs/commands/overview.mdx")
        .expect("failed to read commands overview.mdx");

    for page in meta.pages {
        let href = format!("href=\"/docs/commands/{page}\"");
        assert!(
            index.contains(&href),
            "commands index is missing a card for {page} ({href})"
        );
    }
}

#[test]
fn test_acton_meta_get_schema_prints_embedded_schema() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let output = Command::new(common::acton_exe())
        .args(["meta", "get-schema"])
        .current_dir(temp_dir.path())
        .output()
        .unwrap_or_else(|err| panic!("failed to run acton meta get-schema: {err}"));

    assert!(
        output.status.success(),
        "acton meta get-schema failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    assert_eq!(String::from_utf8_lossy(&output.stdout), ACTON_SCHEMA_JSON);
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}
