use crate::common::assertion;
use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use crate::support::snapshots::normalize_output;
use std::time::Duration;
use std::{fs, thread};

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";
const COUNTER_CONTRACT: &str = include_str!("../acton-stdlib/contracts/counter.tolk");
const COUNTER_TYPES: &str = include_str!("../acton-stdlib/contracts/types.tolk");

#[test]
fn test_disasm_from_boc_file() {
    let project = ProjectBuilder::new("disasm-file")
        .contract_with_output("simple", SIMPLE_CONTRACT, "simple.boc")
        .build();

    project.acton().build().run().success();

    project
        .acton()
        .disasm_file("simple.boc")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/test_disasm_from_boc_file.stdout.txt");
}

#[test]
fn test_disasm_reads_source_map_emitted_by_compile() {
    let project = ProjectBuilder::new("disasm-source-map")
        .raw_file("contracts/counter.tolk", COUNTER_CONTRACT)
        .raw_file("contracts/types.tolk", COUNTER_TYPES)
        .build();

    project
        .acton()
        .compile("contracts/counter.tolk")
        .with_boc_output("counter.boc")
        .with_source_map("counter.source_map.json")
        .run()
        .success();

    project
        .acton()
        .disasm_file("counter.boc")
        .with_source_map("counter.source_map.json")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_disasm_reads_source_map_emitted_by_compile.stdout.txt",
        );
}

#[test]
fn test_disasm_from_boc_file_with_output() {
    let project = ProjectBuilder::new("disasm-output")
        .contract_with_output("simple", SIMPLE_CONTRACT, "simple.boc")
        .build();

    project.acton().build().run().success();

    project
        .acton()
        .disasm_file("simple.boc")
        .with_output("output.tasm")
        .run()
        .success()
        .assert_contains("Disassembled code written to output.tasm");

    let output_file = project.path().join("output.tasm");
    assert!(output_file.exists(), "Output file should exist");
}

#[test]
fn test_disasm_from_boc_file_with_base64() {
    let project = ProjectBuilder::new("disasm-file")
        .raw_file("simple.base64", "te6ccgEBBAEAbwABFP8A9KQT9LzyyAsBAgFiAgMAmtD4kZEw4CDXLCP0Oyd8jhgx7UTQAdcLHwHWH9cLH1igAcjOyx/J7VTg1ywh06l4NDGOEjDtRNDWHzDIzs+QAAAAAsntVOCEDwHHAPL0ABehlaHaiaGmPmOuFj8=")
        .build();

    project.acton().build().run().success();

    project
        .acton()
        .disasm_file("simple.base64")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_disasm_from_boc_file_with_base64.stdout.txt",
        );
}

#[test]
fn test_disasm_from_boc_file_with_hex() {
    let project = ProjectBuilder::new("disasm-file")
        .raw_file("simple.hex", "b5ee9c7201010401006f000114ff00f4a413f4bcf2c80b0102016203020017a195a1da89a1a63e63ae163f009ad0f8919130e020d72c23f43b277c8e1831ed44d001d70b1f01d61fd70b1f58a001c8cecb1fc9ed54e0d72c21d3a97834318e1230ed44d0d61f30c8cecf9000000002c9ed54e0840f01c700f2f4")
        .build();

    project.acton().build().run().success();

    project
        .acton()
        .disasm_file("simple.hex")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_disasm_from_boc_file_with_hex.stdout.txt",
        );
}

#[test]
fn test_disasm_from_boc_file_with_hex_with_newlines() {
    let project = ProjectBuilder::new("disasm-file")
        .raw_file("simple.hex", "\n\nb5ee9c7201010401006f000114ff00f4a413f4bcf2c80b0102016203020017a195a1da89a1a63e63ae163f009ad0f8919130e020d72c23f43b277c8e1831ed44d001d70b1f01d61fd70b1f58a001c8cecb1fc9ed54e0d72c21d3a97834318e1230ed44d0d61f30c8cecf9000000002c9ed54e0840f01c700f2f4\n\n")
        .build();

    project.acton().build().run().success();

    project
        .acton()
        .disasm_file("simple.hex")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_disasm_from_boc_file_with_hex_with_newlines.stdout.txt",
        );
}

#[test]
fn test_disasm_from_boc_file_with_invalid_hex() {
    let project = ProjectBuilder::new("disasm-file")
        .raw_file("simple.hex", "123\n\nb5ee9c7201010401006f000114ff00f4a413f4bcf2c80b0102016203020017a195a1da89a1a63e63ae163f009ad0f8919130e020d72c23f43b277c8e1831ed44d001d70b1f01d61fd70b1f58a001c8cecb1fc9ed54e0d72c21d3a97834318e1230ed44d0d61f30c8cecf9000000002c9ed54e0840f01c700f2f4\n\n")
        .build();

    project.acton().build().run().success();

    project
        .acton()
        .disasm_file("simple.hex")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_disasm_from_boc_file_with_invalid_hex.stderr.txt",
        );
}

#[test]
fn test_disasm_from_hex_string() {
    let project = ProjectBuilder::new("disasm-hex")
        .contract_with_output("simple", SIMPLE_CONTRACT, "simple.boc")
        .build();

    project.acton().build().run().success();

    let boc_bytes = fs::read(project.path().join("simple.boc")).unwrap();
    let hex_string = hex::encode(boc_bytes);

    project
        .acton()
        .disasm()
        .disasm_string(&hex_string)
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/test_disasm_from_hex_string.stdout.txt");
}

#[test]
fn test_disasm_from_base64_string() {
    let project = ProjectBuilder::new("disasm-base64")
        .contract_with_output("simple", SIMPLE_CONTRACT, "simple.boc")
        .build();

    project.acton().build().run().success();

    let boc_bytes = fs::read(project.path().join("simple.boc")).unwrap();
    let base64_string =
        tycho_types::boc::Boc::encode_base64(tycho_types::boc::Boc::decode(boc_bytes).unwrap());

    project
        .acton()
        .disasm()
        .disasm_string(&base64_string)
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/test_disasm_from_base64_string.stdout.txt");
}

#[test]
fn test_disasm_file_not_found() {
    let project = ProjectBuilder::new("disasm-not-found").build();

    project
        .acton()
        .disasm_file("nonexistent.boc")
        .run()
        .failure()
        .assert_stderr_contains("Cannot find file or director");
}

#[test]
fn test_disasm_invalid_boc_data() {
    let project = ProjectBuilder::new("disasm-invalid").build();

    fs::create_dir_all(project.path().join("data")).unwrap();
    fs::write(project.path().join("data/invalid.boc"), "invalid boc data").unwrap();

    project
        .acton()
        .disasm_file("data/invalid.boc")
        .run()
        .failure()
        .assert_stderr_contains("Failed to decode BoC");
}

#[test]
fn test_disasm_invalid_hex_string() {
    let project = ProjectBuilder::new("disasm-invalid-hex").build();

    project
        .acton()
        .disasm()
        .disasm_string("not_valid_hex_or_base64")
        .run()
        .failure()
        .assert_stderr_contains("Failed to decode BoC");
}

#[test]
fn test_disasm_no_input_provided() {
    let project = ProjectBuilder::new("disasm-no-input").build();

    project
        .acton()
        .disasm()
        .run()
        .failure()
        .assert_stderr_contains(" Either --string, -s, --address or BOC_FILE argument must be provided, run with --help for more information");
}

#[test]
fn test_disasm_built_contract() {
    let complex_contract = r"
    fun onInternalMessage(in: InMessage) {}
    fun onBouncedMessage(_: InMessageBounced) {}
    ";

    let project = ProjectBuilder::new("disasm-complex")
        .contract_with_output("complex", complex_contract, "complex.boc")
        .build();

    project.acton().build().run().success();

    project
        .acton()
        .disasm_file("complex.boc")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_disasm_with_complex_contract.stdout.txt",
        );
}

#[test]
fn test_disasm_snapshot() {
    let project = ProjectBuilder::new("disasm-snapshot")
        .contract_with_output("simple", SIMPLE_CONTRACT, "simple.boc")
        .build();

    project.acton().build().run().success();

    project
        .acton()
        .disasm_file("simple.boc")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/test_disasm_snapshot.stdout.txt");
}

#[test]
fn test_disasm_output_file_created() {
    let project = ProjectBuilder::new("disasm-create")
        .contract_with_output("simple", SIMPLE_CONTRACT, "simple.boc")
        .build();

    project.acton().build().run().success();

    let output_file = project.path().join("result.tasm");
    assert!(!output_file.exists());

    project
        .acton()
        .disasm_file("simple.boc")
        .with_output("result.tasm")
        .run()
        .success();

    assert!(output_file.exists());

    let content = fs::read_to_string(&output_file).unwrap();
    assertion().eq(
        normalize_output(&content, project.path()),
        snapbox::file!("snapshots/test_disasm_output_file_created.tasm.gen"),
    );
}

#[test]
fn test_disasm_overwrite_existing_file() {
    let project = ProjectBuilder::new("disasm-overwrite")
        .contract_with_output("simple", SIMPLE_CONTRACT, "simple.boc")
        .build();

    project.acton().build().run().success();

    let output_file = project.path().join("output.tasm");
    fs::write(&output_file, "old content").unwrap();

    project
        .acton()
        .disasm_file("simple.boc")
        .with_output("output.tasm")
        .run()
        .success();

    let content = fs::read_to_string(&output_file).unwrap();
    assert_ne!(content, "old content");
    assertion().eq(
        normalize_output(&content, project.path()),
        snapbox::file!("snapshots/test_disasm_overwrite_existing_file.tasm.gen"),
    );
}

// We don't usually want to store keys this way, but without keys it's almost
// impossible to use API calls :(
fn toncenter_api_key() -> &'static str {
    option_env!("TONCENTER_API_KEY")
        .unwrap_or("49efa980ccdcd018fd09d387e63537afd9db4dbb8509d69e7bc2303ca2b2c860")
}

#[test]
#[cfg(feature = "only_ci")]
fn test_disasm_from_blockchain_mainnet_address() {
    thread::sleep(Duration::from_secs(1)); // rate limit
    let project = ProjectBuilder::new("disasm-blockchain-mainnet").build();

    project
        .acton()
        .disasm()
        .with_address("UQA_ftKIJsHEAE_UgtFOUK15hPzycZooFuUr8duyY9T3kwwM")
        .with_api_key(toncenter_api_key())
        .run()
        .success()
        .assert_contains("PUSHINT")
        .assert_contains("POP");
}

#[test]
#[cfg(feature = "only_ci")]
fn test_disasm_from_blockchain_testnet_address() {
    thread::sleep(Duration::from_secs(1)); // rate limit
    let project = ProjectBuilder::new("disasm-blockchain-testnet").build();

    project
        .acton()
        .disasm()
        .with_address("kQAlDMBKCT8WJ4nwdwNRp0lvKMP4vUnHYspFPhEnyR36cg44")
        .with_api_key(toncenter_api_key())
        .run()
        .success()
        .assert_contains("PUSHINT")
        .assert_contains("POP");
}

#[test]
#[cfg(feature = "only_ci")]
fn test_disasm_from_blockchain_mainnet_address_with_exotic_cell_lib() {
    thread::sleep(Duration::from_secs(1)); // rate limit
    let project = ProjectBuilder::new("disasm-blockchain-testnet").build();

    project
        .acton()
        .disasm()
        .with_address("EQC2jeGorIAFh2LXwsDjHfRK-GSo9UzchdIEMh24A7T7AHot")
        .with_api_key(toncenter_api_key())
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/test_disasm_from_blockchain_mainnet_address_with_exotic_cell_lib.stdout.txt");
}

#[test]
#[cfg(feature = "only_ci")]
fn test_disasm_follow_libraries() {
    thread::sleep(Duration::from_secs(1)); // rate limit
    let project = ProjectBuilder::new("disasm-follow-libraries").build();

    project
        .acton()
        .disasm()
        .with_address("EQC2jeGorIAFh2LXwsDjHfRK-GSo9UzchdIEMh24A7T7AHot")
        .with_api_key(toncenter_api_key())
        .with_net("mainnet")
        .follow_libraries()
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/test_disasm_follow_libraries.stdout.txt");
}

#[test]
fn test_disasm_from_blockchain_invalid_address() {
    thread::sleep(Duration::from_secs(1)); // rate limit
    let project = ProjectBuilder::new("disasm-blockchain-invalid").build();

    project
        .acton()
        .disasm()
        .with_address("invalid-address")
        .with_api_key(toncenter_api_key())
        .run()
        .failure()
        .assert_stderr_contains("Address invalid-address is not a valid address.");
}

#[test]
#[cfg(feature = "only_ci")]
fn test_disasm_from_blockchain_with_wrong_api_key() {
    thread::sleep(Duration::from_secs(1)); // rate limit
    let project = ProjectBuilder::new("disasm-blockchain-api-key").build();

    project
        .acton()
        .disasm()
        .with_address("UQA_ftKIJsHEAE_UgtFOUK15hPzycZooFuUr8duyY9T3kwwM")
        .with_api_key("wrong-test-api-key")
        .run()
        .failure()
        .assert_contains("Contract with address UQA_ftKIJsHEAE_UgtFOUK15hPzycZooFuUr8duyY9T3kwwM not found on both mainnet and testnet");
}

#[test]
fn test_disasm_directory_as_file() {
    let project = ProjectBuilder::new("disasm-directory")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .disasm_file("contracts") // contracts is a directory
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_disasm_directory_as_file.stderr.txt",
        );
}

#[test]
fn test_disasm_invalid_network() {
    let project = ProjectBuilder::new("disasm-invalid-net")
        .contract_with_output("simple", SIMPLE_CONTRACT, "simple.boc")
        .build();

    project.acton().build().run().success();

    project
        .acton()
        .disasm_file("simple.boc")
        .with_net("invalid-network")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_disasm_invalid_network.stderr.txt",
        );
}

#[test]
fn test_disasm_multiple_input_sources_file_and_string() {
    let project = ProjectBuilder::new("disasm-multiple-inputs")
        .contract_with_output("simple", SIMPLE_CONTRACT, "simple.boc")
        .build();

    project.acton().build().run().success();

    project
        .acton()
        .disasm_file("simple.boc")
        .disasm_string("some_hex_data")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_disasm_multiple_input_sources_file_and_string.stderr.txt",
        );
}

#[test]
fn test_disasm_address_with_invalid_network() {
    let project = ProjectBuilder::new("disasm-addr-invalid-net").build();

    project
        .acton()
        .disasm()
        .with_address("UQA_ftKIJsHEAE_UgtFOUK15hPzycZooFuUr8duyY9T3kwwM")
        .with_net("invalid-network")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_disasm_address_with_invalid_network.stderr.txt",
        );
}

#[test]
fn test_disasm_empty_address() {
    let project = ProjectBuilder::new("disasm-empty-addr").build();

    project
        .acton()
        .disasm()
        .with_address("")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_disasm_empty_address.stderr.txt",
        );
}

#[test]
fn test_disasm_empty_string() {
    let project = ProjectBuilder::new("disasm-empty-string").build();

    project
        .acton()
        .disasm()
        .disasm_string("")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_disasm_empty_string.stderr.txt",
        );
}

#[test]
fn test_disasm_file_without_read_permission() {
    let project = ProjectBuilder::new("disasm-no-read")
        .raw_file("secret.boc", "some boc data")
        .build();

    // Make the file unreadable (on Unix systems)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let file_path = project.path().join("secret.boc");
        let mut perms = fs::metadata(&file_path).unwrap().permissions();
        perms.set_mode(0o000); // no permissions
        fs::set_permissions(&file_path, perms).unwrap();
    }

    project
        .acton()
        .disasm_file("secret.boc")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_disasm_file_without_read_permission.stderr.txt",
        );
}

#[test]
fn test_disasm_empty_file_path() {
    let project = ProjectBuilder::new("disasm-empty-path").build();

    project
        .acton()
        .disasm_file("")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_disasm_empty_file_path.stderr.txt",
        );
}
