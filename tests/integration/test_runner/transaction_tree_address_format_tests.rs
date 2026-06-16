use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

#[test]
fn transaction_tree_short_addresses_use_url_friendly_base64() {
    ProjectBuilder::new("transaction-tree-url-friendly-address")
        .test_file(
            "transaction_tree_url_friendly_address",
            r#"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"
            import "../../lib/io"

            get fun `test transaction tree url friendly address`() {
                val sender = testing.treasury("url_friendly_sender");
                val txs = net.send(sender.address, createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest: address("0:FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"),
                }));

                println(txs);
            }
            "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/transaction_tree_address_format/transaction_tree_short_addresses_use_url_friendly_base64.stdout.txt",
        );
}
