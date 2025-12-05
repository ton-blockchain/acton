use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const SIMPLE_CONTRACT: &str = r#"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#;

#[test]
fn test_verify_contract_not_found() {
    let project = ProjectBuilder::new("verify-contract-not-found")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .verify()
        .verify_contract("nonexistent")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_verify_contract_not_found.stderr.txt",
        );
}

#[test]
fn test_verify_boc_file() {
    let project = ProjectBuilder::new("verify-boc-file")
        .raw_file("contracts/contract.boc", "some boc content")
        .build();

    let toml_content = r#"[package]
name = "verify-boc-file"
description = ""
version = "0.1.0"

[contracts.contract]
name = "contract"
src = "contracts/contract.boc"
depends = []
"#;
    std::fs::write(project.path().join("Acton.toml"), toml_content).expect("Write Acton.toml");

    project
        .acton()
        .verify()
        .verify_contract("contract")
        .run()
        .failure()
        .assert_stderr_snapshot_matches("integration/snapshots/test_verify_boc_file.stderr.txt");
}

#[test]
fn test_verify_invalid_network() {
    let project = ProjectBuilder::new("verify-invalid-net")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .verify()
        .verify_contract("simple")
        .network("invalid-network")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_verify_invalid_network.stderr.txt",
        );
}

#[test]
fn test_verify_invalid_address() {
    let project = ProjectBuilder::new("verify-invalid-addr")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .verify()
        .verify_contract("simple")
        .verify_address("invalid-address-format")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_verify_invalid_address.stderr.txt",
        );
}

#[test]
fn test_verify_wallet_not_found_without_wallets() {
    let project = ProjectBuilder::new("verify-wallet-not-found")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .verify()
        .verify_contract("simple")
        .verify_address("EQC2jeGorIAFh2LXwsDjHfRK-GSo9UzchdIEMh24A7T7AHot")
        .wallet("nonexistent-wallet")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_verify_wallet_not_found_without_wallets.stderr.txt",
        );
}

#[test]
fn test_verify_wallet_not_found() {
    let project = ProjectBuilder::new("verify-wallet-not-found")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    let toml_content = r#"[package]
name = "verify-contracts"
description = ""
version = "0.1.0"

[contracts.simple]
name = "Simple"
src = "contracts/simple.tolk"

[wallets.deployer]
kind = "v5r1"
workchain = 0
keys = { mnemonic-file = "Acton.toml" }
"#;
    std::fs::write(project.path().join("Acton.toml"), toml_content).expect("Write Acton.toml");

    project
        .acton()
        .verify()
        .verify_contract("simple")
        .verify_address("EQC2jeGorIAFh2LXwsDjHfRK-GSo9UzchdIEMh24A7T7AHot")
        .wallet("nonexistent-wallet")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_verify_wallet_not_found.stderr.txt",
        );
}

#[test]
fn test_verify_compilation_error() {
    let project = ProjectBuilder::new("verify-compilation-error")
        .contract(
            "broken",
            r#"
            fun onInternalMessage(in: InMessage) {
                val x = nonexistent_symbol();
            }
            fun onBouncedMessage(_: InMessageBounced) {}
        "#,
        )
        .build();

    project
        .acton()
        .verify()
        .verify_contract("broken")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_verify_compilation_error.stderr.txt",
        );
}

#[test]
fn test_verify_no_contracts_configured() {
    let project = ProjectBuilder::new("verify-no-contracts").build();

    let toml_content = r#"[package]
name = "verify-no-contracts"
description = ""
version = "0.1.0"
"#;
    std::fs::write(project.path().join("Acton.toml"), toml_content).expect("Write Acton.toml");

    project
        .acton()
        .verify()
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_verify_no_contracts_configured.stderr.txt",
        );
}

#[test]
fn test_verify_empty_contracts_section() {
    let project = ProjectBuilder::new("verify-empty-contracts").build();

    let toml_content = r#"[package]
name = "verify-empty-contracts"
description = ""
version = "0.1.0"

[contracts]
"#;
    std::fs::write(project.path().join("Acton.toml"), toml_content).expect("Write Acton.toml");

    project
        .acton()
        .verify()
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_verify_empty_contracts_section.stderr.txt",
        );
}
