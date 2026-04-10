use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const CRYPTO_IMPORTS: &str = r#"
import "../../lib/crypto/crypto"
import "../../lib/testing/expect"
"#;

fn run_crypto_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{CRYPTO_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("dw_crypto_mnemonic", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn crypto_create_mnemonic_returns_24_non_empty_words() {
    run_crypto_case(
        "dw-stdlib-create-mnemonic-returns-24-non-empty-words",
        r#"
get fun `test dw stdlib create mnemonic returns 24 non empty words`() {
    val words = crypto.createMnemonic();
    expect(words.size()).toEqual(24);

    var i = 0;
    while (i < words.size()) {
        val word = words.get(i) as string;
        expect(word).toNotEqual("");
        i = i + 1;
    }
}
"#,
        "integration/snapshots/test-runner/crypto_create_mnemonic_returns_24_non_empty_words/crypto_create_mnemonic_returns_24_non_empty_words.stdout.txt",
    );
}

#[test]
fn crypto_create_mnemonic_outputs_are_valid_for_keypair_derivation_in_fixture_project() {
    let fixture = FixtureProject::load("basic");
    let test_path = "tests/dw_crypto_create_mnemonic_keypair_shape.test.tolk";
    let source = format!(
        r#"
{CRYPTO_IMPORTS}
get fun `test dw stdlib create mnemonic keypair shape`() {{
    val words = crypto.createMnemonic();
    expect(words.size()).toEqual(24);

    var i = 0;
    while (i < words.size()) {{
        val word = words.get(i) as string;
        expect(word).toNotEqual("");
        i = i + 1;
    }}

    val kp = words.toKeyPair();
    expect(kp.privateKey).toNotEqual(0);
    expect(kp.publicKey).toNotEqual(0);
}}
"#
    );

    fs::write(fixture.path().join(test_path), source)
        .expect("failed to write dw fixture mnemonic shape test");

    fixture
        .acton()
        .test()
        .path(test_path)
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/crypto_create_mnemonic_returns_24_non_empty_words/crypto_create_mnemonic_outputs_are_valid_for_keypair_derivation_in_fixture_project.stdout.txt",
        );
}
