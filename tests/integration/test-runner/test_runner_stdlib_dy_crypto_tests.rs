//! Reserved integration test module for subagent DY.
//!
//! Ownership boundary for agent DY:
//! - tests/integration/test-runner/test_runner_stdlib_dy_crypto_tests.rs
//! - tests/integration/snapshots/test-runner/test_runner_stdlib_dy_crypto_tests/**
//! - tests/integration/testdata/test_std_agent_dy/**
//! - tests/support/test_std_agent_dy/** (optional)
//!
//! Required test name prefix:
//! - dy_stdlib_

use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const SIMPLE_CONTRACT: &str = r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#;

const CRYPTO_IMPORTS: &str = r#"
import "../../lib/crypto/crypto"
import "../../lib/testing/expect"
"#;

fn run_dy_secure_random_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{CRYPTO_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("dy_secure_random_boundaries", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn crypto_secure_random_bytes_boundaries_1_and_2_have_expected_lengths() {
    run_dy_secure_random_case(
        "dy-stdlib-secure-random-boundaries-1-and-2-lengths",
        r#"
get fun `test-dy-stdlib-secure-random-boundaries-1-and-2-lengths`() {
    val bytes1 = crypto.getSecureRandomBytes(1);
    val bytes2 = crypto.getSecureRandomBytes(2);

    expect(bytes1.remainingBitsCount()).toEqual(8);
    expect(bytes1.remainingRefsCount()).toEqual(0);
    expect(bytes2.remainingBitsCount()).toEqual(16);
    expect(bytes2.remainingRefsCount()).toEqual(0);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_dy_crypto_tests/dy_stdlib_crypto_secure_random_bytes_boundaries_1_and_2_have_expected_lengths.stdout.txt",
    );
}

#[test]
fn crypto_secure_random_bytes_size_2_keeps_length_across_calls_in_fixture_project() {
    let fixture = FixtureProject::load("basic");
    let source = format!(
        r#"
{CRYPTO_IMPORTS}
get fun `test-dy-stdlib-secure-random-size-2-keeps-length-across-calls`() {{
    val bytesA = crypto.getSecureRandomBytes(2);
    val bytesB = crypto.getSecureRandomBytes(2);

    expect(bytesA.remainingBitsCount()).toEqual(16);
    expect(bytesA.remainingRefsCount()).toEqual(0);
    expect(bytesB.remainingBitsCount()).toEqual(16);
    expect(bytesB.remainingRefsCount()).toEqual(0);
}}
"#
    );

    fs::write(
        fixture
            .path()
            .join("tests/dy_secure_random_size_2_across_calls.test.tolk"),
        source,
    )
    .expect("failed to write dy secure random size-2 fixture test");

    fixture
        .acton()
        .test()
        .path("tests/dy_secure_random_size_2_across_calls.test.tolk")
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_stdlib_dy_crypto_tests/dy_stdlib_crypto_secure_random_bytes_size_2_keeps_length_across_calls_in_fixture_project.stdout.txt",
        );
}
