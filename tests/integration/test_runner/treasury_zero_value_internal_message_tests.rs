use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const SNAPSHOT_DIR: &str = "integration/snapshots/test-runner/treasury_zero_value_internal_message";

const MESSAGES: &str = r"
struct (0x05138d91) OwnershipAssigned {
    queryId: uint64
    prevOwner: address
}

struct (0xA0440001) SendOwnershipAssigned {
    queryId: uint64
    recipient: address
    value: coins
}
";

const RELAY_CONTRACT: &str = r#"
import "messages"

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val msg = lazy SendOwnershipAssigned.fromSlice(in.body);
    createMessage({
        bounce: false,
        value: msg.value,
        dest: msg.recipient,
        body: OwnershipAssigned {
            queryId: msg.queryId,
            prevOwner: in.senderAddress,
        },
    }).send(SEND_MODE_PAY_FEES_SEPARATELY);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const TEST_IMPORTS: &str = r#"
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/io"
import "../../lib/testing/expect"
import "../../lib/types/big_array"
import "../contracts/messages"

fun deployOwnershipRelay(sender: Treasury): address {
    val init = ContractState {
        code: build("ownership_relay"),
        data: createEmptyCell(),
    };
    val relayAddress = AutoDeployAddress { stateInit: init }.calculateAddress();

    expect(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: init,
        },
    }))).toHaveSuccessfulDeploy({ to: relayAddress });

    return relayAddress;
}

fun sendOwnershipAssigned(
    sender: Treasury,
    relayAddress: address,
    recipient: address,
    queryId: uint64,
    value: coins,
): SendResultList {
    return net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: relayAddress,
        body: SendOwnershipAssigned {
            queryId,
            recipient,
            value,
        },
    }));
}
"#;

fn run_success(project_name: &str, test_body: &str, snapshot_name: &str) {
    ProjectBuilder::new(project_name)
        .file("contracts/messages", MESSAGES)
        .contract("ownership_relay", RELAY_CONTRACT)
        .test_file(
            "treasury_zero_value",
            &format!("{TEST_IMPORTS}\n{test_body}\n"),
        )
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(&format!("{SNAPSHOT_DIR}/{snapshot_name}.stdout.txt"));
}

#[test]
fn zero_value_pay_fees_separately_message_to_treasury_aborts_without_gas() {
    run_success(
        "ag-treasury-zero-value-pay-fees-separately",
        r#"
get fun `test zero value pay fees separately message to treasury aborts without gas`() {
    val sender = testing.treasury("sender");
    val recipient = testing.treasury("recipient");

    val relayAddress = deployOwnershipRelay(sender);
    val txs = sendOwnershipAssigned(sender, relayAddress, recipient.address, 44, 0);

    expect(txs).toHaveLength(2);
    expect(txs.findTransaction({
        from: sender.address,
        to: relayAddress,
        success: true,
    })).toBeNotNull();
    expect(txs.findTransaction<OwnershipAssigned>({
        from: relayAddress,
        to: recipient.address,
        success: false,
        computePhaseSkipped: true,
    })).toBeNotNull();

    println(txs);
    println("txCount={}", txs.size());
}
"#,
        "zero_value_pay_fees_separately_message_to_treasury_aborts_without_gas",
    );
}

#[test]
fn nonzero_value_pay_fees_separately_message_to_treasury_is_delivered() {
    run_success(
        "ag-treasury-nonzero-value-pay-fees-separately",
        r#"
get fun `test nonzero value pay fees separately message to treasury is delivered`() {
    val sender = testing.treasury("sender");
    val recipient = testing.treasury("recipient");

    val relayAddress = deployOwnershipRelay(sender);
    val txs = sendOwnershipAssigned(sender, relayAddress, recipient.address, 45, ton("0.01"));

    expect(txs).toHaveLength(2);
    expect(txs.findTransaction({
        from: sender.address,
        to: relayAddress,
        success: true,
    })).toBeNotNull();
    expect(txs.findTransaction<OwnershipAssigned>({
        from: relayAddress,
        to: recipient.address,
        success: true,
        computePhaseSkipped: false,
    })).toBeNotNull();

    println(txs);
    println("txCount={}", txs.size());
}
"#,
        "nonzero_value_pay_fees_separately_message_to_treasury_is_delivered",
    );
}

#[test]
fn one_nanogram_pay_fees_separately_message_to_treasury_aborts_without_enough_gas() {
    run_success(
        "ag-treasury-one-nanogram-pay-fees-separately",
        r#"
get fun `test one nanogram pay fees separately message to treasury aborts without enough gas`() {
    val sender = testing.treasury("sender");
    val recipient = testing.treasury("recipient");

    val relayAddress = deployOwnershipRelay(sender);
    val txs = sendOwnershipAssigned(sender, relayAddress, recipient.address, 46, 1);

    expect(txs).toHaveLength(2);
    expect(txs.findTransaction({
        from: sender.address,
        to: relayAddress,
        success: true,
    })).toBeNotNull();
    expect(txs.findTransaction<OwnershipAssigned>({
        from: relayAddress,
        to: recipient.address,
        success: false,
        computePhaseSkipped: true,
    })).toBeNotNull();

    println(txs);
    println("txCount={}", txs.size());
}
"#,
        "one_nanogram_pay_fees_separately_message_to_treasury_aborts_without_enough_gas",
    );
}
