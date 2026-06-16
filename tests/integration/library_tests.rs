#[cfg(feature = "only_ci")]
use crate::common::assertion;
use crate::support::TestOutputExt;
use crate::support::project::{Project, ProjectBuilder};
#[cfg(feature = "only_ci")]
use crate::support::snapshots::normalize_output;
use base64::Engine;
use serde_json::Value as JsonValue;
use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::time::Instant;
use std::{fs, thread};
use toml::Value as TomlValue;
use toncenter_keys::{TONCENTER_MAINNET_API_KEY_ENV, TONCENTER_TESTNET_API_KEY_ENV};

const LIB_HASH: &str = "b993c68c596425f05d1bc492d7c03e2979ab669901ed5a57e35e6dd4d6089d27";
const PUBLISH_TEST_CODE_ARG: &str = "te6cckEBAQEAAgAAAEysuc0=";
const PUBLISH_TEST_CODE_BOC64: &str = "te6ccgEBAQEAAgAAAA==";
const PUBLISH_TEST_CODE_HASH: &str =
    "96a296d224f285c67bee93c30f8a309157f0daa35dc5b87e410b78630a09cfc7";
const LOCALNET_LIBRARY_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

const LOCALNET_DEPLOYER_WALLET_CONFIG: &str = r#"[wallets.deployer]
kind = "v4r2"
workchain = 0
keys = { mnemonic = "cupboard match uphold miracle fog balance unknown region share hand trophy million toy narrow ability exchange first toast fresh maid report cram strong later" }
"#;
const TEST_LIBRARY_ACCOUNT: &str = "kQBBSo2ccLuHuGiTn1z9Lei17LfBVOPewQmFR8pA2dAv2ixT";
const TEST_TONCENTER_MAINNET_V2_URL_ENV: &str = "ACTON_TEST_TONCENTER_MAINNET_V2_URL";
const TEST_TONCENTER_TESTNET_V2_URL_ENV: &str = "ACTON_TEST_TONCENTER_TESTNET_V2_URL";

// We don't usually want to store keys this way, but without keys it's almost
// impossible to use API calls :(
fn toncenter_api_key() -> &'static str {
    option_env!("TONCENTER_TESTNET_API_KEY")
        .or(option_env!("TONCENTER_MAINNET_API_KEY"))
        .unwrap_or("49efa980ccdcd018fd09d387e63537afd9db4dbb8509d69e7bc2303ca2b2c860")
}

#[test]
#[cfg(feature = "only_ci")]
fn test_library_fetch_basic() {
    thread::sleep(Duration::from_secs(1));
    let project = ProjectBuilder::new("library-fetch-basic").build();

    project
        .acton()
        .library()
        .fetch(LIB_HASH)
        .with_net("testnet")
        .with_api_key(toncenter_api_key())
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/library/test_library_fetch_basic.stdio.txt",
        );
}

#[test]
#[cfg(feature = "only_ci")]
fn test_library_fetch_json() {
    thread::sleep(Duration::from_secs(1));
    let project = ProjectBuilder::new("library-fetch-json").build();

    project
        .acton()
        .library()
        .fetch(LIB_HASH)
        .with_net("testnet")
        .with_api_key(toncenter_api_key())
        .with_json()
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/library/test_library_fetch_json.stdout.json.txt",
        );
}

#[test]
#[cfg(feature = "only_ci")]
fn test_library_fetch_fail_json() {
    thread::sleep(Duration::from_secs(1));
    let project = ProjectBuilder::new("library-fetch-json").build();

    project
        .acton()
        .library()
        .fetch("b993c68c596425f05d1bc492d7c03e2979ab669901ed5a57e35e6dd4d6089d28")
        .with_net("testnet")
        .with_api_key(toncenter_api_key())
        .with_json()
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/library/test_library_fetch_fail_json.stdout.json.txt",
        );
}

#[test]
#[cfg(feature = "only_ci")]
fn test_library_fetch_unknown() {
    thread::sleep(Duration::from_secs(1));
    let project = ProjectBuilder::new("library-fetch-unknown").build();

    project
        .acton()
        .library()
        .fetch("b993c68c596425f05d1bc492d7c03e2979ab669901ed5a57e35e6dd4d6089d28")
        .with_net("testnet")
        .with_api_key(toncenter_api_key())
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/library/test_library_fetch_unknown.stderr.txt",
        );
}

#[test]
#[cfg(feature = "only_ci")]
fn test_library_fetch_unknown_json() {
    thread::sleep(Duration::from_secs(1));
    let project = ProjectBuilder::new("library-fetch-unknown-json").build();

    project
        .acton()
        .library()
        .fetch("b993c68c596425f05d1bc492d7c03e2979ab669901ed5a57e35e6dd4d6089d28")
        .with_net("testnet")
        .with_api_key(toncenter_api_key())
        .with_json()
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/library/test_library_fetch_unknown.stdout.json.txt",
        );
}

#[test]
#[cfg(feature = "only_ci")]
fn test_library_fetch_disasm() {
    thread::sleep(Duration::from_secs(1));
    let project = ProjectBuilder::new("library-fetch-disasm").build();

    project
        .acton()
        .library()
        .fetch(LIB_HASH)
        .with_net("testnet")
        .with_api_key(toncenter_api_key())
        .with_disasm_flag()
        .run()
        .success()
        .assert_contains("Fetched successfully")
        .assert_snapshot_matches(
            "integration/snapshots/library/test_library_fetch_basic_disasm.stdio.txt",
        );
}

#[test]
#[cfg(feature = "only_ci")]
fn test_library_fetch_output() {
    thread::sleep(Duration::from_secs(1));
    let project = ProjectBuilder::new("library-fetch-output").build();

    project
        .acton()
        .library()
        .fetch(LIB_HASH)
        .with_net("testnet")
        .with_output("lib.txt")
        .with_api_key(toncenter_api_key())
        .run()
        .success()
        .assert_contains("Fetched successfully")
        .assert_contains("Written to lib.txt");

    let lib_file = project.path().join("lib.txt");
    assert!(lib_file.exists());

    let content = fs::read_to_string(&lib_file).expect("Should read lib.txt file");

    assertion().eq(
        normalize_output(content.as_str(), project.path()),
        snapbox::file!("snapshots/library/test_library_fetch_basic.lib.txt"),
    );
}

#[test]
#[cfg(feature = "only_ci")]
fn test_library_fetch_boc() {
    thread::sleep(Duration::from_secs(1));
    let project = ProjectBuilder::new("library-fetch-boc").build();

    project
        .acton()
        .library()
        .fetch(LIB_HASH)
        .with_net("testnet")
        .with_output("lib.boc")
        .with_api_key(toncenter_api_key())
        .run()
        .success()
        .assert_contains("Fetched successfully")
        .assert_contains("Written to lib.boc");

    let boc_path = project.path().join("lib.boc");
    assert!(boc_path.exists());

    let content = fs::read(boc_path).unwrap();
    assert!(!content.is_empty());
}

#[test]
#[cfg(feature = "only_ci")]
fn test_library_fetch_disasm_output() {
    thread::sleep(Duration::from_secs(1));
    let project = ProjectBuilder::new("library-fetch-disasm-output").build();

    project
        .acton()
        .library()
        .fetch(LIB_HASH)
        .with_net("testnet")
        .with_disasm_flag()
        .with_output("lib.tasm")
        .with_api_key(toncenter_api_key())
        .run()
        .success()
        .assert_contains("Fetched successfully");

    let lib_file = project.path().join("lib.tasm");
    assert!(lib_file.exists());

    let content = fs::read_to_string(&lib_file).expect("Should read lib.txt file");

    assertion().eq(
        normalize_output(content.as_str(), project.path()),
        snapbox::file!("snapshots/library/test_library_fetch_basic.lib.tasm.txt"),
    );
}

#[test]
fn test_library_fetch_invalid_hash_format() {
    thread::sleep(Duration::from_secs(1));
    let project = ProjectBuilder::new("library-fetch-invalid-hash-format").build();

    project
        .acton()
        .library()
        .fetch("not-a-valid-hash")
        .with_net("testnet")
        .with_api_key(toncenter_api_key())
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/library/test_library_fetch_invalid_hash_format.stderr.txt",
        );
}

#[test]
fn test_library_fetch_invalid_network() {
    let project = ProjectBuilder::new("library-fetch-invalid-network").build();

    project
        .acton()
        .library()
        .fetch(LIB_HASH)
        .with_net("invalid")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/library/test_library_fetch_invalid_network.stderr.txt",
        );
}

#[test]
fn test_library_publish_invalid_network() {
    thread::sleep(Duration::from_secs(1));
    let project = ProjectBuilder::new("library-publish-invalid-net").build();

    project
        .acton()
        .library()
        .publish()
        .with_net("invalid")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/library/test_library_publish_invalid_network.stderr.txt",
        );
}

#[test]
fn test_library_publish_invalid_code() {
    thread::sleep(Duration::from_secs(1));
    let project = ProjectBuilder::new("library-publish-invalid-code").build();

    project
        .acton()
        .library()
        .publish()
        .with_code("not-hex-or-base64")
        .with_net("testnet")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/library/test_library_publish_invalid_code.stderr.txt",
        );
}

#[test]
fn test_library_publish_tonconnect_rejects_localnet() {
    let project = ProjectBuilder::new("library-publish-tonconnect-localnet").build();

    project
        .acton()
        .library()
        .publish()
        .arg("--tonconnect")
        .with_net("localnet")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/library/test_library_publish_tonconnect_rejects_localnet.stderr.txt",
        );
}

#[test]
fn test_library_publish_contract_not_found() {
    thread::sleep(Duration::from_secs(1));
    let project = ProjectBuilder::new("library-publish-contract-not-found").build();

    let toml_content = r#"[package]
name = "library-publish-contract-not-found"
description = ""
version = "0.1.0"
"#;
    fs::write(project.path().join("Acton.toml"), toml_content).expect("Write Acton.toml");

    project
        .acton()
        .library()
        .publish()
        .with_net("testnet")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/library/test_library_publish_contract_not_found.stderr.txt",
        );
}

#[test]
fn test_library_publish_compilation_error() {
    thread::sleep(Duration::from_secs(1));
    let project = ProjectBuilder::new("library-publish-compilation-error")
        .contract("broken", "fun main() { return 1 +; }")
        .build();

    project
        .acton()
        .library()
        .publish()
        .contract("broken")
        .with_net("testnet")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/library/test_library_publish_compilation_error.stderr.txt",
        );
}

#[test]
fn test_library_publish_invalid_duration() {
    thread::sleep(Duration::from_secs(1));
    let project = ProjectBuilder::new("library-publish-invalid-duration")
        .contract("simple", "fun main() {}")
        .build();

    project
        .acton()
        .library()
        .publish()
        .contract("simple")
        .with_duration("invalid-duration")
        .with_net("testnet")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/library/test_library_publish_invalid_duration.stderr.txt",
        );
}

#[test]
fn test_library_publish_wallet_not_found() {
    thread::sleep(Duration::from_secs(1));
    let project = ProjectBuilder::new("library-publish-wallet-not-found")
        .contract("simple", "fun main() {}")
        .build();

    let toml_content = r#"[package]
name = "library-publish-wallet-not-found"
description = ""
version = "0.1.0"

[contracts.simple]
display-name = "Simple"
src = "contracts/simple.tolk"
"#;
    fs::write(project.path().join("Acton.toml"), toml_content).expect("Write Acton.toml");

    let home_temp = tempfile::TempDir::new().unwrap();
    let home_path = home_temp.path();

    project
        .acton()
        .env("HOME", home_path.to_str().unwrap())
        .library()
        .publish()
        .contract("simple")
        .wallet("nonexistent")
        .with_net("testnet")
        .with_duration("100d") // Provide duration to bypass prompt
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/library/test_library_publish_wallet_not_found.stderr.txt",
        );
}

#[test]
fn test_library_publish_no_wallets() {
    let project = ProjectBuilder::new("library-publish-invalid-net")
        .contract("simple", "fun main() {}")
        .build();

    let home_temp = tempfile::TempDir::new().unwrap();
    let home_path = home_temp.path();

    project
        .acton()
        .env("HOME", home_path.to_str().unwrap())
        .library()
        .publish()
        .contract("simple")
        .with_net("testnet")
        .with_duration("1d")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/library/test_library_publish_no_wallets.stderr.txt",
        );
}

#[test]
fn test_library_publish_unknown_wallet() {
    let project = ProjectBuilder::new("library-publish-invalid-net")
        .contract("simple", "fun main() {}")
        .build();

    let home_temp = tempfile::TempDir::new().unwrap();
    let home_path = home_temp.path();

    let toml_content = r#"[wallets.wallet]
kind = "v5r1"
workchain = 0
keys = { mnemonic = "number bone assume survey solar debris liquid destroy minute end edge fine exhaust ginger mirror tongue proof guide blossom parrot mechanic style dad dynamic" }

[wallets.wallet.expected]
address-testnet = "kQBBSo2ccLuHuGiTn1z9Lei17LfBVOPewQmFR8pA2dAv2ixT"
"#;
    fs::write(project.path().join("wallets.toml"), toml_content).expect("Write wallets.toml");

    project
        .acton()
        .env("HOME", home_path.to_str().unwrap())
        .library()
        .publish()
        .contract("simple")
        .with_net("testnet")
        .with_duration("1d")
        .wallet("unknown")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/library/test_library_publish_unknown_wallet.stderr.txt",
        );
}

#[test]
fn test_library_info_basic() {
    let project = ProjectBuilder::new("library-info-basic").build();
    let home_temp = tempfile::TempDir::new().unwrap();

    let toml_content = r#"[libraries.my-lib]
name = "MyLib"
hash = "b993c68c596425f05d1bc492d7c03e2979ab669901ed5a57e35e6dd4d6089d27"
code = "b5ee9c72..."
account = "EQD..."
duration = 31536000
network = "testnet"
timestamp = "2026-01-05T12:00:00Z"
last_topup_timestamp = "2026-01-05T12:00:00Z"
bits = 1024
cells = 4
"#;
    fs::write(project.path().join("libraries.toml"), toml_content).expect("Write libraries.toml");

    project
        .acton()
        .env("HOME", home_temp.path().to_str().unwrap())
        .library()
        .arg("info")
        .arg("my-lib")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/library/test_library_info_basic.stdout.txt",
        );
}

#[test]
fn test_library_info_not_found() {
    let project = ProjectBuilder::new("library-info-not-found").build();
    let home_temp = tempfile::TempDir::new().unwrap();

    let toml_content = r#"[libraries.my-lib]
name = "MyLib"
hash = "..."
code = "..."
account = "..."
duration = 100
network = "testnet"
timestamp = "2026-01-05T12:00:00Z"
last_topup_timestamp = "2026-01-05T12:00:00Z"
bits = 10
cells = 1
"#;
    fs::write(project.path().join("libraries.toml"), toml_content).expect("Write libraries.toml");

    project
        .acton()
        .env("HOME", home_temp.path().to_str().unwrap())
        .library()
        .arg("info")
        .arg("nonexistent")
        .run()
        .failure()
        .assert_stderr_contains(
            "Library nonexistent not found in libraries.toml and global.libraries.toml",
        )
        .assert_stderr_contains("Available libraries:")
        .assert_stderr_contains("my-lib");
}

#[test]
fn test_library_info_no_libraries() {
    let project = ProjectBuilder::new("library-info-no-libs").build();
    let home_temp = tempfile::TempDir::new().unwrap();

    project
        .acton()
        .env("HOME", home_temp.path().to_str().unwrap())
        .library()
        .arg("info")
        .arg("any")
        .run()
        .failure()
        .assert_stderr_contains(
            "No libraries configured in libraries.toml or global.libraries.toml",
        );
}

#[test]
fn test_library_info_global() {
    let project = ProjectBuilder::new("library-info-global").build();
    let home_temp = tempfile::TempDir::new().unwrap();
    let global_libs_dir = home_temp
        .path()
        .join(".config")
        .join("acton")
        .join("libraries");
    fs::create_dir_all(&global_libs_dir).expect("Create global libs dir");

    let toml_content = r#"[libraries.global-lib]
name = "GlobalLib"
hash = "..."
code = "..."
account = "..."
duration = 100
network = "mainnet"
timestamp = "2026-01-05T12:00:00Z"
last_topup_timestamp = "2026-01-05T12:00:00Z"
bits = 20
cells = 2
"#;
    fs::write(global_libs_dir.join("global.libraries.toml"), toml_content)
        .expect("Write global.libraries.toml");

    project
        .acton()
        .env("HOME", home_temp.path().to_str().unwrap())
        .library()
        .arg("info")
        .arg("global-lib")
        .run()
        .success()
        .assert_contains("Library:     global-lib")
        .assert_contains("Contract:    GlobalLib")
        .assert_contains("Network:     mainnet");
}

#[test]
fn test_library_publish_rejects_non_tolk_contract_source() {
    let project = ProjectBuilder::new("library-publish-non-tolk-source").build();

    let toml_content = r#"[package]
name = "library-publish-non-tolk-source"
description = ""
version = "0.1.0"

[contracts.simple]
display-name = "Simple"
src = "contracts/simple.fif"
"#;
    fs::write(project.path().join("Acton.toml"), toml_content).expect("Write Acton.toml");
    fs::create_dir_all(project.path().join("contracts")).expect("Create contracts directory");
    fs::write(project.path().join("contracts/simple.fif"), "TEST")
        .expect("Write non-tolk source file");

    project
        .acton()
        .library()
        .publish()
        .contract("simple")
        .with_net("testnet")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/library/test_library_publish_rejects_non_tolk_contract_source.stderr.txt",
        );
}

#[test]
fn test_library_topup_no_libraries() {
    let project = ProjectBuilder::new("library-topup-no-libraries").build();
    let home_temp = tempfile::TempDir::new().unwrap();

    project
        .acton()
        .env("HOME", home_temp.path().to_str().unwrap())
        .library()
        .arg("topup")
        .arg("any")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/library/test_library_topup_no_libraries.stderr.txt",
        );
}

#[test]
fn test_library_topup_not_found() {
    let project = ProjectBuilder::new("library-topup-not-found").build();
    let home_temp = tempfile::TempDir::new().unwrap();

    let toml_content = r#"[libraries.my-lib]
name = "MyLib"
hash = "..."
code = "..."
account = "..."
duration = 100
network = "testnet"
timestamp = "2026-01-05T12:00:00Z"
last_topup_timestamp = "2026-01-05T12:00:00Z"
bits = 10
cells = 1
"#;
    fs::write(project.path().join("libraries.toml"), toml_content).expect("Write libraries.toml");

    project
        .acton()
        .env("HOME", home_temp.path().to_str().unwrap())
        .library()
        .arg("topup")
        .arg("nonexistent")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/library/test_library_topup_not_found.stderr.txt",
        );
}

#[test]
fn test_library_topup_tonconnect_rejects_localnet() {
    let project = ProjectBuilder::new("library-topup-tonconnect-localnet").build();
    let home_temp = tempfile::TempDir::new().unwrap();

    let libraries_toml = r#"[libraries.my-lib]
name = "MyLib"
hash = "..."
code = "..."
account = "EQD..."
duration = 100
network = "localnet"
timestamp = "2026-01-05T12:00:00Z"
last_topup_timestamp = "2026-01-05T12:00:00Z"
bits = 10
cells = 1
"#;
    fs::write(project.path().join("libraries.toml"), libraries_toml).expect("Write libraries.toml");

    project
        .acton()
        .env("HOME", home_temp.path().to_str().unwrap())
        .library()
        .arg("topup")
        .arg("my-lib")
        .arg("--tonconnect")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/library/test_library_topup_tonconnect_rejects_localnet.stderr.txt",
        );
}

#[test]
fn test_library_topup_invalid_duration() {
    let project = ProjectBuilder::new("library-topup-invalid-duration").build();
    let home_temp = tempfile::TempDir::new().unwrap();

    let libraries_toml = r#"[libraries.my-lib]
name = "MyLib"
hash = "..."
code = "..."
account = "EQD..."
duration = 100
network = "testnet"
timestamp = "2026-01-05T12:00:00Z"
last_topup_timestamp = "2026-01-05T12:00:00Z"
bits = 10
cells = 1
"#;
    fs::write(project.path().join("libraries.toml"), libraries_toml).expect("Write libraries.toml");

    let wallets_toml = r#"[wallets.wallet]
kind = "v5r1"
workchain = 0
keys = { mnemonic = "number bone assume survey solar debris liquid destroy minute end edge fine exhaust ginger mirror tongue proof guide blossom parrot mechanic style dad dynamic" }

[wallets.wallet.expected]
address-testnet = "kQBBSo2ccLuHuGiTn1z9Lei17LfBVOPewQmFR8pA2dAv2ixT"
"#;
    fs::write(project.path().join("wallets.toml"), wallets_toml).expect("Write wallets.toml");

    project
        .acton()
        .env("HOME", home_temp.path().to_str().unwrap())
        .library()
        .arg("topup")
        .arg("my-lib")
        .wallet("wallet")
        .with_duration("invalid-duration")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/library/test_library_topup_invalid_duration.stderr.txt",
        );
}

#[test]
fn test_library_topup_invalid_amount() {
    let project = ProjectBuilder::new("library-topup-invalid-amount").build();
    let home_temp = tempfile::TempDir::new().unwrap();

    let libraries_toml = r#"[libraries.my-lib]
name = "MyLib"
hash = "..."
code = "..."
account = "EQD..."
duration = 100
network = "testnet"
timestamp = "2026-01-05T12:00:00Z"
last_topup_timestamp = "2026-01-05T12:00:00Z"
bits = 10
cells = 1
"#;
    fs::write(project.path().join("libraries.toml"), libraries_toml).expect("Write libraries.toml");

    let wallets_toml = r#"[wallets.wallet]
kind = "v5r1"
workchain = 0
keys = { mnemonic = "number bone assume survey solar debris liquid destroy minute end edge fine exhaust ginger mirror tongue proof guide blossom parrot mechanic style dad dynamic" }

[wallets.wallet.expected]
address-testnet = "kQBBSo2ccLuHuGiTn1z9Lei17LfBVOPewQmFR8pA2dAv2ixT"
"#;
    fs::write(project.path().join("wallets.toml"), wallets_toml).expect("Write wallets.toml");

    project
        .acton()
        .env("HOME", home_temp.path().to_str().unwrap())
        .library()
        .arg("topup")
        .arg("my-lib")
        .wallet("wallet")
        .arg("--amount")
        .arg("1.2.3")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/library/test_library_topup_invalid_amount.stderr.txt",
        );
}

#[cfg(unix)]
#[test]
fn test_library_publish_interactive_cancel_confirmation() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("library-publish-interactive-cancel").build();
    let home_temp = tempfile::TempDir::new().expect("failed to create home temp dir");

    let wallets_toml = r#"[wallets.wallet]
kind = "v5r1"
workchain = 0
keys = { mnemonic = "number bone assume survey solar debris liquid destroy minute end edge fine exhaust ginger mirror tongue proof guide blossom parrot mechanic style dad dynamic" }

[wallets.wallet.expected]
address-testnet = "kQBBSo2ccLuHuGiTn1z9Lei17LfBVOPewQmFR8pA2dAv2ixT"
"#;
    fs::write(project.path().join("wallets.toml"), wallets_toml).expect("Write wallets.toml");

    let mut session = project
        .acton()
        .env(
            "HOME",
            home_temp.path().to_str().expect("home path should be utf8"),
        )
        .library()
        .publish()
        .with_code("te6cckEBAQEAAgAAAEysuc0=")
        .with_duration("1d")
        .wallet("wallet")
        .with_net("testnet")
        .arg("--amount")
        .arg("1")
        .arg("--local")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(20)));

    session.expect("Send 1 GRAM to publish library? Note that any extra GRAM will be refunded.");
    session.send_line("No", "failed to send cancellation response");
    session.expect(Eof);

    assert!(
        !project.path().join("libraries.toml").exists(),
        "libraries.toml should not be created when publish is cancelled"
    );
}

#[cfg(unix)]
#[test]
fn test_library_publish_prompts_to_topup_tracked_exact_match() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("library-publish-topup-tracked-exact-match").build();
    let home_temp = tempfile::TempDir::new().expect("failed to create home temp dir");
    write_deployer_wallets(project.path());
    write_publish_library_metadata_file(
        &project.path().join("libraries.toml"),
        "tracked-lib",
        "testnet",
        "2026-01-05T12:00:00Z",
    );

    let mut session = project
        .acton()
        .env(
            "HOME",
            home_temp.path().to_str().expect("home path should be utf8"),
        )
        .library()
        .publish()
        .with_code(PUBLISH_TEST_CODE_ARG)
        .with_duration("1d")
        .wallet("deployer")
        .with_net("testnet")
        .arg("--amount")
        .arg("1")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(20)));

    session.expect("Library with this hash is already tracked on testnet:");
    session.expect("Top up an existing tracked library instead of publishing a new one?");
    session.send_line("Yes", "failed to choose tracked library top-up");
    session.expect("Send 1 GRAM to top-up library?");
    session.send_line("No", "failed to cancel tracked library top-up");
    session.expect(Eof);
    session.assert_file_snapshot_matches(
        "libraries.toml",
        "integration/snapshots/library/test_library_publish_prompts_to_topup_tracked_exact_match.libraries.toml",
    );
}

#[cfg(unix)]
#[test]
fn test_library_publish_prompts_to_topup_tracked_exact_match_from_global_libraries() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("library-publish-topup-global-exact-match").build();
    let home_temp = tempfile::TempDir::new().expect("failed to create home temp dir");
    write_deployer_wallets(project.path());
    let global_path = home_temp
        .path()
        .join(".config")
        .join("acton")
        .join("libraries")
        .join("global.libraries.toml");
    write_publish_library_metadata_file(
        &global_path,
        "global-tracked-lib",
        "testnet",
        "2026-01-05T12:00:00Z",
    );

    let mut session = project
        .acton()
        .env(
            "HOME",
            home_temp.path().to_str().expect("home path should be utf8"),
        )
        .library()
        .publish()
        .with_code(PUBLISH_TEST_CODE_ARG)
        .with_duration("1d")
        .wallet("deployer")
        .with_net("testnet")
        .arg("--amount")
        .arg("1")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(20)));

    session.expect("Library with this hash is already tracked on testnet:");
    session.expect("global-tracked-lib in global.libraries.toml");
    session.expect("Top up an existing tracked library instead of publishing a new one?");
    session.send_line("Yes", "failed to choose tracked library top-up");
    session.expect("Send 1 GRAM to top-up library?");
    session.send_line("No", "failed to cancel tracked library top-up");
    session.expect(Eof);

    fs::copy(&global_path, project.path().join("global.libraries.toml"))
        .expect("failed to copy global libraries file for snapshot assertion");
    session.assert_file_snapshot_matches(
        "global.libraries.toml",
        "integration/snapshots/library/test_library_publish_prompts_to_topup_tracked_exact_match_from_global_libraries.global.libraries.toml",
    );
}

#[allow(clippy::significant_drop_tightening)]
#[cfg(unix)]
#[test]
fn test_library_publish_selects_specific_tracked_match_and_updates_global_topup_timestamp() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("library-publish-topup-selects-global-match").build();
    let home_temp = tempfile::TempDir::new().expect("failed to create home temp dir");
    write_deployer_wallets(project.path());
    let (mock_url, mock_handle, captured) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_seqno_ok_response(),
        toncenter_v2_send_boc_ok_response(),
    ]);
    append_custom_network(project.path(), "mock-v2", &mock_url);

    let local_path = project.path().join("libraries.toml");
    write_publish_library_metadata_file(
        &local_path,
        "local-tracked-lib",
        "custom:mock-v2",
        "2026-01-05T12:00:00Z",
    );
    let global_path = home_temp
        .path()
        .join(".config")
        .join("acton")
        .join("libraries")
        .join("global.libraries.toml");
    write_publish_library_metadata_file(
        &global_path,
        "global-tracked-lib",
        "custom:mock-v2",
        "2026-01-05T12:00:00Z",
    );

    let before_local = read_library_entry(&local_path, "local-tracked-lib");
    let before_global = read_library_entry(&global_path, "global-tracked-lib");
    let mut session = project
        .acton()
        .env(
            "HOME",
            home_temp.path().to_str().expect("home path should be utf8"),
        )
        .library()
        .publish()
        .with_code(PUBLISH_TEST_CODE_ARG)
        .with_duration("1d")
        .wallet("deployer")
        .arg("--net")
        .arg("custom:mock-v2")
        .arg("--amount")
        .arg("1")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(30)));

    session.expect("Library with this hash is already tracked on mock-v2:");
    session.expect("local-tracked-lib in local libraries.toml");
    session.expect("global-tracked-lib in global.libraries.toml");
    session.expect("Top up an existing tracked library instead of publishing a new one?");
    session.send_line("Yes", "failed to choose tracked library top-up");
    session.expect("Select tracked library to top up:");
    session.send_line(
        "global-tracked-lib",
        "failed to select global tracked library",
    );
    session.expect("Send 1 GRAM to top-up library?");
    session.send_line("Yes", "failed to confirm selected tracked library top-up");
    session.expect("Top-up transaction sent successfully");
    session.expect(Eof);

    let after_local = read_library_entry(&local_path, "local-tracked-lib");
    let after_global = read_library_entry(&global_path, "global-tracked-lib");
    assert_eq!(
        before_local.last_topup_timestamp, after_local.last_topup_timestamp,
        "local tracked library timestamp should stay unchanged when global match is selected"
    );
    assert_ne!(
        before_global.last_topup_timestamp, after_global.last_topup_timestamp,
        "selected global tracked library timestamp should be updated after top-up"
    );

    mock_handle.join().expect("mock toncenter v2 must finish");
    let captured = captured
        .lock()
        .expect("captured toncenter v2 requests mutex poisoned");
    assert_eq!(captured.len(), 2, "expected seqno + sendBoc requests");
}

#[allow(clippy::significant_drop_tightening)]
#[cfg(unix)]
#[test]
fn test_library_publish_declines_tracked_topup_and_continues_publish() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("library-publish-decline-tracked-topup").build();
    let home_temp = tempfile::TempDir::new().expect("failed to create home temp dir");
    write_deployer_wallets(project.path());
    let (mock_url, mock_handle, captured) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_get_libraries_not_found_response(),
        toncenter_v2_seqno_ok_response(),
        toncenter_v2_send_boc_ok_response(),
    ]);
    append_custom_network(project.path(), "mock-v2", &mock_url);
    let libraries_path = project.path().join("libraries.toml");
    write_publish_library_metadata_file(
        &libraries_path,
        "tracked-lib",
        "custom:mock-v2",
        "2026-01-05T12:00:00Z",
    );

    let mut session = project
        .acton()
        .env(
            "HOME",
            home_temp.path().to_str().expect("home path should be utf8"),
        )
        .library()
        .publish()
        .with_code(PUBLISH_TEST_CODE_ARG)
        .with_duration("1d")
        .wallet("deployer")
        .arg("--net")
        .arg("custom:mock-v2")
        .arg("--amount")
        .arg("1")
        .arg("--local")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(30)));

    session.expect("Top up an existing tracked library instead of publishing a new one?");
    session.send_line("No", "failed to decline tracked library top-up");
    session.expect("Send 1 GRAM to publish library? Note that any extra GRAM will be refunded.");
    session.send_line("Yes", "failed to confirm publish after declining top-up");
    session.expect("Transaction sent successfully");
    session.expect("Library info saved");
    session.expect(Eof);

    let content = fs::read_to_string(&libraries_path).expect("failed to read libraries.toml");
    assert!(
        content.contains("[libraries.tracked-lib]") && content.contains("[libraries.unknown]"),
        "declining top-up should continue publish and append new metadata, got:\n{content}"
    );

    mock_handle.join().expect("mock toncenter v2 must finish");
    let captured = captured
        .lock()
        .expect("captured toncenter v2 requests mutex poisoned");
    assert_eq!(
        captured.len(),
        3,
        "expected getLibraries check + seqno + sendBoc requests"
    );
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_library_publish_yes_with_tracked_match_warns_and_continues_publish() {
    let project = ProjectBuilder::new("library-publish-yes-tracked-match-warns").build();
    let home_temp = tempfile::TempDir::new().expect("failed to create home temp dir");
    write_deployer_wallets(project.path());
    let (mock_url, mock_handle, captured) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_get_libraries_not_found_response(),
        toncenter_v2_seqno_ok_response(),
        toncenter_v2_send_boc_ok_response(),
    ]);
    append_custom_network(project.path(), "mock-v2", &mock_url);
    write_publish_library_metadata_file(
        &project.path().join("libraries.toml"),
        "tracked-lib",
        "custom:mock-v2",
        "2026-01-05T12:00:00Z",
    );

    project
        .acton()
        .env(
            "HOME",
            home_temp.path().to_str().expect("home path should be utf8"),
        )
        .library()
        .publish()
        .with_code(PUBLISH_TEST_CODE_ARG)
        .with_duration("1d")
        .wallet("deployer")
        .arg("--net")
        .arg("custom:mock-v2")
        .arg("--amount")
        .arg("1")
        .arg("--yes")
        .arg("--local")
        .run()
        .success()
        .assert_contains("Library with this hash is already tracked on mock-v2:")
        .assert_contains("Library info saved")
        .assert_not_contains("Top up an existing tracked library instead of publishing a new one?");

    mock_handle.join().expect("mock toncenter v2 must finish");
    let captured = captured
        .lock()
        .expect("captured toncenter v2 requests mutex poisoned");
    assert_eq!(
        captured.len(),
        3,
        "--yes should skip tracked prompts but still run warning-only on-chain getLibraries check"
    );
}

#[allow(clippy::significant_drop_tightening)]
#[cfg(unix)]
#[test]
fn test_library_publish_warns_on_onchain_match_and_continues_publish() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("library-publish-onchain-match-warning").build();
    let home_temp = tempfile::TempDir::new().expect("failed to create home temp dir");
    write_deployer_wallets(project.path());
    let (mock_url, mock_handle, captured) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_get_libraries_ok_response(PUBLISH_TEST_CODE_ARG),
        toncenter_v2_seqno_ok_response(),
        toncenter_v2_send_boc_ok_response(),
    ]);
    append_custom_network(project.path(), "mock-v2", &mock_url);

    let mut session = project
        .acton()
        .env(
            "HOME",
            home_temp.path().to_str().expect("home path should be utf8"),
        )
        .library()
        .publish()
        .with_code(PUBLISH_TEST_CODE_ARG)
        .with_duration("1d")
        .wallet("deployer")
        .arg("--net")
        .arg("custom:mock-v2")
        .arg("--amount")
        .arg("1")
        .arg("--local")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(30)));

    session.expect("Send 1 GRAM to publish library? Note that any extra GRAM will be refunded.");
    session.send_line("Yes", "failed to confirm publish");
    session.expect("Library code with this hash is already available on-chain on mock-v2.");
    session.expect("Transaction sent successfully");
    session.expect("Library info saved");
    session.expect(Eof);

    mock_handle.join().expect("mock toncenter v2 must finish");
    let captured = captured
        .lock()
        .expect("captured toncenter v2 requests mutex poisoned");
    assert_eq!(
        captured.len(),
        3,
        "expected getLibraries warning check + seqno + sendBoc requests"
    );
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_library_publish_yes_warns_on_onchain_match_and_continues_publish() {
    let project = ProjectBuilder::new("library-publish-yes-onchain-match-warning").build();
    let home_temp = tempfile::TempDir::new().expect("failed to create home temp dir");
    write_deployer_wallets(project.path());
    let (mock_url, mock_handle, captured) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_get_libraries_ok_response(PUBLISH_TEST_CODE_ARG),
        toncenter_v2_seqno_ok_response(),
        toncenter_v2_send_boc_ok_response(),
    ]);
    append_custom_network(project.path(), "mock-v2", &mock_url);

    project
        .acton()
        .env(
            "HOME",
            home_temp.path().to_str().expect("home path should be utf8"),
        )
        .library()
        .publish()
        .with_code(PUBLISH_TEST_CODE_ARG)
        .with_duration("1d")
        .wallet("deployer")
        .arg("--net")
        .arg("custom:mock-v2")
        .arg("--amount")
        .arg("1")
        .arg("--yes")
        .arg("--local")
        .run()
        .success()
        .assert_contains("Library code with this hash is already available on-chain on mock-v2.")
        .assert_contains("Library info saved");

    mock_handle.join().expect("mock toncenter v2 must finish");
    let captured = captured
        .lock()
        .expect("captured toncenter v2 requests mutex poisoned");
    assert_eq!(
        captured.len(),
        3,
        "expected getLibraries warning check + seqno + sendBoc requests"
    );
}

#[allow(clippy::significant_drop_tightening)]
#[cfg(unix)]
#[test]
fn test_library_publish_ignores_onchain_check_error_and_continues_publish() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("library-publish-onchain-check-error-ignored").build();
    let home_temp = tempfile::TempDir::new().expect("failed to create home temp dir");
    write_deployer_wallets(project.path());
    let (mock_url, mock_handle, captured) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_get_libraries_error_response("mock getLibraries failure"),
        toncenter_v2_get_libraries_error_response("mock getLibraries failure"),
        toncenter_v2_get_libraries_error_response("mock getLibraries failure"),
        toncenter_v2_seqno_ok_response(),
        toncenter_v2_send_boc_ok_response(),
    ]);
    append_custom_network(project.path(), "mock-v2", &mock_url);

    let mut session = project
        .acton()
        .env(
            "HOME",
            home_temp.path().to_str().expect("home path should be utf8"),
        )
        .library()
        .publish()
        .with_code(PUBLISH_TEST_CODE_ARG)
        .with_duration("1d")
        .wallet("deployer")
        .arg("--net")
        .arg("custom:mock-v2")
        .arg("--amount")
        .arg("1")
        .arg("--local")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(30)));

    session.expect("Send 1 GRAM to publish library? Note that any extra GRAM will be refunded.");
    session.send_line("Yes", "failed to confirm publish");
    session.expect("Transaction sent successfully");
    session.expect("Library info saved");
    session.expect(Eof);

    mock_handle.join().expect("mock toncenter v2 must finish");
    let captured = captured
        .lock()
        .expect("captured toncenter v2 requests mutex poisoned");
    assert_eq!(
        captured.len(),
        5,
        "expected retried getLibraries check + seqno + sendBoc requests"
    );
}

#[cfg(unix)]
#[test]
fn test_library_publish_same_hash_different_network_does_not_prompt_to_topup() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("library-publish-same-hash-different-network").build();
    let home_temp = tempfile::TempDir::new().expect("failed to create home temp dir");
    write_deployer_wallets(project.path());
    write_publish_library_metadata_file(
        &project.path().join("libraries.toml"),
        "tracked-on-mainnet",
        "mainnet",
        "2026-01-05T12:00:00Z",
    );

    let mut session = project
        .acton()
        .env(
            "HOME",
            home_temp.path().to_str().expect("home path should be utf8"),
        )
        .library()
        .publish()
        .with_code(PUBLISH_TEST_CODE_ARG)
        .with_duration("1d")
        .wallet("deployer")
        .with_net("testnet")
        .arg("--amount")
        .arg("1")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(20)));

    session.expect("Send 1 GRAM to publish library? Note that any extra GRAM will be refunded.");
    session.send_line(
        "No",
        "failed to cancel publish after skipping top-up prompt",
    );
    session.expect(Eof);
}

#[cfg(unix)]
#[test]
fn test_library_topup_interactive_cancel_confirmation() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("library-topup-interactive-cancel").build();

    let home_temp = tempfile::TempDir::new().unwrap();
    let libraries_toml = r#"[libraries.my-lib]
name = "MyLib"
hash = "..."
code = "..."
account = "EQD..."
duration = 100
network = "testnet"
timestamp = "2026-01-05T12:00:00Z"
last_topup_timestamp = "2026-01-05T12:00:00Z"
bits = 10
cells = 1
"#;
    fs::write(project.path().join("libraries.toml"), libraries_toml).expect("Write libraries.toml");

    let wallets_toml = r#"[wallets.wallet]
kind = "v5r1"
workchain = 0
keys = { mnemonic = "number bone assume survey solar debris liquid destroy minute end edge fine exhaust ginger mirror tongue proof guide blossom parrot mechanic style dad dynamic" }

[wallets.wallet.expected]
address-testnet = "kQBBSo2ccLuHuGiTn1z9Lei17LfBVOPewQmFR8pA2dAv2ixT"
"#;
    fs::write(project.path().join("wallets.toml"), wallets_toml).expect("Write wallets.toml");

    let mut session = project
        .acton()
        .env("HOME", home_temp.path().to_str().unwrap())
        .library()
        .arg("topup")
        .arg("my-lib")
        .wallet("wallet")
        .arg("--amount")
        .arg("1")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(20)));

    session.expect("Send 1 GRAM to top-up library?");
    session.send_line("No", "failed to send cancellation response");
    session.expect(Eof);
    session.assert_file_snapshot_matches(
        "libraries.toml",
        "integration/snapshots/library/test_library_topup_interactive_cancel.libraries.toml",
    );
}

#[test]
fn test_library_publish_happy_path_localnet_saves_local_metadata() {
    let project = ProjectBuilder::new("library-publish-localnet-local")
        .contract("library_contract", LOCALNET_LIBRARY_CONTRACT)
        .build();
    write_deployer_wallets(project.path());
    let node = start_localnet_with_localnet(&project);

    let output = project
        .acton()
        .library()
        .publish()
        .arg("library_contract")
        .arg("--wallet")
        .arg("deployer")
        .arg("--net")
        .arg("localnet")
        .arg("--duration")
        .arg("1y")
        .arg("--amount")
        .arg("5")
        .arg("--yes")
        .arg("--local")
        .run()
        .success();

    output
        .assert_contains("Transaction sent successfully")
        .assert_contains("Library info saved");

    let libraries_path = project.path().join("libraries.toml");
    assert!(
        libraries_path.exists(),
        "libraries.toml should be created for --local publish"
    );

    let (library_id, library) = read_first_library_entry(&libraries_path);
    assert_eq!(library_id, "library_contract");
    assert_eq!(library.network, "localnet");
    assert_eq!(library.hash.len(), 64);
    assert!(
        !library.account.is_empty(),
        "library account must be present"
    );

    wait_for_library_in_api(&node, &library.hash, Duration::from_secs(12));
    node.stop();
}

#[test]
fn test_library_publish_happy_path_localnet_saves_global_metadata_with_flag() {
    let project = ProjectBuilder::new("library-publish-localnet-global")
        .contract("library_contract", LOCALNET_LIBRARY_CONTRACT)
        .build();
    write_deployer_wallets(project.path());
    let home_temp = tempfile::TempDir::new().expect("failed to create home temp dir");
    let node = start_localnet_with_localnet(&project);

    project
        .acton()
        .env(
            "HOME",
            home_temp.path().to_str().expect("home path should be utf8"),
        )
        .library()
        .publish()
        .arg("library_contract")
        .arg("--wallet")
        .arg("deployer")
        .arg("--net")
        .arg("localnet")
        .arg("--duration")
        .arg("1y")
        .arg("--amount")
        .arg("5")
        .arg("--yes")
        .arg("--global")
        .run()
        .success()
        .assert_contains("Library info saved");

    assert!(
        !project.path().join("libraries.toml").exists(),
        "libraries.toml should not be created for --global publish"
    );

    let global_path = home_temp
        .path()
        .join(".config")
        .join("acton")
        .join("libraries")
        .join("global.libraries.toml");
    assert!(
        global_path.exists(),
        "global.libraries.toml should be created for --global publish"
    );

    let (_, library) = read_first_library_entry(&global_path);
    assert_eq!(library.network, "localnet");
    assert_eq!(library.hash.len(), 64);

    wait_for_library_in_api(&node, &library.hash, Duration::from_secs(12));
    node.stop();
}

#[cfg(unix)]
#[test]
fn test_library_publish_interactive_save_location_defaults_to_local_localnet() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("library-publish-localnet-interactive-save")
        .contract("library_contract", LOCALNET_LIBRARY_CONTRACT)
        .build();
    write_deployer_wallets(project.path());
    let home_temp = tempfile::TempDir::new().expect("failed to create home temp dir");
    let node = start_localnet_with_localnet(&project);

    let mut session = project
        .acton()
        .env(
            "HOME",
            home_temp.path().to_str().expect("home path should be utf8"),
        )
        .library()
        .publish()
        .arg("library_contract")
        .arg("--wallet")
        .arg("deployer")
        .arg("--net")
        .arg("localnet")
        .arg("--duration")
        .arg("1y")
        .arg("--amount")
        .arg("5")
        .arg("--yes")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(30)));

    session.expect("Save library info to:");
    session.send_line("", "failed to select default local storage");
    session.expect(Eof);

    assert!(
        project.path().join("libraries.toml").exists(),
        "interactive publish should save to local file by default"
    );

    let global_path = home_temp
        .path()
        .join(".config")
        .join("acton")
        .join("libraries")
        .join("global.libraries.toml");
    assert!(
        !global_path.exists(),
        "interactive publish should not save to global file when default option is selected"
    );

    node.stop();
}

#[test]
fn test_library_topup_happy_path_localnet_updates_last_topup_timestamp() {
    let project = ProjectBuilder::new("library-topup-localnet-happy-path")
        .contract("library_contract", LOCALNET_LIBRARY_CONTRACT)
        .build();
    write_deployer_wallets(project.path());
    let node = start_localnet_with_localnet(&project);

    project
        .acton()
        .library()
        .publish()
        .arg("library_contract")
        .arg("--wallet")
        .arg("deployer")
        .arg("--net")
        .arg("localnet")
        .arg("--duration")
        .arg("1y")
        .arg("--amount")
        .arg("5")
        .arg("--yes")
        .arg("--local")
        .run()
        .success();

    let libraries_path = project.path().join("libraries.toml");
    let (library_id, before_topup) = read_first_library_entry(&libraries_path);
    wait_for_library_in_api(&node, &before_topup.hash, Duration::from_secs(12));
    wait_until_address_state_active(&node, &before_topup.account, Duration::from_secs(12));

    thread::sleep(Duration::from_secs(1));

    project
        .acton()
        .library()
        .arg("topup")
        .arg(&library_id)
        .arg("--wallet")
        .arg("deployer")
        .arg("--amount")
        .arg("1")
        .arg("--yes")
        .run()
        .success()
        .assert_contains("Top-up transaction sent successfully");

    let (_, after_topup) = read_first_library_entry(&libraries_path);
    assert_ne!(
        before_topup.last_topup_timestamp, after_topup.last_topup_timestamp,
        "last_topup_timestamp should be updated after successful topup"
    );

    node.stop();
}

#[test]
fn test_library_info_shows_balance_and_runway_on_localnet() {
    let project = ProjectBuilder::new("library-info-localnet-balance-runway")
        .contract("library_contract", LOCALNET_LIBRARY_CONTRACT)
        .build();
    write_deployer_wallets(project.path());
    let node = start_localnet_with_localnet(&project);

    project
        .acton()
        .library()
        .publish()
        .arg("library_contract")
        .arg("--wallet")
        .arg("deployer")
        .arg("--net")
        .arg("localnet")
        .arg("--duration")
        .arg("1y")
        .arg("--amount")
        .arg("5")
        .arg("--yes")
        .arg("--local")
        .run()
        .success();

    let libraries_path = project.path().join("libraries.toml");
    let (library_id, library) = read_first_library_entry(&libraries_path);
    wait_for_library_in_api(&node, &library.hash, Duration::from_secs(12));
    wait_until_address_state_active(&node, &library.account, Duration::from_secs(12));

    project
        .acton()
        .library()
        .arg("info")
        .arg(&library_id)
        .run()
        .success()
        .assert_contains("Library:")
        .assert_contains("Balance:")
        .assert_contains("Remaining:");

    node.stop();
}

#[test]
fn test_library_info_shows_runway_warning_when_exhausted_on_localnet() {
    let project = ProjectBuilder::new("library-info-localnet-runway-warning")
        .contract("library_contract", LOCALNET_LIBRARY_CONTRACT)
        .build();
    write_deployer_wallets(project.path());
    let node = start_localnet_with_localnet(&project);

    project
        .acton()
        .library()
        .publish()
        .arg("library_contract")
        .arg("--wallet")
        .arg("deployer")
        .arg("--net")
        .arg("localnet")
        .arg("--duration")
        .arg("1y")
        .arg("--amount")
        .arg("5")
        .arg("--yes")
        .arg("--local")
        .run()
        .success();

    let libraries_path = project.path().join("libraries.toml");
    let (library_id, library) = read_first_library_entry(&libraries_path);
    wait_for_library_in_api(&node, &library.hash, Duration::from_secs(12));
    wait_until_address_state_active(&node, &library.account, Duration::from_secs(12));
    mark_library_runway_exhausted(&libraries_path, &library_id);

    project
        .acton()
        .library()
        .arg("info")
        .arg(&library_id)
        .run()
        .success()
        .assert_contains("Storage runway is exhausted");

    node.stop();
}

#[test]
fn test_library_fetch_json_with_output_behavior_is_stable_on_localnet() {
    let project = ProjectBuilder::new("library-fetch-json-output-localnet")
        .contract("library_contract", LOCALNET_LIBRARY_CONTRACT)
        .build();
    write_deployer_wallets(project.path());
    let node = start_localnet_with_localnet(&project);

    project
        .acton()
        .library()
        .publish()
        .arg("library_contract")
        .arg("--wallet")
        .arg("deployer")
        .arg("--net")
        .arg("localnet")
        .arg("--duration")
        .arg("1y")
        .arg("--amount")
        .arg("5")
        .arg("--yes")
        .arg("--local")
        .run()
        .success();

    let libraries_path = project.path().join("libraries.toml");
    let (_, library) = read_first_library_entry(&libraries_path);
    wait_for_library_in_api(&node, &library.hash, Duration::from_secs(12));

    let output_file = "json-output-should-not-be-written.boc";
    let output = project
        .acton()
        .library()
        .fetch(&library.hash)
        .arg("--net")
        .arg("localnet")
        .arg("--json")
        .arg("--output")
        .arg(output_file)
        .run()
        .success();

    let payload: JsonValue =
        serde_json::from_str(&output.get_stdout()).expect("fetch --json must print JSON payload");
    assert_eq!(payload["success"].as_bool(), Some(true));
    assert!(
        payload["code_boc64"]
            .as_str()
            .is_some_and(|value| !value.is_empty()),
        "fetch --json payload must include code_boc64"
    );
    assert!(
        !project.path().join(output_file).exists(),
        "fetch --json should not create output file when --output is also provided"
    );

    node.stop();
}

#[test]
fn test_library_fetch_json_with_disasm_behavior_is_stable_on_localnet() {
    let project = ProjectBuilder::new("library-fetch-json-disasm-localnet")
        .contract("library_contract", LOCALNET_LIBRARY_CONTRACT)
        .build();
    write_deployer_wallets(project.path());
    let node = start_localnet_with_localnet(&project);

    project
        .acton()
        .library()
        .publish()
        .arg("library_contract")
        .arg("--wallet")
        .arg("deployer")
        .arg("--net")
        .arg("localnet")
        .arg("--duration")
        .arg("1y")
        .arg("--amount")
        .arg("5")
        .arg("--yes")
        .arg("--local")
        .run()
        .success();

    let libraries_path = project.path().join("libraries.toml");
    let (_, library) = read_first_library_entry(&libraries_path);
    wait_for_library_in_api(&node, &library.hash, Duration::from_secs(12));

    let output = project
        .acton()
        .library()
        .fetch(&library.hash)
        .arg("--net")
        .arg("localnet")
        .arg("--json")
        .arg("--disasm")
        .run()
        .success();

    let stdout = output.get_stdout();
    assert!(
        !stdout.trim().is_empty(),
        "fetch --json --disasm should print disassembly output"
    );
    assert!(
        serde_json::from_str::<JsonValue>(&stdout).is_err(),
        "fetch --json --disasm should not output JSON when disassembly path is used"
    );
    output.assert_not_contains("\"success\": true");

    node.stop();
}

#[test]
fn test_library_publish_custom_network_unknown_fails() {
    let project = ProjectBuilder::new("library-publish-custom-network-unknown").build();
    write_deployer_wallets(project.path());

    project
        .acton()
        .library()
        .publish()
        .with_code("te6cckEBAQEAAgAAAEysuc0=")
        .with_duration("1d")
        .wallet("deployer")
        .arg("--amount")
        .arg("1")
        .arg("--yes")
        .arg("--local")
        .arg("--net")
        .arg("custom:missing-network")
        .run()
        .failure()
        .assert_stderr_contains("unknown custom network: missing-network");
}

#[test]
fn test_library_publish_custom_network_missing_v2_url_fails() {
    let project = ProjectBuilder::new("library-publish-custom-network-missing-v2").build();
    write_deployer_wallets(project.path());

    let acton_toml_path = project.path().join("Acton.toml");
    let mut acton_toml =
        fs::read_to_string(&acton_toml_path).expect("failed to read generated Acton.toml");
    acton_toml.push_str(
        r#"

[networks.broken]
api = { v3 = "http://127.0.0.1:1/api/v3" }
"#,
    );
    fs::write(&acton_toml_path, acton_toml).expect("failed to write malformed custom network");

    project
        .acton()
        .library()
        .publish()
        .with_code("te6cckEBAQEAAgAAAEysuc0=")
        .with_duration("1d")
        .wallet("deployer")
        .arg("--amount")
        .arg("1")
        .arg("--yes")
        .arg("--local")
        .arg("--net")
        .arg("custom:broken")
        .run()
        .failure()
        .assert_stderr_contains("unknown custom network: broken");
}

#[cfg(unix)]
#[test]
fn test_library_publish_interactive_selects_global_storage() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("library-publish-localnet-interactive-global")
        .contract("library_contract", LOCALNET_LIBRARY_CONTRACT)
        .build();
    write_deployer_wallets(project.path());
    let home_temp = tempfile::TempDir::new().expect("failed to create home temp dir");
    let node = start_localnet_with_localnet(&project);

    let mut session = project
        .acton()
        .env(
            "HOME",
            home_temp.path().to_str().expect("home path should be utf8"),
        )
        .library()
        .publish()
        .arg("library_contract")
        .arg("--wallet")
        .arg("deployer")
        .arg("--net")
        .arg("localnet")
        .arg("--duration")
        .arg("1y")
        .arg("--amount")
        .arg("5")
        .arg("--yes")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(30)));

    session.expect("Save library info to:");
    session.send_line("Global", "failed to select global storage");
    session.expect(Eof);

    assert!(
        !project.path().join("libraries.toml").exists(),
        "libraries.toml should not be created when global storage is selected interactively"
    );

    let global_path = home_temp
        .path()
        .join(".config")
        .join("acton")
        .join("libraries")
        .join("global.libraries.toml");
    assert!(
        global_path.exists(),
        "global.libraries.toml should be created when global storage is selected interactively"
    );

    let (_, library) = read_first_library_entry(&global_path);
    wait_for_library_in_api(&node, &library.hash, Duration::from_secs(12));
    node.stop();
}

#[cfg(unix)]
#[test]
fn test_library_topup_reports_metadata_update_failure_after_successful_send() {
    use std::os::unix::fs::PermissionsExt;

    let project = ProjectBuilder::new("library-topup-metadata-update-failure")
        .contract("library_contract", LOCALNET_LIBRARY_CONTRACT)
        .build();
    write_deployer_wallets(project.path());
    let node = start_localnet_with_localnet(&project);

    project
        .acton()
        .library()
        .publish()
        .arg("library_contract")
        .arg("--wallet")
        .arg("deployer")
        .arg("--net")
        .arg("localnet")
        .arg("--duration")
        .arg("1y")
        .arg("--amount")
        .arg("5")
        .arg("--yes")
        .arg("--local")
        .run()
        .success();

    let libraries_path = project.path().join("libraries.toml");
    let (library_id, before_topup) = read_first_library_entry(&libraries_path);
    wait_for_library_in_api(&node, &before_topup.hash, Duration::from_secs(12));
    wait_until_address_state_active(&node, &before_topup.account, Duration::from_secs(12));

    let mut permissions = fs::metadata(&libraries_path)
        .expect("failed to read libraries.toml metadata")
        .permissions();
    permissions.set_mode(0o444);
    fs::set_permissions(&libraries_path, permissions)
        .expect("failed to make libraries.toml read-only");

    let output = project
        .acton()
        .library()
        .arg("topup")
        .arg(&library_id)
        .arg("--wallet")
        .arg("deployer")
        .arg("--amount")
        .arg("1")
        .arg("--yes")
        .run()
        .failure();

    output
        .assert_contains("Top-up transaction sent successfully")
        .assert_contains("Top-up transaction was sent, but failed to update library metadata");

    let mut restore_permissions = fs::metadata(&libraries_path)
        .expect("failed to read libraries.toml metadata for restore")
        .permissions();
    restore_permissions.set_mode(0o644);
    fs::set_permissions(&libraries_path, restore_permissions)
        .expect("failed to restore libraries.toml permissions");

    node.stop();
}

#[cfg(unix)]
#[test]
fn test_library_info_interactive_library_select() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("library-info-interactive-select")
        .contract("library_contract", LOCALNET_LIBRARY_CONTRACT)
        .build();
    write_deployer_wallets(project.path());
    let node = start_localnet_with_localnet(&project);

    for _ in 0..2 {
        project
            .acton()
            .library()
            .publish()
            .arg("library_contract")
            .arg("--wallet")
            .arg("deployer")
            .arg("--net")
            .arg("localnet")
            .arg("--duration")
            .arg("1y")
            .arg("--amount")
            .arg("5")
            .arg("--yes")
            .arg("--local")
            .run()
            .success();
    }

    let mut session = project
        .acton()
        .library()
        .arg("info")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(30)));

    session.expect("Select library:");
    session.send_line("", "failed to select default library");
    session.expect("Library:");
    session.expect("library_contract");
    session.expect(Eof);

    node.stop();
}

#[cfg(unix)]
#[test]
fn test_library_topup_interactive_library_and_wallet_select() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("library-topup-interactive-library-wallet-select")
        .contract("library_contract", LOCALNET_LIBRARY_CONTRACT)
        .build();
    write_two_wallets(project.path());
    let node = start_localnet_with_localnet(&project);

    for _ in 0..2 {
        project
            .acton()
            .library()
            .publish()
            .arg("library_contract")
            .arg("--wallet")
            .arg("deployer")
            .arg("--net")
            .arg("localnet")
            .arg("--duration")
            .arg("1y")
            .arg("--amount")
            .arg("5")
            .arg("--yes")
            .arg("--local")
            .run()
            .success();
    }

    let mut session = project
        .acton()
        .library()
        .arg("topup")
        .arg("--amount")
        .arg("1")
        .arg("--yes")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(30)));

    session.expect("Select library to top up:");
    session.send_line("", "failed to select default library");
    session.expect("Multiple wallets configured. Please select which wallet to use:");
    session.send_line("", "failed to select default wallet");
    session.expect("Top-up transaction sent successfully");
    session.expect(Eof);

    node.stop();
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_library_fetch_uses_testnet_env_api_key() {
    let mock_response_body = serde_json::json!({
        "ok": true,
        "result": {
            "result": [{
                "found": true,
                "data": "te6cckEBAQEAAgAAAEysuc0="
            }]
        }
    })
    .to_string();

    let (mock_url, mock_handle, captured) =
        spawn_toncenter_v2_mock(vec![ToncenterV2MockResponse {
            status: 200,
            body: mock_response_body,
        }]);

    let project = ProjectBuilder::new("library-fetch-env-api-key").build();

    project
        .acton()
        .library()
        .fetch(LIB_HASH)
        .arg("--net")
        .arg("testnet")
        .arg("--json")
        .env(TEST_TONCENTER_TESTNET_V2_URL_ENV, &mock_url)
        .env(TONCENTER_TESTNET_API_KEY_ENV, "env-api-key")
        .run()
        .success()
        .assert_contains("\"success\":true");

    mock_handle.join().expect("mock toncenter v2 must finish");
    let captured = captured
        .lock()
        .expect("captured toncenter v2 requests mutex poisoned");
    assert_eq!(captured.len(), 1, "expected one getLibraries request");
    assert_eq!(captured[0].method, "GET");
    assert!(
        captured[0].path.starts_with("/getLibraries?libraries="),
        "unexpected path: {}",
        captured[0].path
    );
    let header = captured[0]
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("x-api-key"))
        .map(|(_, value)| value.as_str());
    assert_eq!(header, Some("env-api-key"));
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_library_fetch_uses_mainnet_env_api_key_for_mainnet() {
    let mock_response_body = serde_json::json!({
        "ok": true,
        "result": {
            "result": [{
                "found": true,
                "data": "te6cckEBAQEAAgAAAEysuc0="
            }]
        }
    })
    .to_string();

    let (mock_url, mock_handle, captured) =
        spawn_toncenter_v2_mock(vec![ToncenterV2MockResponse {
            status: 200,
            body: mock_response_body,
        }]);

    let project = ProjectBuilder::new("library-fetch-mainnet-env-api-key").build();

    project
        .acton()
        .library()
        .fetch(LIB_HASH)
        .arg("--net")
        .arg("mainnet")
        .arg("--json")
        .env(TEST_TONCENTER_MAINNET_V2_URL_ENV, &mock_url)
        .env(TONCENTER_MAINNET_API_KEY_ENV, "mainnet-api-key")
        .env(TONCENTER_TESTNET_API_KEY_ENV, "testnet-api-key")
        .run()
        .success()
        .assert_contains("\"success\":true");

    mock_handle.join().expect("mock toncenter v2 must finish");
    let captured = captured
        .lock()
        .expect("captured toncenter v2 requests mutex poisoned");
    assert_eq!(captured.len(), 1, "expected one getLibraries request");
    let header = captured[0]
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("x-api-key"))
        .map(|(_, value)| value.as_str());
    assert_eq!(header, Some("mainnet-api-key"));
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_library_topup_updates_global_metadata_when_local_missing() {
    let project = ProjectBuilder::new("library-topup-global-fallback").build();
    write_deployer_wallets(project.path());

    let (mock_url, mock_handle, captured) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_seqno_ok_response(),
        toncenter_v2_send_boc_ok_response(),
    ]);
    append_custom_network(project.path(), "mock-v2", &mock_url);

    let home_temp = tempfile::TempDir::new().expect("failed to create home temp dir");
    let global_path = home_temp
        .path()
        .join(".config")
        .join("acton")
        .join("libraries")
        .join("global.libraries.toml");
    write_library_metadata_file(
        &global_path,
        "global-lib",
        "custom:mock-v2",
        "2026-01-05T12:00:00Z",
    );

    let (_, before_topup) = read_first_library_entry(&global_path);
    thread::sleep(Duration::from_secs(1));

    project
        .acton()
        .env(
            "HOME",
            home_temp.path().to_str().expect("home path should be utf8"),
        )
        .library()
        .arg("topup")
        .arg("global-lib")
        .arg("--wallet")
        .arg("deployer")
        .arg("--amount")
        .arg("1")
        .arg("--yes")
        .run()
        .success()
        .assert_contains("Top-up transaction sent successfully");

    let (_, after_topup) = read_first_library_entry(&global_path);
    assert_ne!(
        before_topup.last_topup_timestamp, after_topup.last_topup_timestamp,
        "topup should update timestamp in global.libraries.toml when local metadata is absent"
    );
    assert!(
        !project.path().join("libraries.toml").exists(),
        "local libraries.toml must stay absent in global fallback scenario"
    );

    mock_handle.join().expect("mock toncenter v2 must finish");
    let captured = captured
        .lock()
        .expect("captured toncenter v2 requests mutex poisoned");
    assert_eq!(captured.len(), 2, "expected seqno + sendBoc requests");
}

#[allow(clippy::significant_drop_tightening)]
#[cfg(unix)]
#[test]
fn test_library_topup_happy_path_with_duration_and_prompted_amount() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("library-topup-duration-success").build();
    write_deployer_wallets(project.path());

    let (mock_url, mock_handle, captured) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_seqno_ok_response(),
        toncenter_v2_send_boc_ok_response(),
    ]);
    append_custom_network(project.path(), "mock-v2", &mock_url);

    let libraries_path = project.path().join("libraries.toml");
    write_library_metadata_file(
        &libraries_path,
        "my-lib",
        "custom:mock-v2",
        "2026-01-05T12:00:00Z",
    );

    let (_, before_topup) = read_first_library_entry(&libraries_path);
    let mut session = project
        .acton()
        .library()
        .arg("topup")
        .arg("my-lib")
        .arg("--wallet")
        .arg("deployer")
        .arg("--duration")
        .arg("1d")
        .arg("--yes")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(30)));

    session.expect("Enter amount in GRAM");
    session.send_line("1", "failed to provide amount for duration-based topup");
    session.expect("Top-up transaction sent successfully");
    session.expect(Eof);

    let (_, after_topup) = read_first_library_entry(&libraries_path);
    assert_ne!(
        before_topup.last_topup_timestamp, after_topup.last_topup_timestamp,
        "duration-based topup should update last_topup_timestamp"
    );

    mock_handle.join().expect("mock toncenter v2 must finish");
    let captured = captured
        .lock()
        .expect("captured toncenter v2 requests mutex poisoned");
    assert_eq!(captured.len(), 2, "expected seqno + sendBoc requests");
}

#[test]
fn test_library_fetch_invalid_hash_json_reports_error_object() {
    let project = ProjectBuilder::new("library-fetch-invalid-hash-json-error").build();

    let output = project
        .acton()
        .library()
        .fetch("not-a-valid-hash")
        .with_net("testnet")
        .with_json()
        .run()
        .success();

    let payload: JsonValue = serde_json::from_str(&output.get_stdout())
        .expect("fetch --json must output JSON envelope for validation errors");
    assert_eq!(payload["success"].as_bool(), Some(false));
    assert!(
        payload["error"]
            .as_str()
            .unwrap_or_default()
            .contains("Invalid library hash format"),
        "unexpected error payload: {}",
        output.get_stdout()
    );
}

#[test]
fn test_library_fetch_invalid_network_json_reports_error_object() {
    let project = ProjectBuilder::new("library-fetch-invalid-network-json-error").build();

    let output = project
        .acton()
        .library()
        .fetch(LIB_HASH)
        .with_net("invalid")
        .with_json()
        .run()
        .success();

    let payload: JsonValue = serde_json::from_str(&output.get_stdout())
        .expect("fetch --json must output JSON envelope for network errors");
    assert_eq!(payload["success"].as_bool(), Some(false));
    assert!(
        payload["error"]
            .as_str()
            .unwrap_or_default()
            .contains("Unknown network"),
        "unexpected error payload: {}",
        output.get_stdout()
    );
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_library_publish_uses_testnet_env_api_key() {
    let project = ProjectBuilder::new("library-publish-env-api-key").build();
    write_deployer_wallets(project.path());

    let (mock_url, mock_handle, captured) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_get_libraries_not_found_response(),
        toncenter_v2_seqno_ok_response(),
        toncenter_v2_send_boc_ok_response(),
    ]);

    project
        .acton()
        .library()
        .publish()
        .with_code("te6cckEBAQEAAgAAAEysuc0=")
        .with_duration("1d")
        .wallet("deployer")
        .arg("--net")
        .arg("testnet")
        .arg("--amount")
        .arg("1")
        .arg("--yes")
        .arg("--local")
        .env(TEST_TONCENTER_TESTNET_V2_URL_ENV, &mock_url)
        .env(TONCENTER_TESTNET_API_KEY_ENV, "env-api-key")
        .run()
        .success();

    mock_handle.join().expect("mock toncenter v2 must finish");
    let captured = captured
        .lock()
        .expect("captured toncenter v2 requests mutex poisoned");
    assert!(
        !captured.is_empty(),
        "publish should produce toncenter requests in happy path"
    );
    for req in captured.iter() {
        let header = req
            .headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("x-api-key"))
            .map(|(_, value)| value.as_str());
        assert_eq!(header, Some("env-api-key"));
    }
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_library_publish_uses_mainnet_env_api_key_for_mainnet() {
    let project = ProjectBuilder::new("library-publish-mainnet-env-api-key").build();
    write_deployer_wallets(project.path());

    let (mock_url, mock_handle, captured) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_get_libraries_not_found_response(),
        toncenter_v2_seqno_ok_response(),
        toncenter_v2_send_boc_ok_response(),
    ]);

    project
        .acton()
        .library()
        .publish()
        .with_code("te6cckEBAQEAAgAAAEysuc0=")
        .with_duration("1d")
        .wallet("deployer")
        .arg("--net")
        .arg("mainnet")
        .arg("--amount")
        .arg("1")
        .arg("--yes")
        .arg("--local")
        .env(TEST_TONCENTER_MAINNET_V2_URL_ENV, &mock_url)
        .env(TONCENTER_MAINNET_API_KEY_ENV, "mainnet-api-key")
        .env(TONCENTER_TESTNET_API_KEY_ENV, "testnet-api-key")
        .run()
        .success();

    mock_handle.join().expect("mock toncenter v2 must finish");
    let captured = captured
        .lock()
        .expect("captured toncenter v2 requests mutex poisoned");
    assert!(
        !captured.is_empty(),
        "publish should produce toncenter requests in happy path"
    );
    for req in captured.iter() {
        let header = req
            .headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("x-api-key"))
            .map(|(_, value)| value.as_str());
        assert_eq!(header, Some("mainnet-api-key"));
    }
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_library_info_uses_testnet_env_api_key() {
    let project = ProjectBuilder::new("library-info-env-api-key").build();
    let libraries_path = project.path().join("libraries.toml");
    write_library_metadata_file(&libraries_path, "my-lib", "testnet", "2026-01-05T12:00:00Z");

    let (mock_url, mock_handle, captured) =
        spawn_toncenter_v2_mock(vec![toncenter_v2_balance_ok_response("1000000000")]);

    project
        .acton()
        .library()
        .arg("info")
        .arg("my-lib")
        .env(TEST_TONCENTER_TESTNET_V2_URL_ENV, &mock_url)
        .env(TONCENTER_TESTNET_API_KEY_ENV, "env-api-key")
        .run()
        .success();

    mock_handle.join().expect("mock toncenter v2 must finish");
    let captured = captured
        .lock()
        .expect("captured toncenter v2 requests mutex poisoned");
    assert_eq!(captured.len(), 1, "expected one getAddressBalance request");
    let header = captured[0]
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("x-api-key"))
        .map(|(_, value)| value.as_str());
    assert_eq!(header, Some("env-api-key"));
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_library_info_uses_mainnet_env_api_key_for_mainnet() {
    let project = ProjectBuilder::new("library-info-mainnet-env-api-key").build();
    let libraries_path = project.path().join("libraries.toml");
    write_library_metadata_file(&libraries_path, "my-lib", "mainnet", "2026-01-05T12:00:00Z");

    let (mock_url, mock_handle, captured) =
        spawn_toncenter_v2_mock(vec![toncenter_v2_balance_ok_response("1000000000")]);

    project
        .acton()
        .library()
        .arg("info")
        .arg("my-lib")
        .env(TEST_TONCENTER_MAINNET_V2_URL_ENV, &mock_url)
        .env(TONCENTER_MAINNET_API_KEY_ENV, "mainnet-api-key")
        .env(TONCENTER_TESTNET_API_KEY_ENV, "testnet-api-key")
        .run()
        .success();

    mock_handle.join().expect("mock toncenter v2 must finish");
    let captured = captured
        .lock()
        .expect("captured toncenter v2 requests mutex poisoned");
    assert_eq!(captured.len(), 1, "expected one getAddressBalance request");
    let header = captured[0]
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("x-api-key"))
        .map(|(_, value)| value.as_str());
    assert_eq!(header, Some("mainnet-api-key"));
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_library_topup_uses_testnet_env_api_key() {
    let project = ProjectBuilder::new("library-topup-env-api-key").build();
    write_deployer_wallets(project.path());
    let libraries_path = project.path().join("libraries.toml");
    write_library_metadata_file(&libraries_path, "my-lib", "testnet", "2026-01-05T12:00:00Z");

    let (mock_url, mock_handle, captured) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_seqno_ok_response(),
        toncenter_v2_send_boc_ok_response(),
    ]);

    project
        .acton()
        .library()
        .arg("topup")
        .arg("my-lib")
        .arg("--wallet")
        .arg("deployer")
        .arg("--amount")
        .arg("1")
        .arg("--yes")
        .env(TEST_TONCENTER_TESTNET_V2_URL_ENV, &mock_url)
        .env(TONCENTER_TESTNET_API_KEY_ENV, "env-api-key")
        .run()
        .success();

    mock_handle.join().expect("mock toncenter v2 must finish");
    let captured = captured
        .lock()
        .expect("captured toncenter v2 requests mutex poisoned");
    assert!(
        !captured.is_empty(),
        "topup should produce toncenter requests in happy path"
    );
    for req in captured.iter() {
        let header = req
            .headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("x-api-key"))
            .map(|(_, value)| value.as_str());
        assert_eq!(header, Some("env-api-key"));
    }
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_library_topup_uses_mainnet_env_api_key_for_mainnet() {
    let project = ProjectBuilder::new("library-topup-mainnet-env-api-key").build();
    write_deployer_wallets(project.path());
    let libraries_path = project.path().join("libraries.toml");
    write_library_metadata_file(&libraries_path, "my-lib", "mainnet", "2026-01-05T12:00:00Z");

    let (mock_url, mock_handle, captured) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_seqno_ok_response(),
        toncenter_v2_send_boc_ok_response(),
    ]);

    project
        .acton()
        .library()
        .arg("topup")
        .arg("my-lib")
        .arg("--wallet")
        .arg("deployer")
        .arg("--amount")
        .arg("1")
        .arg("--yes")
        .env(TEST_TONCENTER_MAINNET_V2_URL_ENV, &mock_url)
        .env(TONCENTER_MAINNET_API_KEY_ENV, "mainnet-api-key")
        .env(TONCENTER_TESTNET_API_KEY_ENV, "testnet-api-key")
        .run()
        .success();

    mock_handle.join().expect("mock toncenter v2 must finish");
    let captured = captured
        .lock()
        .expect("captured toncenter v2 requests mutex poisoned");
    assert!(
        !captured.is_empty(),
        "topup should produce toncenter requests in happy path"
    );
    for req in captured.iter() {
        let header = req
            .headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("x-api-key"))
            .map(|(_, value)| value.as_str());
        assert_eq!(header, Some("mainnet-api-key"));
    }
}

#[test]
fn test_library_publish_rejects_local_and_global_flags_together() {
    let project = ProjectBuilder::new("library-publish-local-global-precedence").build();

    project
        .acton()
        .library()
        .publish()
        .with_code("te6cckEBAQEAAgAAAEysuc0=")
        .with_duration("1d")
        .wallet("deployer")
        .arg("--yes")
        .arg("--local")
        .arg("--global")
        .run()
        .failure()
        .assert_stderr_contains("cannot be used with")
        .assert_stderr_contains("--local")
        .assert_stderr_contains("--global");
}

#[cfg(unix)]
#[test]
fn test_library_publish_interactive_empty_amount_exits_without_metadata_changes() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("library-publish-empty-amount-interactive").build();
    write_deployer_wallets(project.path());
    let home_temp = tempfile::TempDir::new().expect("failed to create home temp dir");

    let mut session = project
        .acton()
        .env(
            "HOME",
            home_temp.path().to_str().expect("home path should be utf8"),
        )
        .library()
        .publish()
        .with_code("te6cckEBAQEAAgAAAEysuc0=")
        .with_duration("1d")
        .wallet("deployer")
        .arg("--net")
        .arg("custom:unused")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(30)));

    session.expect("Enter amount in GRAM");
    session.send_line(
        "   ",
        "failed to submit whitespace amount input that should trim to empty",
    );
    session.expect(Eof);

    assert!(
        !project.path().join("libraries.toml").exists(),
        "libraries.toml should not be created when publish amount prompt is left empty"
    );
    let global_path = home_temp
        .path()
        .join(".config")
        .join("acton")
        .join("libraries")
        .join("global.libraries.toml");
    assert!(
        !global_path.exists(),
        "global metadata should not be created when publish exits on empty amount"
    );
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_library_publish_reports_send_boc_failure_and_skips_metadata_save() {
    let project = ProjectBuilder::new("library-publish-send-boc-failure").build();
    write_deployer_wallets(project.path());

    let (mock_url, mock_handle, captured) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_get_libraries_not_found_response(),
        toncenter_v2_seqno_ok_response(),
        toncenter_v2_send_boc_error_response("mock publish failure"),
        toncenter_v2_send_boc_error_response("mock publish failure"),
        toncenter_v2_send_boc_error_response("mock publish failure"),
    ]);
    append_custom_network(project.path(), "mock-v2", &mock_url);

    let output = project
        .acton()
        .library()
        .publish()
        .with_code("te6cckEBAQEAAgAAAEysuc0=")
        .with_duration("1d")
        .wallet("deployer")
        .arg("--net")
        .arg("custom:mock-v2")
        .arg("--amount")
        .arg("1")
        .arg("--yes")
        .arg("--local")
        .run()
        .failure();

    output
        .assert_contains("Failed to send publication transaction")
        .assert_contains("mock publish failure");
    assert!(
        !project.path().join("libraries.toml").exists(),
        "publish must not save metadata when sendBoc fails"
    );

    mock_handle.join().expect("mock toncenter v2 must finish");
    let captured = captured
        .lock()
        .expect("captured toncenter v2 requests mutex poisoned");
    assert_eq!(
        captured.len(),
        5,
        "expected getLibraries check + seqno + 3 failing sendBoc retries"
    );
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_library_publish_retries_send_boc_and_succeeds_on_third_attempt() {
    let project = ProjectBuilder::new("library-publish-send-boc-retry-success").build();
    write_deployer_wallets(project.path());

    let (mock_url, mock_handle, captured) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_get_libraries_not_found_response(),
        toncenter_v2_seqno_ok_response(),
        toncenter_v2_send_boc_error_response("transient publish failure"),
        toncenter_v2_send_boc_error_response("transient publish failure"),
        toncenter_v2_send_boc_ok_response(),
    ]);
    append_custom_network(project.path(), "mock-v2", &mock_url);

    project
        .acton()
        .library()
        .publish()
        .with_code("te6cckEBAQEAAgAAAEysuc0=")
        .with_duration("1d")
        .wallet("deployer")
        .arg("--net")
        .arg("custom:mock-v2")
        .arg("--amount")
        .arg("1")
        .arg("--yes")
        .arg("--local")
        .run()
        .success()
        .assert_contains("Library info saved");

    assert!(
        project.path().join("libraries.toml").exists(),
        "publish should save metadata after successful retry"
    );

    mock_handle.join().expect("mock toncenter v2 must finish");
    let captured = captured
        .lock()
        .expect("captured toncenter v2 requests mutex poisoned");
    assert_eq!(
        captured.len(),
        5,
        "expected getLibraries check + seqno + 2 failing sendBoc retries + final success"
    );
    assert!(captured[0].path.starts_with("/getLibraries?libraries="));
    assert_eq!(captured[1].path, "/jsonRPC");
    for request in &captured[2..] {
        assert_eq!(request.path, "/sendBoc");
    }
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_library_topup_reports_send_boc_failure_and_keeps_timestamp_unchanged() {
    let project = ProjectBuilder::new("library-topup-send-boc-failure").build();
    write_deployer_wallets(project.path());
    let libraries_path = project.path().join("libraries.toml");
    write_library_metadata_file(
        &libraries_path,
        "my-lib",
        "custom:mock-v2",
        "2026-01-05T12:00:00Z",
    );
    let (_, before_topup) = read_first_library_entry(&libraries_path);

    let (mock_url, mock_handle, captured) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_seqno_ok_response(),
        toncenter_v2_send_boc_error_response("mock topup failure"),
        toncenter_v2_send_boc_error_response("mock topup failure"),
        toncenter_v2_send_boc_error_response("mock topup failure"),
    ]);
    append_custom_network(project.path(), "mock-v2", &mock_url);

    let output = project
        .acton()
        .library()
        .arg("topup")
        .arg("my-lib")
        .arg("--wallet")
        .arg("deployer")
        .arg("--amount")
        .arg("1")
        .arg("--yes")
        .run()
        .failure();

    output
        .assert_contains("Failed to send top-up transaction")
        .assert_contains("mock topup failure");

    let (_, after_topup) = read_first_library_entry(&libraries_path);
    assert_eq!(
        before_topup.last_topup_timestamp, after_topup.last_topup_timestamp,
        "metadata timestamp must stay unchanged when sendBoc fails"
    );

    mock_handle.join().expect("mock toncenter v2 must finish");
    let captured = captured
        .lock()
        .expect("captured toncenter v2 requests mutex poisoned");
    assert_eq!(
        captured.len(),
        4,
        "expected seqno + 3 failing sendBoc retries"
    );
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_library_topup_retries_send_boc_and_succeeds_on_third_attempt() {
    let project = ProjectBuilder::new("library-topup-send-boc-retry-success").build();
    write_deployer_wallets(project.path());
    let libraries_path = project.path().join("libraries.toml");
    write_library_metadata_file(
        &libraries_path,
        "my-lib",
        "custom:mock-v2",
        "2026-01-05T12:00:00Z",
    );
    let (_, before_topup) = read_first_library_entry(&libraries_path);

    let (mock_url, mock_handle, captured) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_seqno_ok_response(),
        toncenter_v2_send_boc_error_response("transient topup failure"),
        toncenter_v2_send_boc_error_response("transient topup failure"),
        toncenter_v2_send_boc_ok_response(),
    ]);
    append_custom_network(project.path(), "mock-v2", &mock_url);

    project
        .acton()
        .library()
        .arg("topup")
        .arg("my-lib")
        .arg("--wallet")
        .arg("deployer")
        .arg("--amount")
        .arg("1")
        .arg("--yes")
        .run()
        .success()
        .assert_contains("Top-up transaction sent successfully");

    let (_, after_topup) = read_first_library_entry(&libraries_path);
    assert_ne!(
        before_topup.last_topup_timestamp, after_topup.last_topup_timestamp,
        "metadata timestamp must be updated after successful retry"
    );

    mock_handle.join().expect("mock toncenter v2 must finish");
    let captured = captured
        .lock()
        .expect("captured toncenter v2 requests mutex poisoned");
    assert_eq!(
        captured.len(),
        4,
        "expected seqno + 2 failing sendBoc retries + final success"
    );
    assert_eq!(captured[0].path, "/jsonRPC");
    for request in &captured[1..] {
        assert_eq!(request.path, "/sendBoc");
    }
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_library_fetch_transport_error_reports_failure_in_plain_mode() {
    let project = ProjectBuilder::new("library-fetch-transport-error-plain").build();
    let (mock_url, mock_handle, captured) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_get_libraries_error_response("mock fetch failure"),
        toncenter_v2_get_libraries_error_response("mock fetch failure"),
        toncenter_v2_get_libraries_error_response("mock fetch failure"),
    ]);
    append_custom_network(project.path(), "mock-v2", &mock_url);

    project
        .acton()
        .library()
        .fetch(LIB_HASH)
        .arg("--net")
        .arg("custom:mock-v2")
        .run()
        .failure()
        .assert_stderr_contains("mock fetch failure");

    mock_handle.join().expect("mock toncenter v2 must finish");
    let captured = captured
        .lock()
        .expect("captured toncenter v2 requests mutex poisoned");
    assert_eq!(
        captured.len(),
        3,
        "expected 3 getLibraries attempts with retry on server error"
    );
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_library_fetch_retries_get_libraries_and_succeeds_on_third_attempt() {
    let project = ProjectBuilder::new("library-fetch-retry-success").build();
    let (mock_url, mock_handle, captured) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_get_libraries_error_response("transient fetch failure"),
        toncenter_v2_get_libraries_error_response("transient fetch failure"),
        toncenter_v2_get_libraries_ok_response("te6cckEBAQEAAgAAAEysuc0="),
    ]);
    append_custom_network(project.path(), "mock-v2", &mock_url);

    let output = project
        .acton()
        .library()
        .fetch(LIB_HASH)
        .arg("--net")
        .arg("custom:mock-v2")
        .arg("--json")
        .run()
        .success();

    let payload: JsonValue =
        serde_json::from_str(&output.get_stdout()).expect("fetch --json must output JSON");
    assert_eq!(payload["success"].as_bool(), Some(true));
    assert!(
        payload["code_boc64"].as_str().is_some(),
        "expected code_boc64 field in fetch --json output after successful retry, got: {}",
        output.get_stdout()
    );

    mock_handle.join().expect("mock toncenter v2 must finish");
    let captured = captured
        .lock()
        .expect("captured toncenter v2 requests mutex poisoned");
    assert_eq!(
        captured.len(),
        3,
        "expected 2 failing getLibraries retries + final success"
    );
    for request in captured.iter() {
        assert!(request.path.starts_with("/getLibraries?libraries="));
    }
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_library_fetch_transport_error_reports_json_envelope_in_json_mode() {
    let project = ProjectBuilder::new("library-fetch-transport-error-json").build();
    let (mock_url, mock_handle, captured) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_get_libraries_error_response("mock fetch failure"),
        toncenter_v2_get_libraries_error_response("mock fetch failure"),
        toncenter_v2_get_libraries_error_response("mock fetch failure"),
    ]);
    append_custom_network(project.path(), "mock-v2", &mock_url);

    let output = project
        .acton()
        .library()
        .fetch(LIB_HASH)
        .arg("--net")
        .arg("custom:mock-v2")
        .arg("--json")
        .run()
        .success();

    let payload: JsonValue =
        serde_json::from_str(&output.get_stdout()).expect("fetch --json must output JSON");
    assert_eq!(payload["success"].as_bool(), Some(false));
    assert!(
        payload["error"]
            .as_str()
            .unwrap_or_default()
            .contains("mock fetch failure"),
        "unexpected payload: {}",
        output.get_stdout()
    );

    mock_handle.join().expect("mock toncenter v2 must finish");
    let captured = captured
        .lock()
        .expect("captured toncenter v2 requests mutex poisoned");
    assert_eq!(
        captured.len(),
        3,
        "expected 3 getLibraries attempts with retry on server error"
    );
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_library_info_balance_transport_failure_keeps_base_output_without_balance_fields() {
    let project = ProjectBuilder::new("library-info-balance-transport-failure").build();
    let libraries_path = project.path().join("libraries.toml");
    write_library_metadata_file(
        &libraries_path,
        "my-lib",
        "custom:mock-v2",
        "2026-01-05T12:00:00Z",
    );

    let (mock_url, mock_handle, captured) =
        spawn_toncenter_v2_mock(vec![toncenter_v2_balance_error_response(
            "mock balance failure",
        )]);
    append_custom_network(project.path(), "mock-v2", &mock_url);

    project
        .acton()
        .library()
        .arg("info")
        .arg("my-lib")
        .run()
        .success()
        .assert_contains("Library:")
        .assert_contains("Hash:")
        .assert_not_contains("Balance:")
        .assert_not_contains("Remaining:");

    mock_handle.join().expect("mock toncenter v2 must finish");
    let captured = captured
        .lock()
        .expect("captured toncenter v2 requests mutex poisoned");
    assert_eq!(captured.len(), 1, "expected one getAddressBalance request");
    assert!(
        captured[0].path.starts_with("/getAddressBalance?address="),
        "unexpected path: {}",
        captured[0].path
    );
}

#[allow(clippy::significant_drop_tightening)]
#[cfg(unix)]
#[test]
fn test_library_publish_fully_interactive_happy_path_without_flags() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("library-publish-fully-interactive-happy").build();
    let home_temp = tempfile::TempDir::new().expect("failed to create home temp dir");
    write_deployer_wallets(project.path());

    let (mock_url, mock_handle, captured) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_get_libraries_not_found_response(),
        toncenter_v2_seqno_ok_response(),
        toncenter_v2_send_boc_ok_response(),
    ]);
    append_custom_network(project.path(), "mock-v2", &mock_url);

    let mut session = project
        .acton()
        .env(
            "HOME",
            home_temp.path().to_str().expect("home path should be utf8"),
        )
        .library()
        .publish()
        .with_code("te6cckEBAQEAAgAAAEysuc0=")
        .wallet("deployer")
        .arg("--net")
        .arg("custom:mock-v2")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(40)));

    session.expect("Enter duration");
    session.send_line("1d", "failed to send duration for interactive publish");
    session.expect("Enter amount in GRAM");
    session.send_line("1", "failed to send amount for interactive publish");
    session.expect("Send 1 GRAM to publish library? Note that any extra GRAM will be refunded.");
    session.send_line("Yes", "failed to confirm interactive publish");
    session.expect("Save library info to:");
    session.send_line("", "failed to select default local storage");
    session.expect("Library info saved");
    session.expect(Eof);

    assert!(
        project.path().join("libraries.toml").exists(),
        "fully interactive publish should save metadata after successful flow"
    );

    mock_handle.join().expect("mock toncenter v2 must finish");
    let captured = captured
        .lock()
        .expect("captured toncenter v2 requests mutex poisoned");
    assert_eq!(
        captured.len(),
        3,
        "expected getLibraries check + seqno + sendBoc requests"
    );
}

#[allow(clippy::significant_drop_tightening)]
#[cfg(unix)]
#[test]
fn test_library_topup_fully_interactive_happy_path_without_duration_or_amount_flags() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("library-topup-fully-interactive-happy").build();
    write_two_wallets(project.path());
    let libraries_path = project.path().join("libraries.toml");
    write_library_metadata_file(
        &libraries_path,
        "my-lib",
        "custom:mock-v2",
        "2026-01-05T12:00:00Z",
    );
    let (_, before_topup) = read_first_library_entry(&libraries_path);

    let (mock_url, mock_handle, captured) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_seqno_ok_response(),
        toncenter_v2_send_boc_ok_response(),
    ]);
    append_custom_network(project.path(), "mock-v2", &mock_url);

    let mut session = project
        .acton()
        .library()
        .arg("topup")
        .arg("--yes")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(40)));

    session.expect("Select library to top up:");
    session.send_line("", "failed to select default library");
    session.expect("Multiple wallets configured. Please select which wallet to use:");
    session.send_line("", "failed to select default wallet");
    session.expect("Enter duration to top up for");
    session.send_line("1d", "failed to send duration for interactive topup");
    session.expect("Enter amount in GRAM");
    session.send_line("1", "failed to send amount for interactive topup");
    session.expect("Top-up transaction sent successfully");
    session.expect(Eof);

    let (_, after_topup) = read_first_library_entry(&libraries_path);
    assert_ne!(
        before_topup.last_topup_timestamp, after_topup.last_topup_timestamp,
        "fully interactive topup should update last_topup_timestamp"
    );

    mock_handle.join().expect("mock toncenter v2 must finish");
    let captured = captured
        .lock()
        .expect("captured toncenter v2 requests mutex poisoned");
    assert_eq!(captured.len(), 2, "expected seqno + sendBoc requests");
}

#[derive(Debug)]
struct StoredLibraryEntry {
    hash: String,
    account: String,
    network: String,
    last_topup_timestamp: String,
}

fn write_deployer_wallets(project_path: &Path) {
    fs::write(
        project_path.join("wallets.toml"),
        LOCALNET_DEPLOYER_WALLET_CONFIG,
    )
    .expect("failed to write wallets.toml");
}

#[cfg(unix)]
fn write_two_wallets(project_path: &Path) {
    fs::write(
        project_path.join("wallets.toml"),
        r#"[wallets.deployer]
kind = "v4r2"
workchain = 0
keys = { mnemonic = "cupboard match uphold miracle fog balance unknown region share hand trophy million toy narrow ability exchange first toast fresh maid report cram strong later" }

[wallets.secondary]
kind = "v4r2"
workchain = 0
keys = { mnemonic = "cupboard match uphold miracle fog balance unknown region share hand trophy million toy narrow ability exchange first toast fresh maid report cram strong later" }
"#,
    )
    .expect("failed to write wallets.toml with multiple wallets");
}

#[derive(Clone)]
struct ToncenterV2MockResponse {
    status: u16,
    body: String,
}

#[derive(Debug, Clone)]
struct CapturedToncenterV2Request {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
}

fn spawn_toncenter_v2_mock(
    responses: Vec<ToncenterV2MockResponse>,
) -> (
    String,
    thread::JoinHandle<()>,
    Arc<Mutex<Vec<CapturedToncenterV2Request>>>,
) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("failed to bind toncenter v2 mock");
    listener
        .set_nonblocking(true)
        .expect("failed to set toncenter v2 mock non-blocking");
    let addr = listener
        .local_addr()
        .expect("failed to get toncenter v2 mock address");

    let captured_requests = Arc::new(Mutex::new(Vec::<CapturedToncenterV2Request>::new()));
    let captured_requests_thread = Arc::clone(&captured_requests);

    let handle = thread::spawn(move || {
        for response in responses {
            let wait_until = Instant::now() + Duration::from_secs(30);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(err) if err.kind() == ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() <= wait_until,
                            "timed out waiting for toncenter v2 request"
                        );
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(err) => panic!("toncenter v2 mock accept failed: {err}"),
                }
            };

            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("failed to set toncenter v2 mock read timeout");

            let mut reader = BufReader::new(
                stream
                    .try_clone()
                    .expect("failed to clone toncenter v2 mock stream"),
            );
            let mut request_line = String::new();
            let read_deadline = Instant::now() + Duration::from_secs(2);
            loop {
                request_line.clear();
                match reader.read_line(&mut request_line) {
                    Ok(0) => {
                        assert!(
                            Instant::now() <= read_deadline,
                            "timed out waiting for toncenter v2 request line"
                        );
                        thread::sleep(Duration::from_millis(10));
                    }
                    Ok(_) => break,
                    Err(err)
                        if matches!(err.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
                    {
                        assert!(
                            Instant::now() <= read_deadline,
                            "timed out waiting for toncenter v2 request line"
                        );
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(err) => panic!("failed to read toncenter v2 request line: {err}"),
                }
            }

            let mut parts = request_line.split_whitespace();
            let method = parts.next().unwrap_or_default().to_string();
            let path = parts.next().unwrap_or_default().to_string();

            let mut headers = Vec::new();
            let mut content_length = 0_usize;
            loop {
                let mut header_line = String::new();
                let read = reader
                    .read_line(&mut header_line)
                    .expect("failed to read toncenter v2 header line");
                if read == 0 || header_line == "\r\n" {
                    break;
                }

                if let Some((name, value)) = header_line.split_once(':') {
                    let name = name.trim().to_string();
                    let value = value.trim().to_string();
                    if name.eq_ignore_ascii_case("content-length") {
                        content_length = value.parse().unwrap_or(0);
                    }
                    headers.push((name, value));
                }
            }

            if content_length > 0 {
                let mut request_body = vec![0_u8; content_length];
                reader
                    .read_exact(&mut request_body)
                    .expect("failed to read toncenter v2 request body");
            }

            captured_requests_thread
                .lock()
                .expect("captured toncenter v2 requests mutex poisoned")
                .push(CapturedToncenterV2Request {
                    method,
                    path,
                    headers,
                });

            let raw_response = format!(
                "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response.status,
                status_text(response.status),
                response.body.len(),
                response.body
            );
            stream
                .write_all(raw_response.as_bytes())
                .expect("failed to write toncenter v2 response");
            stream
                .flush()
                .expect("failed to flush toncenter v2 response");
        }
    });

    (format!("http://{addr}"), handle, captured_requests)
}

fn status_text(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        _ => "Unknown",
    }
}

fn toncenter_v2_seqno_ok_response() -> ToncenterV2MockResponse {
    ToncenterV2MockResponse {
        status: 200,
        body: serde_json::json!({
            "result": {
                "stack": [["num", "0x0"]],
                "exit_code": 0
            }
        })
        .to_string(),
    }
}

fn toncenter_v2_get_libraries_not_found_response() -> ToncenterV2MockResponse {
    ToncenterV2MockResponse {
        status: 200,
        body: serde_json::json!({
            "ok": true,
            "result": {
                "result": [{
                    "found": false
                }]
            }
        })
        .to_string(),
    }
}

fn toncenter_v2_send_boc_ok_response() -> ToncenterV2MockResponse {
    ToncenterV2MockResponse {
        status: 200,
        body: "{}".to_string(),
    }
}

fn toncenter_v2_send_boc_error_response(error: &str) -> ToncenterV2MockResponse {
    ToncenterV2MockResponse {
        status: 500,
        body: serde_json::json!({
            "ok": false,
            "error": error
        })
        .to_string(),
    }
}

fn toncenter_v2_get_libraries_error_response(error: &str) -> ToncenterV2MockResponse {
    ToncenterV2MockResponse {
        status: 500,
        body: serde_json::json!({
            "ok": false,
            "error": error
        })
        .to_string(),
    }
}

fn toncenter_v2_get_libraries_ok_response(data: &str) -> ToncenterV2MockResponse {
    ToncenterV2MockResponse {
        status: 200,
        body: serde_json::json!({
            "ok": true,
            "result": {
                "result": [{
                    "found": true,
                    "data": data
                }]
            }
        })
        .to_string(),
    }
}

fn toncenter_v2_balance_ok_response(balance: &str) -> ToncenterV2MockResponse {
    ToncenterV2MockResponse {
        status: 200,
        body: serde_json::json!({
            "ok": true,
            "result": balance
        })
        .to_string(),
    }
}

fn toncenter_v2_balance_error_response(error: &str) -> ToncenterV2MockResponse {
    ToncenterV2MockResponse {
        status: 500,
        body: serde_json::json!({
            "ok": false,
            "error": error
        })
        .to_string(),
    }
}

fn write_library_metadata_file(path: &Path, library_id: &str, network: &str, last_topup: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("failed to create library metadata parent directory");
    }

    fs::write(
        path,
        format!(
            r#"[libraries.{library_id}]
name = "MyLib"
hash = "{LIB_HASH}"
code = "te6cckEBAQEAAgAAAEysuc0="
account = "{TEST_LIBRARY_ACCOUNT}"
duration = 31536000
network = "{network}"
timestamp = "2026-01-05T12:00:00Z"
last_topup_timestamp = "{last_topup}"
bits = 1024
cells = 4
"#,
        ),
    )
    .expect("failed to write library metadata");
}

fn write_publish_library_metadata_file(
    path: &Path,
    library_id: &str,
    network: &str,
    last_topup: &str,
) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("failed to create publish library metadata directory");
    }

    fs::write(
        path,
        format!(
            r#"[libraries.{library_id}]
name = "MyLib"
hash = "{PUBLISH_TEST_CODE_HASH}"
code = "{PUBLISH_TEST_CODE_BOC64}"
account = "{TEST_LIBRARY_ACCOUNT}"
duration = 31536000
network = "{network}"
timestamp = "2026-01-05T12:00:00Z"
last_topup_timestamp = "{last_topup}"
bits = 1024
cells = 4
"#,
        ),
    )
    .expect("failed to write publish library metadata");
}

fn append_custom_network(project_path: &Path, network_name: &str, v2_url: &str) {
    use std::fmt::Write as _;

    let acton_toml_path = project_path.join("Acton.toml");
    let mut acton_toml =
        fs::read_to_string(&acton_toml_path).expect("failed to read generated Acton.toml");
    let _ = write!(
        acton_toml,
        r#"

[networks.{network_name}]
api = {{ v2 = "{v2_url}" }}
"#
    );
    fs::write(&acton_toml_path, acton_toml)
        .expect("failed to write Acton.toml with custom network");
}

fn start_localnet_with_localnet(project: &Project) -> crate::support::localnet::LocalnetHandle {
    let node = project
        .localnet()
        .before_start(super::super::support::project::ActonCommand::build)
        .args(["--accounts", "deployer"])
        .start();
    append_localnet_network(project.path(), &node.base_url());
    node
}

fn append_localnet_network(project_path: &Path, base_url: &str) {
    use std::fmt::Write as _;

    let acton_toml_path = project_path.join("Acton.toml");
    let mut acton_toml =
        fs::read_to_string(&acton_toml_path).expect("failed to read generated Acton.toml");
    let _ = write!(
        acton_toml,
        r#"

[networks.localnet]
api = {{ v2 = "{base_url}/api/v2", v3 = "{base_url}/api/v3" }}
"#
    );
    fs::write(&acton_toml_path, acton_toml).expect("failed to write Acton.toml with localnet");
}

fn read_library_entry(path: &Path, library_id: &str) -> StoredLibraryEntry {
    let content = fs::read_to_string(path).expect("failed to read libraries file");
    let doc: TomlValue = toml::from_str(&content).expect("libraries file should be valid TOML");
    let libraries = doc
        .get("libraries")
        .and_then(TomlValue::as_table)
        .expect("libraries table should be present");
    let entry = libraries
        .get(library_id)
        .and_then(TomlValue::as_table)
        .unwrap_or_else(|| panic!("library entry '{library_id}' should be present"));

    StoredLibraryEntry {
        hash: entry
            .get("hash")
            .and_then(TomlValue::as_str)
            .expect("library hash should be present")
            .to_string(),
        account: entry
            .get("account")
            .and_then(TomlValue::as_str)
            .expect("library account should be present")
            .to_string(),
        network: entry
            .get("network")
            .and_then(TomlValue::as_str)
            .expect("library network should be present")
            .to_string(),
        last_topup_timestamp: entry
            .get("last_topup_timestamp")
            .and_then(TomlValue::as_str)
            .expect("last_topup_timestamp should be present")
            .to_string(),
    }
}

fn read_first_library_entry(path: &Path) -> (String, StoredLibraryEntry) {
    let content = fs::read_to_string(path).expect("failed to read libraries file");
    let doc: TomlValue = toml::from_str(&content).expect("libraries file should be valid TOML");
    let libraries = doc
        .get("libraries")
        .and_then(TomlValue::as_table)
        .expect("libraries table should be present");
    let (library_id, entry) = libraries
        .iter()
        .next()
        .expect("expected at least one library entry");
    let entry = entry
        .as_table()
        .expect("library entry should be a table with metadata");

    let library = StoredLibraryEntry {
        hash: entry
            .get("hash")
            .and_then(TomlValue::as_str)
            .expect("library hash should be present")
            .to_string(),
        account: entry
            .get("account")
            .and_then(TomlValue::as_str)
            .expect("library account should be present")
            .to_string(),
        network: entry
            .get("network")
            .and_then(TomlValue::as_str)
            .expect("library network should be present")
            .to_string(),
        last_topup_timestamp: entry
            .get("last_topup_timestamp")
            .and_then(TomlValue::as_str)
            .expect("last_topup_timestamp should be present")
            .to_string(),
    };

    (library_id.clone(), library)
}

fn mark_library_runway_exhausted(path: &Path, library_id: &str) {
    let content = fs::read_to_string(path).expect("failed to read libraries file");
    let mut doc: TomlValue = toml::from_str(&content).expect("libraries file should be valid");
    let libraries = doc
        .get_mut("libraries")
        .and_then(TomlValue::as_table_mut)
        .expect("libraries table should be present");
    let entry = libraries
        .get_mut(library_id)
        .and_then(TomlValue::as_table_mut)
        .expect("library entry should exist");

    entry.insert(
        "last_topup_timestamp".to_string(),
        TomlValue::String("2000-01-01T00:00:00Z".to_string()),
    );
    entry.insert("bits".to_string(), TomlValue::Integer(10_000_000));
    entry.insert("cells".to_string(), TomlValue::Integer(10_000_000));

    fs::write(
        path,
        toml::to_string(&doc).expect("failed to serialize updated TOML"),
    )
    .expect("failed to write updated libraries file");
}

fn wait_for_library_in_api(
    node: &crate::support::localnet::LocalnetHandle,
    hash: &str,
    timeout: Duration,
) {
    let query = format!("/api/v2/getLibraries?libraries={hash}");
    let deadline = Instant::now() + timeout;

    loop {
        let response = node.get_json(&query);
        let found = response
            .pointer("/result/result")
            .and_then(JsonValue::as_array)
            .is_some_and(|items| {
                items.iter().any(|item| {
                    item.get("hash")
                        .and_then(JsonValue::as_str)
                        .is_some_and(|api_hash| hashes_equivalent(api_hash, hash))
                })
            });

        if found {
            return;
        }

        assert!(
            Instant::now() < deadline,
            "Timed out waiting for library {hash} in getLibraries response:\n{}",
            serde_json::to_string_pretty(&response).unwrap_or_default()
        );
        thread::sleep(Duration::from_millis(200));
    }
}

fn hashes_equivalent(left: &str, right: &str) -> bool {
    normalize_hash_to_bytes(left) == normalize_hash_to_bytes(right)
}

fn normalize_hash_to_bytes(hash: &str) -> Option<[u8; 32]> {
    let trimmed = hash.trim();

    if let Ok(bytes) = hex::decode(trimmed)
        && bytes.len() == 32
    {
        let mut out = [0_u8; 32];
        out.copy_from_slice(&bytes);
        return Some(out);
    }

    for engine in [
        &base64::engine::general_purpose::STANDARD,
        &base64::engine::general_purpose::URL_SAFE,
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
    ] {
        if let Ok(bytes) = engine.decode(trimmed)
            && bytes.len() == 32
        {
            let mut out = [0_u8; 32];
            out.copy_from_slice(&bytes);
            return Some(out);
        }
    }

    None
}

fn wait_until_address_state_active(
    node: &crate::support::localnet::LocalnetHandle,
    address: &str,
    timeout: Duration,
) {
    let query = format!("/api/v2/getAddressState?address={address}");
    let deadline = Instant::now() + timeout;

    loop {
        let response = node.get_json(&query);
        let is_active =
            response["ok"].as_bool() == Some(true) && response["result"].as_str() == Some("active");
        if is_active {
            return;
        }

        assert!(
            Instant::now() < deadline,
            "Timed out waiting for address {address} to become active:\n{}",
            serde_json::to_string_pretty(&response).unwrap_or_default()
        );
        thread::sleep(Duration::from_millis(200));
    }
}
