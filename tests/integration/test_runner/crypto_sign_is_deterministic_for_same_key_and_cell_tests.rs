use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const CRYPTO_IMPORTS: &str = r#"
import "../../lib/crypto/crypto"
import "../../lib/testing/expect"
"#;

fn run_crypto_success_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{CRYPTO_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("crypto_sign_behavior", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn crypto_sign_is_deterministic_for_same_key_and_cell() {
    run_crypto_success_case(
        "dz-stdlib-sign-deterministic-same-key-cell",
        r"
get fun `test-dz-stdlib-sign-deterministic-same-key-cell`() {
    val words = crypto.createMnemonic();
    val kp = words.toKeyPair();
    val data = beginCell().storeUint(0xD3, 8).storeUint(0x7A, 8).storeUint(2026, 16).endCell();

    val sigA = crypto.sign(kp.privateKey, data);
    val sigB = crypto.sign(kp.privateKey, data);

    expect(sigA).toEqual(sigB);
    expect(isSignatureValid(data.hash(), sigA, kp.publicKey)).toBeTrue();
}
",
        "integration/snapshots/test-runner/crypto_sign_is_deterministic_for_same_key_and_cell/crypto_sign_is_deterministic_for_same_key_and_cell.stdout.txt",
    );
}

#[test]
fn crypto_sign_has_512bit_slice_shape_and_matches_raw_sign_in_fixture_project() {
    let fixture = FixtureProject::load("basic");
    let test_path = "tests/dz_crypto_sign_shape.test.tolk";
    let source = format!(
        r"
{CRYPTO_IMPORTS}
get fun `test-dz-stdlib-sign-shape-and-raw-sign-parity`() {{
    val words = crypto.createMnemonic();
    val kp = words.toKeyPair();
    val payload = beginCell()
        .storeUint(0xBADA55, 24)
        .storeUint(0x77, 8)
        .storeUint(42, 8)
        .endCell();

    val signSig = crypto.sign(kp.privateKey, payload);
    val rawSig = crypto.rawSign(kp.privateKey, payload.hash());

    expect(signSig.remainingBitsCount()).toEqual(512);
    expect(signSig.remainingRefsCount()).toEqual(0);
    expect(rawSig.remainingBitsCount()).toEqual(512);
    expect(rawSig.remainingRefsCount()).toEqual(0);
    expect(signSig).toEqual(rawSig);
    expect(isSignatureValid(payload.hash(), signSig, kp.publicKey)).toBeTrue();
}}
"
    );

    fs::write(fixture.path().join(test_path), source)
        .expect("failed to write dz crypto.sign shape fixture test");

    fixture
        .acton()
        .test()
        .path(test_path)
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/crypto_sign_is_deterministic_for_same_key_and_cell/crypto_sign_has_512bit_slice_shape_and_matches_raw_sign_in_fixture_project.stdout.txt",
        );
}
