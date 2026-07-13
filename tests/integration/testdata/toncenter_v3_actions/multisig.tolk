import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/scripts"
import "../../lib/io"
import "../types/multisig_order_v2.types"
import "../types/multisig_v2.types"
import "../wrappers/MultisigOrderV2.gen"
import "../wrappers/MultisigV2.gen"

const COSIGNER = "0:2222222222222222222222222222222222222222222222222222222222222222"

fun waitForTransaction(result: SendResultList) {
    if (result.waitForFirstTransaction() == null) {
        throw 1000;
    }
}

fun main() {
    val owner = scripts.wallet("deployer");
    val cosigner = address(COSIGNER);

    val publisherState = ContractState {
        code: build("librarypublisher"),
        data: build("multisigorderv2library"),
    };
    waitForTransaction(net.send(owner.address, createMessage({
        bounce: false,
        value: ton("0.2"),
        dest: { stateInit: publisherState },
    })));

    var signers: map<uint8, address> = [];
    signers.set(0, owner.address);
    signers.set(1, cosigner);

    val multisig = MultisigV2.fromStorage({
        nextOrderSeqno: 0,
        threshold: 2,
        signers: signers.toLowLevelDict()!,
        signersNum: 2,
        proposers: [],
        allowArbitraryOrderSeqno: false,
    });
    waitForTransaction(multisig.deploy(owner.address, { value: ton("1"), bounce: false }));

    var actions: map<uint8, cell> = [];
    actions.set(0, MultisigV2UpdateMultisigParamAction {
        threshold: 2,
        signers: signers.toLowLevelDict()!,
        proposers: [],
    }.toCell());
    actions.set(1, MultisigV2SendMessageAction {
        mode: 3,
        message: createMessage({
            bounce: false,
            value: ton("0.01"),
            dest: cosigner,
        }).messageCell,
    }.toCell());
    actions.set(2, MultisigV2SendMessageAction {
        mode: 64,
        message: createExternalLogMessage({
            dest: createAddressNone(),
        }).messageCell,
    }.toCell());
    actions.set(3, beginCell().storeUint(0xdeadbeef, 32).endCell());
    waitForTransaction(multisig.sendMultisigV2NewOrder(
        owner.address,
        300,
        0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff,
        true,
        0,
        (blockchain.now() + 3600) as uint48,
        actions.toLowLevelDict()!,
        { value: ton("1"), bounce: false },
    ));

    val order = MultisigOrderV2.fromAddress(multisig.getOrderAddress(0));
    println("OWNER={}", owner.address);
    println("COSIGNER={}", cosigner);
    println("MULTISIG={}", multisig.address);
    println("MULTISIG_ORDER={}", order.address);
}
