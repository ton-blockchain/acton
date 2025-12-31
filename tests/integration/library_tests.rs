use crate::common::assertion;
use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use crate::support::snapshots::normalize_output;
use std::time::Duration;
use std::{fs, thread};

const LIB_HASH: &str = "b993c68c596425f05d1bc492d7c03e2979ab669901ed5a57e35e6dd4d6089d27";

// We don't usually want to store keys this way, but without keys it's almost
// impossible to use API calls :(
fn toncenter_api_key() -> &'static str {
    option_env!("TONCENTER_API_KEY")
        .unwrap_or("49efa980ccdcd018fd09d387e63537afd9db4dbb8509d69e7bc2303ca2b2c860")
}

#[test]
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
        .assert_snapshot_matches("integration/snapshots/test_library_fetch_basic.stdio.txt");
}

#[test]
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
        .assert_snapshot_matches("integration/snapshots/test_library_fetch_json.stdout.json.txt");
}

#[test]
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
            "integration/snapshots/test_library_fetch_fail_json.stdout.json.txt",
        );
}

#[test]
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
            "integration/snapshots/test_library_fetch_unknown.stderr.txt",
        );
}

#[test]
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
            "integration/snapshots/test_library_fetch_unknown.stdout.json.txt",
        );
}

#[test]
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
        .assert_snapshot_matches("integration/snapshots/test_library_fetch_basic_disasm.stdio.txt");
}

#[test]
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
        snapbox::file!("snapshots/test_library_fetch_basic.lib.txt"),
    );
}

#[test]
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
        snapbox::file!("snapshots/test_library_fetch_basic.lib.tasm.txt"),
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
            "integration/snapshots/test_library_publish_invalid_network.stderr.txt",
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
            "integration/snapshots/test_library_publish_invalid_code.stderr.txt",
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
            "integration/snapshots/test_library_publish_contract_not_found.stderr.txt",
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
            "integration/snapshots/test_library_publish_compilation_error.stderr.txt",
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
            "integration/snapshots/test_library_publish_invalid_duration.stderr.txt",
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
name = "Simple"
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
            "integration/snapshots/test_library_publish_wallet_not_found.stderr.txt",
        );
}
