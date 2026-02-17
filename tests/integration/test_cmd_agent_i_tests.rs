use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

const RAW_ADDRESS_MAINNET: &str = "EQBvDB/H7FFBs0nF4ap/DBdcOrwY/rMIpNVVOR6SWYFHByMJ";
const RAW_ADDRESS_TESTNET: &str = "kQBvDB/H7FFBs0nF4ap/DBdcOrwY/rMIpNVVOR6SWYFHB5iD";

#[test]
fn i_test_cmd_fork_net_mainnet_formats_addresses_as_mainnet() {
    let project = ProjectBuilder::new("i-fork-mainnet-address")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "address",
            r#"
            import "../../lib/io"

            get fun `test-mainnet-format`() {
                println(address("EQBvDB/H7FFBs0nF4ap/DBdcOrwY/rMIpNVVOR6SWYFHByMJ"));
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
            "integration/snapshots/test_cmd_agent_i/i_test_cmd_fork_net_mainnet_formats_addresses_as_mainnet.stdout.txt",
        );
}

#[test]
fn i_test_cmd_fork_net_testnet_formats_addresses_as_testnet() {
    let project = ProjectBuilder::new("i-fork-testnet-address")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "address",
            r#"
            import "../../lib/io"

            get fun `test-testnet-format`() {
                println(address("EQBvDB/H7FFBs0nF4ap/DBdcOrwY/rMIpNVVOR6SWYFHByMJ"));
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
            "integration/snapshots/test_cmd_agent_i/i_test_cmd_fork_net_testnet_formats_addresses_as_testnet.stdout.txt",
        );
}

#[test]
fn i_test_cmd_rejects_non_numeric_fork_block_number() {
    let project = ProjectBuilder::new("i-invalid-fork-block")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "smoke",
            r#"
            import "../../lib/testing/expect"

            get fun `test-smoke`() {
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
            "integration/snapshots/test_cmd_agent_i/i_test_cmd_rejects_non_numeric_fork_block_number.stderr.txt",
        );
}

#[test]
fn i_test_cmd_accepts_api_key_and_fork_block_without_remote_access() {
    let project = ProjectBuilder::new("i-api-key-no-fork")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "smoke",
            r#"
            import "../../lib/testing/expect"

            get fun `test-local-smoke`() {
                expect(2 + 2).toEqual(4);
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .arg("--api-key")
        .arg("local-test-api-key")
        .arg("--fork-block-number")
        .arg("42")
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test_cmd_agent_i/i_test_cmd_accepts_api_key_and_fork_block_without_remote_access.stdout.txt",
        );
}

#[test]
fn i_test_cmd_unknown_fork_network_should_fail_fast() {
    let project = ProjectBuilder::new("i-unknown-fork-network")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "smoke",
            r#"
            import "../../lib/testing/expect"

            get fun `test-smoke`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .build();

    // BUG: unknown --fork-net values are silently ignored in `acton test`; expected CLI parse error.
    project
        .acton()
        .test()
        .arg("--fork-net")
        .arg("definitely-invalid-network")
        .run()
        .failure()
        .assert_stderr_contains("Unknown network");
}
