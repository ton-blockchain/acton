use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const NETWORK_IMPORTS: &str = r#"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/testing/transaction_expect"
"#;

fn run_wallet_success(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{NETWORK_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("wallet_fallback", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn wallet_matches_treasury_address_without_broadcast() {
    run_wallet_success(
        "bf-stdlib-wallet-fallback-matches-treasury-address",
        r#"
get fun `test-bf-wallet-fallback-matches-treasury-address`() {
    expect(net.isBroadcasting()).toEqual(false);

    val wallet = net.wallet("bf_fallback_owner");
    val treasury = net.treasury("bf_fallback_owner");

    expect(wallet.address).toEqual(treasury.address);
}
"#,
        "integration/snapshots/test-runner/wallet_matches_treasury_address_without_broadcast/wallet_matches_treasury_address_without_broadcast.stdout.txt",
    );
}

#[test]
fn wallet_unknown_name_uses_treasury_and_sends_in_non_broadcast_mode() {
    run_wallet_success(
        "bf-stdlib-wallet-fallback-unknown-name-non-broadcast",
        r#"
get fun `test-bf-wallet-fallback-unknown-name-non-broadcast`() {
    expect(net.isBroadcasting()).toEqual(false);

    val wallet = net.wallet("bf_wallet_missing_from_wallets_config");
    val treasury = net.treasury("bf_wallet_missing_from_wallets_config");
    val receiver = net.treasury("bf_wallet_receiver");

    expect(wallet.address).toEqual(treasury.address);

    val transfer = createMessage({
        bounce: false,
        value: ton("0.2"),
        dest: receiver.address,
    });
    val txs = net.send(wallet.address, transfer);
    expect(txs).toHaveSuccessfulTx({
        from: treasury.address,
        to: receiver.address,
    });
}
"#,
        "integration/snapshots/test-runner/wallet_matches_treasury_address_without_broadcast/wallet_unknown_name_uses_treasury_and_sends_in_non_broadcast_mode.stdout.txt",
    );
}
