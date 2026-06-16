use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const SIGNATURE_CONTRACT: &str = r"
const ERR_INVALID_SIGNATURE = 77;
const PUBLIC_KEY = 0x1234;
const SIGNATURE_BITS = 512;

struct Storage {
    accepted: uint32
}

struct (0x71000001) SignedPingPayload {
    value: uint32
}

struct SignedPing {
    payload: SignedPingPayload
    signature: bits512
}

fun onInternalMessage(in: InMessage) {
    var bodySlice = in.body;
    if (bodySlice.remainingBitsCount() < SIGNATURE_BITS) {
        return;
    }

    val signature = bodySlice.getLastBits(SIGNATURE_BITS);
    val signedSlice = bodySlice.removeLastBits(SIGNATURE_BITS);
    val payload = SignedPingPayload.fromSlice(signedSlice);

    assert (isSignatureValid(signedSlice.hash(), signature, PUBLIC_KEY)) throw ERR_INVALID_SIGNATURE;

    contract.setData(Storage { accepted: payload.value }.toCell());
}

fun onBouncedMessage(_: InMessageBounced) {}

get fun accepted(): uint32 {
    return Storage.fromCell(contract.getData()).accepted;
}
";

const TEST_SOURCE: &str = r#"
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/testing/expect"
import "../../lib/types/message"

struct Storage {
    accepted: uint32
}

struct (0x71000001) SignedPingPayload {
    value: uint32
}

struct SignedPing {
    payload: SignedPingPayload
    signature: bits512
}

struct SignatureHarness {
    address: address
    init: ContractState
}

fun invalidSignature(): bits512 {
    return beginCell()
        .storeUint(0, 256)
        .storeUint(0, 256)
        .endCell()
        .beginParse() as bits512;
}

fun signedPingMessage(dest: address, value: uint32): OutMessage {
    return createMessage({
        bounce: BounceMode.NoBounce,
        value: ton("0.1"),
        dest,
        body: SignedPing {
            payload: SignedPingPayload { value },
            signature: invalidSignature(),
        },
    });
}

fun deployHarness(): SignatureHarness {
    val init = ContractState {
        code: build("signature"),
        data: Storage { accepted: 0 }.toCell(),
    };
    val address = AutoDeployAddress { stateInit: init }.calculateAddress();
    val deployer = testing.treasury("deployer");

    val deployTxs = net.send(
        deployer.address,
        createMessage({
            bounce: false,
            value: ton("1"),
            dest: { stateInit: init },
        }),
    );
    expect(deployTxs).toHaveSuccessfulDeploy({ to: address });

    return SignatureHarness { address, init };
}

get fun `test checksig ignore controls transaction signature checks`() {
    val harness = deployHarness();
    val sender = testing.treasury("sender");

    val rejectedBefore = net.send(sender.address, signedPingMessage(harness.address, 7));
    expect(rejectedBefore).toHaveTx({
        to: harness.address,
        exitCode: fun(code: int32): bool {
            return code != 0;
        },
    });

    testing.setChecksigIgnore(true);

    val accepted = net.send(sender.address, signedPingMessage(harness.address, 7));
    expect(accepted).toHaveSuccessfulTx({ to: harness.address });
    expect(net.runGetMethod<uint32>(harness.address, "accepted")).toEqual(7);

    testing.setChecksigIgnore(false);

    val rejectedAfter = net.send(sender.address, signedPingMessage(harness.address, 9));
    expect(rejectedAfter).toHaveTx({
        to: harness.address,
        exitCode: fun(code: int32): bool {
            return code != 0;
        },
    });
    expect(net.runGetMethod<uint32>(harness.address, "accepted")).toEqual(7);
}
"#;

#[test]
fn testing_set_checksig_ignore_controls_emulated_signature_checks() {
    ProjectBuilder::new("testing-set-checksig-ignore")
        .contract("signature", SIGNATURE_CONTRACT)
        .test_file("signature", TEST_SOURCE)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/testing_set_checksig_ignore_controls_emulated_signature_checks/testing_set_checksig_ignore_controls_emulated_signature_checks.stdout.txt",
        );
}
