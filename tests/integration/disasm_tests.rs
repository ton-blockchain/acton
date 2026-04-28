use crate::common::assertion;
use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use crate::support::snapshots::normalize_output;
use crate::support::toncenter::{
    CapturedToncenterRequest, append_custom_network_with_urls, spawn_toncenter_mock_with_capture,
    spawn_toncenter_v3_mock, toncenter_v2_error_response, toncenter_v2_get_libraries_ok_response,
    toncenter_v3_account_states_ok_response, toncenter_v3_error_response,
};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;
use std::{fs, thread};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder};

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";
const COUNTER_CONTRACT: &str = include_str!("../acton-stdlib/contracts/counter.tolk");
const COUNTER_TYPES: &str = include_str!("../acton-stdlib/contracts/types.tolk");
const TEST_API_KEY: &str = "test-toncenter-api-key";
const TEST_TONCENTER_MAINNET_V3_URL_ENV: &str = "ACTON_TEST_TONCENTER_MAINNET_V3_URL";
const TEST_TONCENTER_TESTNET_V3_URL_ENV: &str = "ACTON_TEST_TONCENTER_TESTNET_V3_URL";

static REMOTE_MOCK_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn build_simple_contract_project(name: &str) -> crate::support::project::Project {
    let project = ProjectBuilder::new(name)
        .contract_with_output("simple", SIMPLE_CONTRACT, "simple.boc")
        .build();

    project.acton().build().run().success();
    project
}

fn build_counter_source_map_project(name: &str) -> crate::support::project::Project {
    let project = ProjectBuilder::new(name)
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
}

fn simple_contract_cell(project: &crate::support::project::Project) -> Cell {
    let boc_bytes = fs::read(project.path().join("simple.boc")).unwrap();
    Boc::decode(boc_bytes).unwrap()
}

fn simple_contract_boc_base64(project: &crate::support::project::Project) -> String {
    Boc::encode_base64(simple_contract_cell(project))
}

fn simple_contract_library_reference_boc_base64(
    project: &crate::support::project::Project,
) -> String {
    let cell = simple_contract_cell(project);
    let library_ref = CellBuilder::build_library(cell.repr_hash());
    Boc::encode_base64(&library_ref)
}

fn header_value<'a>(request: &'a CapturedToncenterRequest, name: &str) -> Option<&'a str> {
    request
        .headers
        .iter()
        .find(|(header, _)| header.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

fn assert_api_key_header(request: &CapturedToncenterRequest) {
    assert_eq!(
        header_value(request, "X-API-Key"),
        Some(TEST_API_KEY),
        "expected TonCenter request to carry X-API-Key header"
    );
}

fn remote_mock_guard() -> std::sync::MutexGuard<'static, ()> {
    REMOTE_MOCK_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[test]
fn test_disasm_from_boc_file() {
    let project = build_simple_contract_project("disasm-file");

    project
        .acton()
        .disasm_file("simple.boc")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/disasm/test_disasm_from_boc_file.stdout.txt",
        );
}

#[test]
fn test_disasm_from_boc_file_with_show_hashes() {
    let project = build_simple_contract_project("disasm-file-show-hashes");

    project
        .acton()
        .disasm_file("simple.boc")
        .show_hashes()
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/disasm/test_disasm_from_boc_file_with_show_hashes.stdout.txt",
        );
}

#[test]
fn test_disasm_from_boc_file_with_show_offsets() {
    let project = build_simple_contract_project("disasm-file-show-offsets");

    project
        .acton()
        .disasm_file("simple.boc")
        .show_offsets()
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/disasm/test_disasm_from_boc_file_with_show_offsets.stdout.txt",
        );
}

#[test]
fn test_disasm_from_boc_file_with_show_hashes_and_offsets() {
    let project = build_simple_contract_project("disasm-file-show-hashes-offsets");

    project
        .acton()
        .disasm_file("simple.boc")
        .show_hashes()
        .show_offsets()
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/disasm/test_disasm_from_boc_file_with_show_hashes_and_offsets.stdout.txt",
        );
}

#[test]
fn test_disasm_reads_source_map_emitted_by_compile() {
    let project = build_counter_source_map_project("disasm-source-map");

    project
        .acton()
        .disasm_file("counter.boc")
        .with_source_map("counter.source_map.json")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/disasm/test_disasm_reads_source_map_emitted_by_compile.stdout.txt",
        );
}

#[test]
fn test_disasm_reads_source_map_emitted_by_compile_with_show_hashes() {
    let project = build_counter_source_map_project("disasm-source-map-show-hashes");

    project
        .acton()
        .disasm_file("counter.boc")
        .with_source_map("counter.source_map.json")
        .show_hashes()
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/disasm/test_disasm_reads_source_map_emitted_by_compile_with_show_hashes.stdout.txt",
        );
}

#[test]
fn test_disasm_reads_source_map_emitted_by_compile_with_show_offsets() {
    let project = build_counter_source_map_project("disasm-source-map-show-offsets");

    project
        .acton()
        .disasm_file("counter.boc")
        .with_source_map("counter.source_map.json")
        .show_offsets()
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/disasm/test_disasm_reads_source_map_emitted_by_compile_with_show_offsets.stdout.txt",
        );
}

#[test]
fn test_disasm_reads_source_map_emitted_by_compile_with_show_hashes_and_offsets() {
    let project = build_counter_source_map_project("disasm-source-map-show-hashes-offsets");

    project
        .acton()
        .disasm_file("counter.boc")
        .with_source_map("counter.source_map.json")
        .show_hashes()
        .show_offsets()
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/disasm/test_disasm_reads_source_map_emitted_by_compile_with_show_hashes_and_offsets.stdout.txt",
        );
}

#[test]
fn test_disasm_json_includes_source_map_blocks() {
    let project = build_counter_source_map_project("disasm-source-map-json");

    project
        .acton()
        .disasm_file("counter.boc")
        .with_source_map("counter.source_map.json")
        .with_json()
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/disasm/test_disasm_json_includes_source_map_blocks.stdout.txt",
        );
}

#[test]
fn test_disasm_json_with_output_writes_assembly_file() {
    let project = build_simple_contract_project("disasm-json-output");

    let output = project
        .acton()
        .disasm_file("simple.boc")
        .with_output("output.tasm")
        .with_json()
        .run()
        .success();

    output.assert_snapshot_matches(
        "integration/snapshots/disasm/test_disasm_json_with_output_writes_assembly_file.stdout.txt",
    );
    output.assert_file_snapshot_matches(
        "output.tasm",
        "integration/snapshots/disasm/test_disasm_json_with_output_writes_assembly_file.tasm.gen",
    );
}

#[test]
fn test_disasm_missing_source_map_file() {
    let project = build_simple_contract_project("disasm-source-map-missing");

    project
        .acton()
        .disasm_file("simple.boc")
        .with_source_map("missing.source_map.json")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/disasm/test_disasm_missing_source_map_file.stderr.txt",
        );
}

#[test]
fn test_disasm_source_map_path_is_directory() {
    let project = build_simple_contract_project("disasm-source-map-dir");
    fs::create_dir(project.path().join("maps")).unwrap();

    project
        .acton()
        .disasm_file("simple.boc")
        .with_source_map("maps")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/disasm/test_disasm_source_map_path_is_directory.stderr.txt",
        );
}

#[test]
fn test_disasm_invalid_source_map_json() {
    let project = build_simple_contract_project("disasm-source-map-invalid-json");
    fs::write(project.path().join("invalid.source_map.json"), "{").unwrap();

    project
        .acton()
        .disasm_file("simple.boc")
        .with_source_map("invalid.source_map.json")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/disasm/test_disasm_invalid_source_map_json.stderr.txt",
        );
}

#[test]
fn test_disasm_from_boc_file_with_output() {
    let project = build_simple_contract_project("disasm-output");

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
fn test_disasm_output_creates_nested_directories() {
    let project = build_simple_contract_project("disasm-output-nested");

    project
        .acton()
        .disasm_file("simple.boc")
        .with_output("nested/dir/result.tasm")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/disasm/test_disasm_output_creates_nested_directories.stdout.txt",
        );

    let output_file = project.path().join("nested/dir/result.tasm");
    assert!(output_file.exists(), "Nested output file should exist");

    let content = fs::read_to_string(&output_file).unwrap();
    assertion().eq(
        normalize_output(&content, project.path()),
        snapbox::file!("snapshots/disasm/test_disasm_output_creates_nested_directories.tasm.gen"),
    );
}

#[test]
fn test_disasm_output_parent_directory_creation_error() {
    let project = build_simple_contract_project("disasm-output-dir-error");
    fs::write(project.path().join("blocked"), "not a directory").unwrap();

    project
        .acton()
        .disasm_file("simple.boc")
        .with_output("blocked/result.tasm")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/disasm/test_disasm_output_parent_directory_creation_error.stderr.txt",
        );
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
            "integration/snapshots/disasm/test_disasm_from_boc_file_with_base64.stdout.txt",
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
            "integration/snapshots/disasm/test_disasm_from_boc_file_with_hex.stdout.txt",
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
            "integration/snapshots/disasm/test_disasm_from_boc_file_with_hex_with_newlines.stdout.txt",
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
            "integration/snapshots/disasm/test_disasm_from_boc_file_with_invalid_hex.stderr.txt",
        );
}

#[test]
fn test_disasm_from_hex_string() {
    let project = build_simple_contract_project("disasm-hex");

    let boc_bytes = fs::read(project.path().join("simple.boc")).unwrap();
    let hex_string = hex::encode(boc_bytes);

    project
        .acton()
        .disasm()
        .disasm_string(&hex_string)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/disasm/test_disasm_from_hex_string.stdout.txt",
        );
}

#[test]
fn test_disasm_from_base64_string() {
    let project = build_simple_contract_project("disasm-base64");

    let boc_bytes = fs::read(project.path().join("simple.boc")).unwrap();
    let base64_string = Boc::encode_base64(Boc::decode(boc_bytes).unwrap());

    project
        .acton()
        .disasm()
        .disasm_string(&base64_string)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/disasm/test_disasm_from_base64_string.stdout.txt",
        );
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
            "integration/snapshots/disasm/test_disasm_with_complex_contract.stdout.txt",
        );
}

#[test]
fn test_disasm_snapshot() {
    let project = build_simple_contract_project("disasm-snapshot");

    project
        .acton()
        .disasm_file("simple.boc")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/disasm/test_disasm_snapshot.stdout.txt");
}

#[test]
fn test_disasm_output_file_created() {
    let project = build_simple_contract_project("disasm-create");

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
        snapbox::file!("snapshots/disasm/test_disasm_output_file_created.tasm.gen"),
    );
}

#[test]
fn test_disasm_overwrite_existing_file() {
    let project = build_simple_contract_project("disasm-overwrite");

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
        snapbox::file!("snapshots/disasm/test_disasm_overwrite_existing_file.tasm.gen"),
    );
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_disasm_from_blockchain_custom_network_address_with_mock_toncenter() {
    let _guard = remote_mock_guard();
    let project = build_simple_contract_project("disasm-blockchain-custom-mock");
    let address = "UQA_ftKIJsHEAE_UgtFOUK15hPzycZooFuUr8duyY9T3kwwM";
    let code_boc = simple_contract_boc_base64(&project);

    let (mock_url, mock_handle, captured) =
        spawn_toncenter_v3_mock(vec![toncenter_v3_account_states_ok_response(
            address,
            Some(&code_boc),
            "active",
        )]);
    append_custom_network_with_urls(project.path(), "mock-remote", &mock_url, &mock_url);

    let output = project
        .acton()
        .disasm()
        .with_address(address)
        .with_net("custom:mock-remote")
        .with_api_key(TEST_API_KEY)
        .run()
        .success();

    output.assert_snapshot_matches(
        "integration/snapshots/disasm/test_disasm_from_blockchain_mock_contract.stdout.txt",
    );

    mock_handle.join().expect("mock toncenter must finish");
    let captured = captured
        .lock()
        .expect("captured toncenter requests mutex poisoned");
    assert_eq!(captured.len(), 1, "expected one accountStates request");
    assert_eq!(captured[0].method, "GET");
    assert!(
        captured[0].path.starts_with("/accountStates?address="),
        "unexpected TonCenter path: {}",
        captured[0].path
    );
    assert_api_key_header(&captured[0]);
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_disasm_from_blockchain_custom_network_fetch_failure_with_mock_toncenter() {
    let _guard = remote_mock_guard();
    let project = build_simple_contract_project("disasm-blockchain-custom-mock-failure");
    let address = "UQA_ftKIJsHEAE_UgtFOUK15hPzycZooFuUr8duyY9T3kwwM";

    let (mock_url, mock_handle, captured) =
        spawn_toncenter_v3_mock(vec![toncenter_v3_error_response(
            404,
            "mock accountStates failure",
        )]);
    append_custom_network_with_urls(project.path(), "mock-remote", &mock_url, &mock_url);

    let output = project
        .acton()
        .disasm()
        .with_address(address)
        .with_net("custom:mock-remote")
        .with_api_key(TEST_API_KEY)
        .run()
        .failure();

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/disasm/test_disasm_from_blockchain_custom_network_fetch_failure.stderr.txt",
    );

    mock_handle.join().expect("mock toncenter must finish");
    let captured = captured
        .lock()
        .expect("captured toncenter requests mutex poisoned");
    assert_eq!(
        captured.len(),
        1,
        "expected one failing accountStates request"
    );
    assert_api_key_header(&captured[0]);
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_disasm_from_blockchain_mainnet_address_with_mock_autodetect() {
    let _guard = remote_mock_guard();
    let project = build_simple_contract_project("disasm-blockchain-mainnet-mock");
    let address = "UQA_ftKIJsHEAE_UgtFOUK15hPzycZooFuUr8duyY9T3kwwM";
    let code_boc = simple_contract_boc_base64(&project);

    let (mainnet_url, mainnet_handle, mainnet_captured) =
        spawn_toncenter_v3_mock(vec![toncenter_v3_account_states_ok_response(
            address,
            Some(&code_boc),
            "active",
        )]);

    let output = project
        .acton()
        .disasm()
        .with_address(address)
        .with_api_key(TEST_API_KEY)
        .env(TEST_TONCENTER_MAINNET_V3_URL_ENV, &mainnet_url)
        .env(TEST_TONCENTER_TESTNET_V3_URL_ENV, "http://127.0.0.1:1")
        .run()
        .success();

    output.assert_snapshot_matches(
        "integration/snapshots/disasm/test_disasm_from_blockchain_mock_contract.stdout.txt",
    );

    mainnet_handle
        .join()
        .expect("mainnet mock toncenter must finish");
    let mainnet_captured = mainnet_captured
        .lock()
        .expect("captured toncenter requests mutex poisoned");
    assert_eq!(mainnet_captured.len(), 1, "expected one mainnet request");
    assert_api_key_header(&mainnet_captured[0]);
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_disasm_from_blockchain_testnet_address_with_mock_autodetect() {
    let _guard = remote_mock_guard();
    let project = build_simple_contract_project("disasm-blockchain-testnet-mock");
    let address = "kQAlDMBKCT8WJ4nwdwNRp0lvKMP4vUnHYspFPhEnyR36cg44";
    let code_boc = simple_contract_boc_base64(&project);

    let (mainnet_url, mainnet_handle, mainnet_captured) =
        spawn_toncenter_v3_mock(vec![toncenter_v3_error_response(404, "mock mainnet miss")]);
    let (testnet_url, testnet_handle, testnet_captured) =
        spawn_toncenter_v3_mock(vec![toncenter_v3_account_states_ok_response(
            address,
            Some(&code_boc),
            "active",
        )]);

    let output = project
        .acton()
        .disasm()
        .with_address(address)
        .with_api_key(TEST_API_KEY)
        .env(TEST_TONCENTER_MAINNET_V3_URL_ENV, &mainnet_url)
        .env(TEST_TONCENTER_TESTNET_V3_URL_ENV, &testnet_url)
        .run()
        .success();

    output.assert_snapshot_matches(
        "integration/snapshots/disasm/test_disasm_from_blockchain_mock_contract.stdout.txt",
    );

    mainnet_handle
        .join()
        .expect("mainnet mock toncenter must finish");
    testnet_handle
        .join()
        .expect("testnet mock toncenter must finish");

    let mainnet_captured = mainnet_captured
        .lock()
        .expect("captured toncenter requests mutex poisoned");
    let testnet_captured = testnet_captured
        .lock()
        .expect("captured toncenter requests mutex poisoned");
    assert_eq!(mainnet_captured.len(), 1, "expected one mainnet request");
    assert_eq!(testnet_captured.len(), 1, "expected one testnet request");
    assert_api_key_header(&mainnet_captured[0]);
    assert_api_key_header(&testnet_captured[0]);
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_disasm_from_blockchain_address_not_found_on_both_networks_with_mock_autodetect() {
    let _guard = remote_mock_guard();
    let project = build_simple_contract_project("disasm-blockchain-both-networks-miss-mock");
    let address = "UQA_ftKIJsHEAE_UgtFOUK15hPzycZooFuUr8duyY9T3kwwM";

    let (mainnet_url, mainnet_handle, mainnet_captured) =
        spawn_toncenter_v3_mock(vec![toncenter_v3_error_response(404, "mock mainnet miss")]);
    let (testnet_url, testnet_handle, testnet_captured) =
        spawn_toncenter_v3_mock(vec![toncenter_v3_error_response(404, "mock testnet miss")]);

    let output = project
        .acton()
        .disasm()
        .with_address(address)
        .with_api_key(TEST_API_KEY)
        .env(TEST_TONCENTER_MAINNET_V3_URL_ENV, &mainnet_url)
        .env(TEST_TONCENTER_TESTNET_V3_URL_ENV, &testnet_url)
        .run()
        .failure();

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/disasm/test_disasm_from_blockchain_address_not_found_on_both_networks_with_mock.stderr.txt",
    );

    mainnet_handle
        .join()
        .expect("mainnet mock toncenter must finish");
    testnet_handle
        .join()
        .expect("testnet mock toncenter must finish");

    let mainnet_captured = mainnet_captured
        .lock()
        .expect("captured toncenter requests mutex poisoned");
    let testnet_captured = testnet_captured
        .lock()
        .expect("captured toncenter requests mutex poisoned");
    assert_eq!(mainnet_captured.len(), 1, "expected one mainnet request");
    assert_eq!(testnet_captured.len(), 1, "expected one testnet request");
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_disasm_follow_libraries_with_mock_toncenter() {
    let _guard = remote_mock_guard();
    let project = build_simple_contract_project("disasm-follow-libraries-mock");
    let address = "EQC2jeGorIAFh2LXwsDjHfRK-GSo9UzchdIEMh24A7T7AHot";
    let contract_code_boc = simple_contract_library_reference_boc_base64(&project);
    let library_boc = simple_contract_boc_base64(&project);

    let account_response =
        toncenter_v3_account_states_ok_response(address, Some(&contract_code_boc), "active");
    let library_response = toncenter_v2_get_libraries_ok_response(&library_boc);
    let (mock_url, mock_handle, captured) = spawn_toncenter_mock_with_capture(vec![
        (account_response.status, account_response.body),
        (library_response.status, library_response.body),
    ]);
    append_custom_network_with_urls(project.path(), "mock-remote", &mock_url, &mock_url);

    let output = project
        .acton()
        .disasm()
        .with_address(address)
        .with_net("custom:mock-remote")
        .with_api_key(TEST_API_KEY)
        .follow_libraries()
        .run()
        .success();

    output.assert_snapshot_matches(
        "integration/snapshots/disasm/test_disasm_from_blockchain_mock_contract.stdout.txt",
    );

    mock_handle.join().expect("mock toncenter must finish");
    let captured = captured
        .lock()
        .expect("captured toncenter requests mutex poisoned");
    assert_eq!(
        captured.len(),
        2,
        "expected accountStates fetch followed by getLibraries lookup"
    );
    assert!(captured[0].path.starts_with("/accountStates?address="));
    assert!(captured[1].path.starts_with("/getLibraries?libraries="));
    assert_api_key_header(&captured[0]);
    assert_api_key_header(&captured[1]);
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_disasm_follow_libraries_warns_and_keeps_original_code_with_mock_toncenter() {
    let _guard = remote_mock_guard();
    let project = build_simple_contract_project("disasm-follow-libraries-mock-warning");
    let address = "EQC2jeGorIAFh2LXwsDjHfRK-GSo9UzchdIEMh24A7T7AHot";
    let contract_code_boc = simple_contract_library_reference_boc_base64(&project);

    let account_response =
        toncenter_v3_account_states_ok_response(address, Some(&contract_code_boc), "active");
    let library_response = toncenter_v2_error_response(404, "mock library lookup failure");
    let (mock_url, mock_handle, captured) = spawn_toncenter_mock_with_capture(vec![
        (account_response.status, account_response.body),
        (library_response.status, library_response.body),
    ]);
    append_custom_network_with_urls(project.path(), "mock-remote", &mock_url, &mock_url);

    let output = project
        .acton()
        .disasm()
        .with_address(address)
        .with_net("custom:mock-remote")
        .with_api_key(TEST_API_KEY)
        .follow_libraries()
        .run()
        .success();

    output.assert_snapshot_matches(
        "integration/snapshots/disasm/test_disasm_follow_libraries_warns_and_keeps_original_code.stdout.txt",
    );
    output.assert_stderr_snapshot_matches(
        "integration/snapshots/disasm/test_disasm_follow_libraries_warns_and_keeps_original_code.stderr.txt",
    );

    mock_handle.join().expect("mock toncenter must finish");
    let captured = captured
        .lock()
        .expect("captured toncenter requests mutex poisoned");
    assert_eq!(
        captured.len(),
        2,
        "expected accountStates fetch followed by failing getLibraries lookup"
    );
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_disasm_follow_libraries_skips_lookup_for_non_library_code_with_mock_toncenter() {
    let _guard = remote_mock_guard();
    let project = build_simple_contract_project("disasm-follow-libraries-mock-no-library");
    let address = "UQA_ftKIJsHEAE_UgtFOUK15hPzycZooFuUr8duyY9T3kwwM";
    let code_boc = simple_contract_boc_base64(&project);

    let (mock_url, mock_handle, captured) =
        spawn_toncenter_v3_mock(vec![toncenter_v3_account_states_ok_response(
            address,
            Some(&code_boc),
            "active",
        )]);
    append_custom_network_with_urls(project.path(), "mock-remote", &mock_url, &mock_url);

    let output = project
        .acton()
        .disasm()
        .with_address(address)
        .with_net("custom:mock-remote")
        .with_api_key(TEST_API_KEY)
        .follow_libraries()
        .run()
        .success();

    output.assert_snapshot_matches(
        "integration/snapshots/disasm/test_disasm_from_blockchain_mock_contract.stdout.txt",
    );

    mock_handle.join().expect("mock toncenter must finish");
    let captured = captured
        .lock()
        .expect("captured toncenter requests mutex poisoned");
    assert_eq!(
        captured.len(),
        1,
        "expected no getLibraries lookup for non-library code"
    );
    assert!(captured[0].path.starts_with("/accountStates?address="));
}

// We don't usually want to store keys this way, but without keys it's almost
// impossible to use API calls :(
fn toncenter_api_key() -> &'static str {
    option_env!("TONCENTER_TESTNET_API_KEY")
        .or(option_env!("TONCENTER_MAINNET_API_KEY"))
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
        .assert_snapshot_matches("integration/snapshots/disasm/test_disasm_from_blockchain_mainnet_address_with_exotic_cell_lib.stdout.txt");
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
        .assert_snapshot_matches(
            "integration/snapshots/disasm/test_disasm_follow_libraries.stdout.txt",
        );
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
            "integration/snapshots/disasm/test_disasm_directory_as_file.stderr.txt",
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
            "integration/snapshots/disasm/test_disasm_invalid_network.stderr.txt",
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
            "integration/snapshots/disasm/test_disasm_multiple_input_sources_file_and_string.stderr.txt",
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
            "integration/snapshots/disasm/test_disasm_address_with_invalid_network.stderr.txt",
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
            "integration/snapshots/disasm/test_disasm_empty_address.stderr.txt",
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
            "integration/snapshots/disasm/test_disasm_empty_string.stderr.txt",
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
            "integration/snapshots/disasm/test_disasm_file_without_read_permission.stderr.txt",
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
            "integration/snapshots/disasm/test_disasm_empty_file_path.stderr.txt",
        );
}
