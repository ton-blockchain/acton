#[cfg(test)]
mod common;
#[cfg(test)]
mod integration;
#[cfg(test)]
mod support;

use common::ActonCommandExt;

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
fn test_acton_build_help() {
    snapbox::cmd::Command::acton_ui()
        .arg("build")
        .arg("--help")
        .assert()
        .success()
        .stdout_eq(snapbox::file!["snapshots/build/stdout.txt"])
        .stderr_eq(snapbox::str![""]);
}
