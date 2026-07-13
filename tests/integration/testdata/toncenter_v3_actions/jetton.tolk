import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/scripts"
import "../../lib/io"
import "../types/jetton_v1_master.types"
import "../types/jetton_v1_wallet.types"
import "../wrappers/JettonV1Master.gen"
import "../wrappers/JettonV1Wallet.gen"

const RECIPIENT = "0:1111111111111111111111111111111111111111111111111111111111111111"

fun inlineEmptyForwardPayload(): RemainingBitsAndRefs {
    return beginCell().storeUint(0, 1).endCell().beginParse();
}

fun waitForTransaction(result: SendResultList) {
    if (result.waitForFirstTransaction() == null) {
        throw 1000;
    }
}

fun main() {
    val owner = scripts.wallet("deployer");
    val recipient = address(RECIPIENT);

    val master = JettonV1Master.fromStorage({
        totalSupply: 0,
        adminAddress: owner.address,
        jettonContent: createEmptyCell(),
        jettonWalletCode: build("jettonv1wallet"),
    });
    waitForTransaction(master.deploy(owner.address, { value: ton("1"), bounce: false }));

    val mintBody = JettonInternalTransfer {
        queryId: 100,
        amount: 1000,
        from: owner.address,
        responseAddress: owner.address,
        forwardTonAmount: 0,
        forwardPayload: inlineEmptyForwardPayload(),
    }.toCell();
    waitForTransaction(master.sendJettonV1Mint(
        owner.address,
        100,
        owner.address,
        ton("0.2"),
        mintBody,
        { value: ton("0.5"), bounce: false },
    ));

    val sourceWallet = JettonV1Wallet.fromAddress(master.getWalletAddress(owner.address));
    val recipientWalletAddress = master.getWalletAddress(recipient);
    waitForTransaction(sourceWallet.sendJettonTransfer(
        owner.address,
        101,
        100,
        recipient,
        owner.address,
        null,
        ton("0.01"),
        inlineEmptyForwardPayload(),
        { value: ton("0.3"), bounce: false },
    ));
    waitForTransaction(sourceWallet.sendJettonBurn(
        owner.address,
        102,
        50,
        owner.address,
        null,
        { value: ton("0.2"), bounce: false },
    ));
    waitForTransaction(sourceWallet.sendJettonTransfer(
        owner.address,
        103,
        10000,
        recipient,
        owner.address,
        null,
        ton("0.01"),
        inlineEmptyForwardPayload(),
        { value: ton("0.3"), bounce: false },
    ));
    waitForTransaction(sourceWallet.sendJettonBurn(
        owner.address,
        104,
        10000,
        owner.address,
        null,
        { value: ton("0.2"), bounce: false },
    ));

    println("OWNER={}", owner.address);
    println("RECIPIENT={}", recipient);
    println("JETTON_MASTER={}", master.address);
    println("JETTON_SOURCE_WALLET={}", sourceWallet.address);
    println("JETTON_RECIPIENT_WALLET={}", recipientWalletAddress);
}
