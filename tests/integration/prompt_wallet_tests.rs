use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use std::fs;

const DEPLOYER_MNEMONIC: &str = "cupboard match uphold miracle fog balance unknown region share hand trophy million toy narrow ability exchange first toast fresh maid report cram strong later";

const SECOND_MNEMONIC: &str = "section garden tomato dinner season dice renew length useful spin trade intact use universe what post spike keen mandate behind concert egg doll rug";

const PROMPT_WALLET_SCRIPT: &str = r#"
import "../../lib/promts/prompts"
import "../../lib/io"

fun main() {
    val name = promptWallet("Select a wallet:");
    println("selected={}", name);
}
"#;

#[test]
fn prompt_wallet_without_wallets_fails_with_setup_hint() {
    ProjectBuilder::new("prompt-wallet-no-wallets")
        .script_file("use_prompt_wallet", PROMPT_WALLET_SCRIPT)
        .build()
        .acton()
        .script("scripts/use_prompt_wallet.tolk")
        .verify_network("testnet")
        .run()
        .failure()
        .assert_snapshot_matches(
            "integration/snapshots/prompt_wallet/without_wallets_fails_with_setup_hint.stdout.txt",
        );
}

#[test]
fn prompt_wallet_with_single_wallet_returns_its_name() {
    let project = ProjectBuilder::new("prompt-wallet-single")
        .script_file("use_prompt_wallet", PROMPT_WALLET_SCRIPT)
        .build();

    fs::write(project.path().join("mnemonic.txt"), DEPLOYER_MNEMONIC)
        .expect("failed to write mnemonic");
    fs::write(
        project.path().join("wallets.toml"),
        r#"[wallets.deployer]
kind = "v4r2"
workchain = 0
keys = { mnemonic-file = "mnemonic.txt" }
"#,
    )
    .expect("failed to write wallets.toml");

    project
        .acton()
        .script("scripts/use_prompt_wallet.tolk")
        .verify_network("testnet")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/prompt_wallet/with_single_wallet_returns_its_name.stdout.txt",
        );
}

#[test]
fn prompt_wallet_with_multiple_wallets_fails_in_non_interactive_mode() {
    let project = ProjectBuilder::new("prompt-wallet-multiple-non-interactive")
        .script_file("use_prompt_wallet", PROMPT_WALLET_SCRIPT)
        .build();

    fs::write(project.path().join("deployer.txt"), DEPLOYER_MNEMONIC)
        .expect("failed to write deployer mnemonic");
    fs::write(project.path().join("other.txt"), SECOND_MNEMONIC)
        .expect("failed to write other mnemonic");
    fs::write(
        project.path().join("wallets.toml"),
        r#"[wallets.deployer]
kind = "v4r2"
workchain = 0
keys = { mnemonic-file = "deployer.txt" }

[wallets.other]
kind = "v4r2"
workchain = 0
keys = { mnemonic-file = "other.txt" }
"#,
    )
    .expect("failed to write wallets.toml");

    project
        .acton()
        .script("scripts/use_prompt_wallet.tolk")
        .verify_network("testnet")
        .run()
        .failure()
        .assert_snapshot_matches(
            "integration/snapshots/prompt_wallet/with_multiple_wallets_fails_in_non_interactive_mode.stdout.txt",
        );
}

#[test]
fn prompt_wallet_in_emulate_mode_returns_placeholder() {
    // Without `verify_network` (i.e. plain `acton script` / emulate mode) `net.wallet(name)`
    // accepts any name, so `promptWallet` should return a stable placeholder instead of
    // failing — even when no wallets.toml is present.
    ProjectBuilder::new("prompt-wallet-emulate")
        .script_file("use_prompt_wallet", PROMPT_WALLET_SCRIPT)
        .build()
        .acton()
        .script("scripts/use_prompt_wallet.tolk")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/prompt_wallet/in_emulate_mode_returns_placeholder.stdout.txt",
        );
}
