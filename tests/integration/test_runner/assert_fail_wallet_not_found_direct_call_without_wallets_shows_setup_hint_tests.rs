use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use tempfile::TempDir;

const ASSERT_IMPORTS: &str = r#"
import "../../lib/testing/assert"
"#;

const TEST_MNEMONIC: &str = "cupboard match uphold miracle fog balance unknown region share hand trophy million toy narrow ability exchange first toast fresh maid report cram strong later";

const SINGLE_WALLET_CONFIG: &str = r#"
[wallets.deployer]
kind = "v5r1"
workchain = 0
keys = { mnemonic-file = "mnemonic.txt" }
"#;

fn wallet_not_found_script_source(location: &str) -> String {
    format!(
        r#"
{ASSERT_IMPORTS}

fun main() {{
    Assert.failWalletNotFound("el_missing_wallet", "{location}");
}}
"#,
    )
}

#[test]
fn assert_fail_wallet_not_found_direct_call_without_wallets_shows_setup_hint() {
    let location = "scripts/el_assert_fail_wallet_not_found.tolk:5:5";
    let home_temp = TempDir::new().expect("failed to create HOME tempdir");
    let home = home_temp
        .path()
        .to_str()
        .expect("temp HOME path must be valid UTF-8")
        .to_string();

    let output = ProjectBuilder::new("el-stdlib-assert-fail-wallet-not-found-no-wallets")
        .script_file(
            "el_assert_fail_wallet_not_found",
            &wallet_not_found_script_source(location),
        )
        .build()
        .acton()
        .env("HOME", &home)
        .script("scripts/el_assert_fail_wallet_not_found.tolk")
        .run()
        .failure();

    output
        .assert_stderr_contains(
            "Wallet el_missing_wallet not found in wallets.toml or global.wallets.toml. Wallets are not configured yet.",
        )
        .assert_stderr_contains(
            "See https://i582.github.io/acton/docs/setup-wallets/ for more information",
        )
        .assert_contains("at scripts/el_assert_fail_wallet_not_found.tolk:5:5");
}

#[test]
fn assert_fail_wallet_not_found_direct_call_with_loaded_wallets_lists_available_wallets() {
    let location = "scripts/el_assert_fail_wallet_not_found.tolk:5:5";
    let home_temp = TempDir::new().expect("failed to create HOME tempdir");
    let home = home_temp
        .path()
        .to_str()
        .expect("temp HOME path must be valid UTF-8")
        .to_string();

    let output = ProjectBuilder::new("el-stdlib-assert-fail-wallet-not-found-with-wallets")
        .script_file(
            "el_assert_fail_wallet_not_found",
            &wallet_not_found_script_source(location),
        )
        .raw_file("wallets.toml", SINGLE_WALLET_CONFIG)
        .raw_file("mnemonic.txt", TEST_MNEMONIC)
        .build()
        .acton()
        .env("HOME", &home)
        .script("scripts/el_assert_fail_wallet_not_found.tolk")
        .broadcast()
        .run()
        .failure();

    output
        .assert_stderr_contains("Wallet el_missing_wallet not found in Acton.toml")
        .assert_stderr_contains("Available wallets:")
        .assert_stderr_contains("deployer")
        .assert_contains("at scripts/el_assert_fail_wallet_not_found.tolk:5:5");

    assert!(
        !output
            .get_normalized_stderr()
            .contains("Wallets are not configured yet."),
        "wallet-list branch should not show setup-hint message"
    );
}
