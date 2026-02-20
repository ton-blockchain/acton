use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const CRYPTO_IMPORTS: &str = r#"
import "../../lib/crypto/crypto"
import "../../lib/testing/expect"
"#;

fn run_crypto_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{CRYPTO_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("dx_crypto_to_keypair", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn mnemonic_to_keypair_is_deterministic_for_repeated_calls() {
    run_crypto_case(
        "dx-stdlib-mnemonic-to-keypair-deterministic-repeated-calls",
        r#"
get fun `test-dx-stdlib-mnemonic-to-keypair-deterministic-repeated-calls`() {
    val words = crypto.createMnemonic();
    val kpFirst = words.toKeyPair();
    val kpSecond = words.toKeyPair();

    expect(kpFirst.privateKey).toEqual(kpSecond.privateKey);
    expect(kpFirst.publicKey).toEqual(kpSecond.publicKey);
    expect(kpFirst.privateKey).toNotEqual(0);
    expect(kpFirst.publicKey).toNotEqual(0);
}
"#,
        "integration/snapshots/test-runner/mnemonic_to_keypair_is_deterministic_for_repeated_calls/mnemonic_to_keypair_is_deterministic_for_repeated_calls.stdout.txt",
    );
}
