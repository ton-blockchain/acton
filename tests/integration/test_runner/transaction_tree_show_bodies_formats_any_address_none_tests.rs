use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const ANY_ADDRESS_MESSAGES: &str = r"
struct (0xA6000001) RouteWithMaybeDest {
    queryId: uint64
    maybeDest: any_address
}
";

const ANY_ADDRESS_RECEIVER: &str = r#"
import "messages"

contract Receiver {
    incomingMessages: RouteWithMaybeDest
}

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val _msg = lazy RouteWithMaybeDest.fromSlice(in.body);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

#[test]
fn transaction_tree_show_bodies_formats_any_address_none_in_message_body() {
    ProjectBuilder::new("ex-stdlib-show-bodies-any-address-none")
        .file("contracts/messages", ANY_ADDRESS_MESSAGES)
        .contract("receiver", ANY_ADDRESS_RECEIVER)
        .test_file(
            "show_bodies_any_address_none",
            r#"
            import "../../lib/build"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"
            import "../../lib/testing/expect"
            import "../contracts/messages"

            get fun `test ex show bodies any address none`() {
                val deployer = testing.treasury("deployer");
                val init = ContractState {
                    code: build("receiver"),
                    data: createEmptyCell(),
                };

                val txs = net.send(deployer.address, createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest: {
                        stateInit: init,
                    },
                    body: RouteWithMaybeDest {
                        queryId: 1,
                        maybeDest: createAddressNone(),
                    },
                }));

                expect(txs).toHaveTx({
                    to: address("0:00000000000000000000000000000000000000000000000000000000000000AA"),
                });
            }
            "#,
        )
        .build()
        .acton()
        .test()
        .show_bodies()
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/transaction_tree_show_bodies_formats_any_address_none/transaction_tree_show_bodies_formats_any_address_none_in_message_body.stdout.txt",
        );
}
