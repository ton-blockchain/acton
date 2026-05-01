use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use crate::support::toncenter::{
    ToncenterV2MockResponse, append_custom_network, spawn_toncenter_v2_mock,
    toncenter_v2_error_response,
};
use base64::Engine;
use std::fs;

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

const RAW_ADDRESS_MAINNET: &str = "EQBvDB_H7FFBs0nF4ap_DBdcOrwY_rMIpNVVOR6SWYFHByMJ";
const RAW_ADDRESS_TESTNET: &str = "kQBvDB_H7FFBs0nF4ap_DBdcOrwY_rMIpNVVOR6SWYFHB5iD";

#[test]
fn fork_net_mainnet_formats_addresses_as_mainnet() {
    let project = ProjectBuilder::new("i-fork-mainnet-address")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "address",
            r#"
            import "../../lib/io"

            get fun `test mainnet format`() {
                println(address("EQBvDB_H7FFBs0nF4ap_DBdcOrwY_rMIpNVVOR6SWYFHByMJ"));
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .fork_net("mainnet")
        .run()
        .success()
        .assert_passed(1)
        .assert_contains(RAW_ADDRESS_MAINNET)
        .assert_not_contains(RAW_ADDRESS_TESTNET)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_fork_network/fork_net_mainnet_formats_addresses_as_mainnet.stdout.txt",
        );
}

#[test]
fn fork_net_testnet_formats_addresses_as_testnet() {
    let project = ProjectBuilder::new("i-fork-testnet-address")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "address",
            r#"
            import "../../lib/io"

            get fun `test testnet format`() {
                println(address("EQBvDB_H7FFBs0nF4ap_DBdcOrwY_rMIpNVVOR6SWYFHByMJ"));
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .fork_net("testnet")
        .run()
        .success()
        .assert_passed(1)
        .assert_contains(RAW_ADDRESS_TESTNET)
        .assert_not_contains(RAW_ADDRESS_MAINNET)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_fork_network/fork_net_testnet_formats_addresses_as_testnet.stdout.txt",
        );
}

#[test]
fn fork_net_mainnet_formats_failure_addresses_as_mainnet() {
    let project = ProjectBuilder::new("i-fork-mainnet-failure-address")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "address_failure",
            r#"
            import "../../lib/io"
            import "../../lib/testing/expect"

            get fun `test mainnet failure format`() {
                expect(address("EQC2jeGorIAFh2LXwsDjHfRK-GSo9UzchdIEMh24A7T7AHot"))
                    .toEqual(address("EQD__________________________________________0vo"));
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .fork_net("mainnet")
        .run()
        .code(1)
        .assert_failed(1)
        .assert_contains("EQC2jeGorIAFh2LXwsDjHfRK-GSo9UzchdIEMh24A7T7AHot")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_fork_network/fork_net_mainnet_formats_failure_addresses_as_mainnet.stdout.txt",
        );
}

#[test]
fn forked_remote_account_preserves_last_transaction_metadata() {
    let last_hash_bytes = [0x11_u8; 32];
    let last_hash_b64 = base64::engine::general_purpose::STANDARD.encode(last_hash_bytes);
    let last_hash_hex = hex::encode(last_hash_bytes);
    let last_lt = 424_242_u64;
    let (mock_url, mock_handle) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_error_response(404, "getShardAccountCell is unavailable"),
        toncenter_v2_account_info_ok_response(1000, "uninitialized", last_lt, &last_hash_b64),
    ]);

    let source = format!(
        r#"
            import "../../lib/io"
            import "../../lib/emulation/testing"
            import "../../lib/testing/expect"

            get fun `test remote account last transaction metadata`() {{
                val shard = testing.getShardAccount(address("{RAW_ADDRESS_MAINNET}"));
                expect(shard).toBeNotNull();

                if (shard != null) {{
                    expect(shard!.lastTransLt).toEqual({last_lt});
                    expect(shard!.lastTransHash).toEqual(0x{last_hash_hex});
                }}
            }}
        "#
    );
    let project = ProjectBuilder::new("i-fork-remote-last-transaction")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("remote_metadata", &source)
        .build();
    append_custom_network(project.path(), "remote-meta", &format!("{mock_url}/api/v2"));

    project
        .acton()
        .test()
        .fork_net("custom:remote-meta")
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_fork_network/forked_remote_account_preserves_last_transaction_metadata.stdout.txt",
        );

    mock_handle.join().expect("mock toncenter must finish");
}

#[test]
fn rejects_non_numeric_fork_block_number() {
    let project = ProjectBuilder::new("i-invalid-fork-block")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "smoke",
            r#"
            import "../../lib/testing/expect"

            get fun `test smoke`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .arg("--fork-block-number")
        .arg("not-a-seqno")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_fork_network/rejects_non_numeric_fork_block_number.stderr.txt",
        );
}

#[test]
fn accepts_fork_block_without_remote_access() {
    let project = ProjectBuilder::new("i-api-key-no-fork")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "smoke",
            r#"
            import "../../lib/testing/expect"

            get fun `test local smoke`() {
                expect(2 + 2).toEqual(4);
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .arg("--fork-block-number")
        .arg("42")
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_fork_network/accepts_api_key_and_fork_block_without_remote_access.stdout.txt",
        );
}

#[test]
fn unknown_fork_network_should_fail_fast() {
    let project = ProjectBuilder::new("i-unknown-fork-network")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "smoke",
            r#"
            import "../../lib/testing/expect"

            get fun `test smoke`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .arg("--fork-net")
        .arg("definitely-invalid-network")
        .run()
        .failure()
        .assert_stderr_contains("Unknown network");
}

#[test]
fn unknown_custom_fork_network_should_fail_before_running_tests() {
    let project = ProjectBuilder::new("i-unknown-custom-fork-network")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "smoke",
            r#"
            import "../../lib/testing/expect"

            get fun `test smoke`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .arg("--fork-net")
        .arg("custom:missing-network")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_fork_network/unknown_custom_fork_network_should_fail_before_running_tests.stderr.txt",
        );
}

#[test]
fn custom_fork_network_without_v2_url_should_fail_before_running_tests() {
    let project = ProjectBuilder::new("i-custom-fork-network-missing-v2")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "smoke",
            r#"
            import "../../lib/testing/expect"

            get fun `test smoke`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .build();

    let acton_toml_path = project.path().join("Acton.toml");
    let mut acton_toml =
        fs::read_to_string(&acton_toml_path).expect("failed to read generated Acton.toml");
    acton_toml.push_str(
        r#"

[networks.broken]
explorer = "https://example.invalid"
"#,
    );
    fs::write(&acton_toml_path, acton_toml)
        .expect("failed to write malformed custom network config");

    project
        .acton()
        .test()
        .arg("--fork-net")
        .arg("custom:broken")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_fork_network/custom_fork_network_without_v2_url_should_fail_before_running_tests.stderr.txt",
        );
}

fn toncenter_v2_account_info_ok_response(
    balance: i64,
    state: &str,
    lt: u64,
    hash: &str,
) -> ToncenterV2MockResponse {
    ToncenterV2MockResponse {
        status: 200,
        body: serde_json::json!({
            "result": {
                "balance": balance.to_string(),
                "code": "",
                "data": "",
                "state": state,
                "frozen_hash": "",
                "last_transaction_id": {
                    "lt": lt.to_string(),
                    "hash": hash,
                }
            }
        })
        .to_string(),
    }
}
