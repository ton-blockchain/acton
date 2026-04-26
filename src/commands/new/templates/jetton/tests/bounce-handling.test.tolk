import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"
import "@acton/types/message"
import "@contracts/messages"
import "@wrappers/JettonMinter.gen"
import "@wrappers/JettonWallet.gen"
import "@wrappers/utils"
import "test-utils"

get fun `test minter restores supply on internal_transfer bounce`() {
    val (minter, deployer, _) = setupTest();
    val supplyBefore = minter.getTotalSupply();
    val mintAmount = ton("2");
    val forwardPayload = beginCell().storeMaybeRef(null).endCell().beginParse();

    val mintMsg = createMessage({
        bounce: false,
        value: ton("0.315"),
        dest: minter.address,
        body: MintNewJettons {
            queryId: 0,
            mintRecipient: deployer.address,
            tonAmount: ton("0.3"),
            internalTransferMsg: InternalTransferStep {
                queryId: 0,
                jettonAmount: mintAmount,
                transferInitiator: null,
                sendExcessesTo: minter.address,
                forwardTonAmount: ton("0.05"),
                forwardPayload,
            }.toCell(),
        },
    });

    val res = testing.processSingleTraceStep(deployer.address, mintMsg);
    expect(minter.getTotalSupply()).toEqual(supplyBefore + mintAmount);

    val outMsg = res.outMessages.at<InternalTransferStep>(0);
    val outBody = outMsg.loadBody();
    val bouncedBody = beginCell()
        .storeUint(0xffffffff, 32)
        .storeSlice(outBody.toCell().beginParse())
        .endCell();
    val wallet = userWallet(minter, deployer.address);
    val bouncedMsg = createMessage({
        bounce: false,
        value: ton("0.95"),
        dest: minter.address,
        body: bouncedBody,
    }).bounced();
    testing.processSingleTraceStep(wallet.address, bouncedMsg);

    expect(minter.getTotalSupply()).toEqual(supplyBefore);
}

get fun `test wallet restores balance on internal_transfer bounce`() {
    val (minter, deployer, notDeployer) = setupTest();
    val senderWallet = mintTo(minter, deployer, deployer.address, ton("5"));
    val balanceBefore = senderWallet.getJettonBalance();
    val forwardPayload = beginCell().storeMaybeRef(null).endCell().beginParse();

    val transferMsg = createMessage({
        bounce: true,
        value: ton("0.3"),
        dest: senderWallet.address,
        body: AskToTransfer {
            queryId: 0,
            jettonAmount: ton("2"),
            transferRecipient: notDeployer.address,
            sendExcessesTo: deployer.address,
            customPayload: null,
            forwardTonAmount: 0,
            forwardPayload,
        },
    });

    val res = testing.processSingleTraceStep(deployer.address, transferMsg);
    expect(senderWallet.getJettonBalance()).toEqual(balanceBefore - ton("2"));

    val outMsg = res.outMessages.at<InternalTransferStep>(0);
    val outBody = outMsg.loadBody();
    val bouncedBody = beginCell()
        .storeUint(0xffffffff, 32)
        .storeSlice(outBody.toCell().beginParse())
        .endCell();
    val receiverWallet = userWallet(minter, notDeployer.address);
    val bouncedMsg = createMessage({
        bounce: false,
        value: ton("0.95"),
        dest: senderWallet.address,
        body: bouncedBody,
    }).bounced();
    testing.processSingleTraceStep(receiverWallet.address, bouncedMsg);

    expect(senderWallet.getJettonBalance()).toEqual(balanceBefore);
}

get fun `test wallet restores balance on burn_notification bounce`() {
    val (minter, deployer, _) = setupTest();
    val wallet = mintTo(minter, deployer, deployer.address, ton("4"));
    val balanceBefore = wallet.getJettonBalance();

    val burnMsg = createMessage({
        bounce: false,
        value: ton("0.2"),
        dest: wallet.address,
        body: AskToBurn {
            queryId: 0,
            jettonAmount: ton("1"),
            sendExcessesTo: deployer.address,
            customPayload: null,
        },
    });
    val res = testing.processSingleTraceStep(deployer.address, burnMsg);
    expect(wallet.getJettonBalance()).toEqual(balanceBefore - ton("1"));

    val outMsg = res.outMessages.at<BurnNotificationForMinter>(0);
    val outBody = outMsg.loadBody();
    val bouncedBody = beginCell()
        .storeUint(0xffffffff, 32)
        .storeSlice(outBody.toCell().beginParse())
        .endCell();
    val bouncedMsg = createMessage({
        bounce: false,
        value: ton("0.95"),
        dest: wallet.address,
        body: bouncedBody,
    }).bounced();
    testing.processSingleTraceStep(minter.address, bouncedMsg);

    expect(wallet.getJettonBalance()).toEqual(balanceBefore);
}
