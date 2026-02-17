//! Reserved integration test module for subagent BA.
//!
//! Ownership boundary for agent BA:
//! - tests/integration/test_std_agent_ba_tests.rs
//! - tests/integration/snapshots/test_std_agent_ba/**
//! - tests/integration/testdata/test_std_agent_ba/**
//! - tests/support/test_std_agent_ba/** (optional)
//!
//! Required test name prefix:
//! - ba_stdlib_

use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const CRYPTO_IMPORTS: &str = r#"
import "../../lib/crypto/crypto"
import "../../lib/testing/expect"
"#;

fn run_crypto_failure_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{CRYPTO_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("crypto_signing_edges", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn ba_stdlib_crypto_sign_is_hash_sensitive_for_different_cells_in_fixture_project() {
    let fixture = FixtureProject::load("basic");
    let source = format!(
        r#"
{CRYPTO_IMPORTS}
get fun `test-ba-stdlib-sign-hash-sensitive-for-different-cells`() {{
    val words = crypto.createMnemonic();
    val kp = words.toKeyPair();

    val cellA = beginCell().storeUint(0xAB, 8).storeUint(1, 8).endCell();
    val cellB = beginCell().storeUint(0xAB, 8).storeUint(2, 8).endCell();

    val sigA = crypto.sign(kp.privateKey, cellA);
    val sigB = crypto.sign(kp.privateKey, cellB);

    expect(sigA).toNotEqual(sigB);
    expect(isSignatureValid(cellA.hash(), sigA, kp.publicKey)).toBeTrue();
    expect(isSignatureValid(cellB.hash(), sigA, kp.publicKey)).toBeFalse();
}}
"#
    );

    fs::write(
        fixture
            .path()
            .join("tests/crypto_sign_hash_sensitivity_edge.test.tolk"),
        source,
    )
    .expect("failed to write sign hash-sensitivity fixture test");

    fixture
        .acton()
        .test()
        .path("tests/crypto_sign_hash_sensitivity_edge.test.tolk")
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test_std_agent_ba/ba_stdlib_crypto_sign_is_hash_sensitive_for_different_cells_in_fixture_project.stdout.txt",
        );
}

#[test]
fn ba_stdlib_crypto_raw_sign_positive_and_negative_hash_values_do_not_collapse() {
    run_crypto_failure_case(
        "ba-stdlib-raw-sign-positive-negative-hash-values",
        r#"
get fun `test-ba-stdlib-raw-sign-positive-negative-hash-values`() {
    val words = crypto.createMnemonic();
    val kp = words.toKeyPair();

    val hashA = 1;
    val hashB = -1;

    val sigA = crypto.rawSign(kp.privateKey, hashA);
    val sigB = crypto.rawSign(kp.privateKey, hashB);

    // BUG: rawSign should be hash-sensitive for distinct hash values; expected different signatures for 1 and -1, got equal signatures.
    expect(sigA).toNotEqual(sigB);
}
"#,
        "integration/snapshots/test_std_agent_ba/ba_stdlib_crypto_raw_sign_positive_and_negative_hash_values_do_not_collapse.stdout.txt",
    );
}

#[test]
fn ba_stdlib_crypto_raw_sign_positive_and_negative_private_keys_do_not_collapse() {
    run_crypto_failure_case(
        "ba-stdlib-raw-sign-positive-negative-private-keys",
        r#"
get fun `test-ba-stdlib-raw-sign-positive-negative-private-keys`() {
    val hash = beginCell().storeUint(0xCAFE, 16).endCell().hash();
    val keyA = 1;
    val keyB = -1;

    val sigA = crypto.rawSign(keyA, hash);
    val sigB = crypto.rawSign(keyB, hash);

    // BUG: rawSign should remain key-sensitive for distinct private keys; expected different signatures for 1 and -1, got equal signatures.
    expect(sigA).toNotEqual(sigB);
}
"#,
        "integration/snapshots/test_std_agent_ba/ba_stdlib_crypto_raw_sign_positive_and_negative_private_keys_do_not_collapse.stdout.txt",
    );
}
