use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use tempfile::TempDir;

const NETWORK_IMPORTS: &str = r#"
import "../../lib/emulation/network"
"#;

const TEST_MNEMONIC: &str = "cupboard match uphold miracle fog balance unknown region share hand trophy million toy narrow ability exchange first toast fresh maid report cram strong later";

const SINGLE_WALLET_CONFIG: &str = r#"
[wallets.deployer]
kind = "v5r1"
workchain = 0
keys = { mnemonic-file = "mnemonic.txt" }
"#;

fn run_wallet_missing_failure(
    project_name: &str,
    test_body: &str,
    snapshot_path: &str,
    wallets_toml: Option<&str>,
) {
    let source = format!("{NETWORK_IMPORTS}\n{test_body}\n");
    let mut builder = ProjectBuilder::new(project_name).test_file("wallet_missing", &source);
    if let Some(wallets_toml) = wallets_toml {
        builder = builder
            .raw_file("wallets.toml", wallets_toml)
            .raw_file("mnemonic.txt", TEST_MNEMONIC);
    }

    let home_temp = TempDir::new().expect("failed to create HOME tempdir");
    let home = home_temp
        .path()
        .to_str()
        .expect("temp HOME path must be valid UTF-8")
        .to_string();

    builder
        .build()
        .acton()
        .env("HOME", &home)
        .test()
        .run()
        .code(1)
        .assert_failed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn wallet_missing_in_broadcast_without_wallets_config_reports_setup_instructions() {
    run_wallet_missing_failure(
        "bw-stdlib-wallet-missing-no-wallets-config",
        r#"
get fun `test-bw-wallet-missing-no-wallets-config`() {
    net.enableBroadcast();
    net.wallet("bw_missing_wallet");
}
"#,
        "integration/snapshots/test-runner/wallet_missing_in_broadcast_without_wallets_config_reports_setup_instructions/wallet_missing_in_broadcast_without_wallets_config_reports_setup_instructions.stdout.txt",
        None,
    );
}

#[test]
fn wallet_missing_in_broadcast_with_wallets_config_lists_available_wallets() {
    run_wallet_missing_failure(
        "bw-stdlib-wallet-missing-with-wallets-config",
        r#"
get fun `test-bw-wallet-missing-with-wallets-config`() {
    net.enableBroadcast();
    net.wallet("bw_missing_wallet");
}
"#,
        "integration/snapshots/test-runner/wallet_missing_in_broadcast_without_wallets_config_reports_setup_instructions/wallet_missing_in_broadcast_with_wallets_config_lists_available_wallets.stdout.txt",
        Some(SINGLE_WALLET_CONFIG),
    );
}
