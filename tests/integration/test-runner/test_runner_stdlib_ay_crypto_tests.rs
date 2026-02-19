//! Reserved integration test module for subagent AY.
//!
//! Ownership boundary for agent AY:
//! - tests/integration/test-runner/test_runner_stdlib_ay_crypto_tests.rs
//! - tests/integration/snapshots/test-runner/test_runner_stdlib_ay_crypto_tests/**
//! - tests/integration/testdata/test_std_agent_ay/**
//! - tests/support/test_std_agent_ay/** (optional)
//!
//! Required test name prefix:
//! - ay_stdlib_

use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const CRYPTO_IMPORTS: &str = r#"
import "../../lib/crypto/crypto"
import "../../lib/testing/expect"
"#;

fn run_secure_random_success_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{CRYPTO_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("secure_random_boundary", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

fn run_secure_random_failure_case(
    project_name: &str,
    test_body: &str,
    expected_error: &str,
    snapshot_path: &str,
) {
    let source = format!("{CRYPTO_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("secure_random_boundary", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains(expected_error)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn crypto_secure_random_bytes_accepts_127_bytes_in_fixture_project() {
    let fixture = FixtureProject::load("basic");
    let source = format!(
        r#"
{CRYPTO_IMPORTS}
get fun `test-ay-stdlib-secure-random-accepts-127-bytes`() {{
    val bytes = crypto.getSecureRandomBytes(127);
    expect(bytes.remainingBitsCount()).toEqual(127 * 8);
    expect(bytes.remainingRefsCount()).toEqual(0);
}}
"#
    );

    fs::write(
        fixture
            .path()
            .join("tests/secure_random_127_boundary.test.tolk"),
        source,
    )
    .expect("failed to write secure random 127 boundary test");

    fixture
        .acton()
        .test()
        .path("tests/secure_random_127_boundary.test.tolk")
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_stdlib_ay_crypto_tests/ay_stdlib_crypto_secure_random_bytes_accepts_127_bytes_in_fixture_project.stdout.txt",
        );
}

#[test]
fn crypto_secure_random_bytes_rejects_zero_bytes() {
    run_secure_random_failure_case(
        "ay-stdlib-secure-random-rejects-zero-bytes",
        r#"
get fun `test-ay-stdlib-secure-random-rejects-zero-bytes`() {
    crypto.getSecureRandomBytes(0);
}
"#,
        "bytesNum must be between 1 and 128",
        "integration/snapshots/test-runner/test_runner_stdlib_ay_crypto_tests/ay_stdlib_crypto_secure_random_bytes_rejects_zero_bytes.stdout.txt",
    );
}

#[test]
fn crypto_secure_random_bytes_rejects_128_bytes_with_exit_567() {
    run_secure_random_success_case(
        "ay-stdlib-secure-random-rejects-128-bytes",
        r#"
get fun `test-ay-stdlib-secure-random-rejects-128-bytes`() {
    expectToEndWithExitCode(567);
    crypto.getSecureRandomBytes(128);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_ay_crypto_tests/ay_stdlib_crypto_secure_random_bytes_rejects_128_bytes_with_exit_567.stdout.txt",
    );
}
