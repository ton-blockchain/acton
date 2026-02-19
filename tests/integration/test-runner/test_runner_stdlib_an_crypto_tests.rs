//! Reserved integration test module for subagent AN.
//!
//! Ownership boundary for agent AN:
//! - tests/integration/test-runner/test_runner_stdlib_an_crypto_tests.rs
//! - tests/integration/snapshots/test-runner/test_runner_stdlib_an_crypto_tests/**
//! - tests/integration/testdata/test_std_agent_an/**
//! - tests/support/test_std_agent_an/** (optional)
//!
//! Required test name prefix:
//! - an_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const CRYPTO_IMPORTS: &str = r#"
import "../../lib/crypto/crypto"
import "../../lib/testing/expect"
"#;

fn run_crypto_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{CRYPTO_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("crypto_behavior", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn crypto_create_mnemonic_returns_24_words() {
    run_crypto_case(
        "an-stdlib-create-mnemonic-returns-24-words",
        r#"
get fun `test-an-stdlib-create-mnemonic-returns-24-words`() {
    val words = crypto.createMnemonic();
    expect(words.size()).toEqual(24);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_an_crypto_tests/an_stdlib_crypto_create_mnemonic_returns_24_words.stdout.txt",
    );
}

#[test]
fn crypto_to_keypair_is_deterministic_for_same_mnemonic() {
    run_crypto_case(
        "an-stdlib-to-keypair-deterministic",
        r#"
get fun `test-an-stdlib-to-keypair-deterministic`() {
    val words = crypto.createMnemonic();
    val kp1 = words.toKeyPair();
    val kp2 = words.toKeyPair();

    expect(kp1.privateKey).toEqual(kp2.privateKey);
    expect(kp1.publicKey).toEqual(kp2.publicKey);
    expect(kp1.privateKey).toNotEqual(0);
    expect(kp1.publicKey).toNotEqual(0);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_an_crypto_tests/an_stdlib_crypto_to_keypair_is_deterministic_for_same_mnemonic.stdout.txt",
    );
}

#[test]
fn crypto_sign_matches_raw_sign_and_verifies() {
    run_crypto_case(
        "an-stdlib-sign-matches-raw-sign",
        r#"
get fun `test-an-stdlib-sign-matches-raw-sign`() {
    val words = crypto.createMnemonic();
    val kp = words.toKeyPair();
    val data = beginCell().storeUint(0xA1B2C3D4, 32).storeUint(77, 8).endCell();

    val signSig = crypto.sign(kp.privateKey, data);
    val rawSig = crypto.rawSign(kp.privateKey, data.hash());

    expect(signSig).toEqual(rawSig);
    expect(signSig.remainingBitsCount()).toEqual(512);
    expect(signSig.remainingRefsCount()).toEqual(0);
    expect(isSignatureValid(data.hash(), signSig, kp.publicKey)).toBeTrue();
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_an_crypto_tests/an_stdlib_crypto_sign_matches_raw_sign_and_verifies.stdout.txt",
    );
}

#[test]
fn crypto_raw_sign_is_deterministic_and_hash_sensitive() {
    run_crypto_case(
        "an-stdlib-raw-sign-deterministic-hash-sensitive",
        r#"
get fun `test-an-stdlib-raw-sign-deterministic-hash-sensitive`() {
    val words = crypto.createMnemonic();
    val kp = words.toKeyPair();

    val hashA = beginCell().storeUint(111, 16).endCell().hash();
    val hashB = beginCell().storeUint(112, 16).endCell().hash();

    val sigA1 = crypto.rawSign(kp.privateKey, hashA);
    val sigA2 = crypto.rawSign(kp.privateKey, hashA);

    expect(sigA1).toEqual(sigA2);
    expect(isSignatureValid(hashA, sigA1, kp.publicKey)).toBeTrue();
    expect(isSignatureValid(hashB, sigA1, kp.publicKey)).toBeFalse();
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_an_crypto_tests/an_stdlib_crypto_raw_sign_is_deterministic_and_hash_sensitive.stdout.txt",
    );
}

#[test]
fn crypto_fast_random_bytes_seeded_are_deterministic() {
    run_crypto_case(
        "an-stdlib-fast-random-seeded-deterministic",
        r#"
get fun `test-an-stdlib-fast-random-seeded-deterministic`() {
    val seeded127a = crypto.getFastRandomBytes(127, 42);
    val seeded127b = crypto.getFastRandomBytes(127, 42);
    // BUG: getFastRandomBytes should be deterministic for the same seed; expected equal slices, got different values.
    expect(seeded127a).toEqual(seeded127b);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_an_crypto_tests/an_stdlib_crypto_fast_random_bytes_seeded_are_deterministic.stdout.txt",
    );
}

#[test]
fn crypto_fast_random_bytes_rejects_128_bytes() {
    run_crypto_case(
        "an-stdlib-fast-random-rejects-128-bytes",
        r#"
get fun `test-an-stdlib-fast-random-rejects-128-bytes`() {
    expectToEndWithExitCode(567);
    crypto.getFastRandomBytes(128, 1);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_an_crypto_tests/an_stdlib_crypto_fast_random_bytes_rejects_128_bytes.stdout.txt",
    );
}

#[test]
fn crypto_fast_random_bytes_supports_zero_bytes() {
    run_crypto_case(
        "an-stdlib-fast-random-supports-zero-bytes",
        r#"
get fun `test-an-stdlib-fast-random-supports-zero-bytes`() {
    val seeded = crypto.getFastRandomBytes(0, 42);
    val noSeed = crypto.getFastRandomBytes(0);

    expect(seeded.remainingBitsCount()).toEqual(0);
    expect(seeded.remainingRefsCount()).toEqual(0);
    expect(noSeed.remainingBitsCount()).toEqual(0);
    expect(noSeed.remainingRefsCount()).toEqual(0);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_an_crypto_tests/an_stdlib_crypto_fast_random_bytes_supports_zero_bytes.stdout.txt",
    );
}

#[test]
fn crypto_secure_random_bytes_supports_1_and_127() {
    run_crypto_case(
        "an-stdlib-secure-random-supports-1-and-127",
        r#"
get fun `test-an-stdlib-secure-random-supports-1-and-127`() {
    val b1 = crypto.getSecureRandomBytes(1);
    val b127 = crypto.getSecureRandomBytes(127);

    expect(b1.remainingBitsCount()).toEqual(8);
    expect(b1.remainingRefsCount()).toEqual(0);
    expect(b127.remainingBitsCount()).toEqual(127 * 8);
    expect(b127.remainingRefsCount()).toEqual(0);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_an_crypto_tests/an_stdlib_crypto_secure_random_bytes_supports_1_and_127.stdout.txt",
    );
}

#[test]
fn crypto_secure_random_bytes_rejects_128_bytes() {
    run_crypto_case(
        "an-stdlib-secure-random-rejects-128-bytes",
        r#"
get fun `test-an-stdlib-secure-random-rejects-128-bytes`() {
    expectToEndWithExitCode(567);
    crypto.getSecureRandomBytes(128);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_an_crypto_tests/an_stdlib_crypto_secure_random_bytes_rejects_128_bytes.stdout.txt",
    );
}
