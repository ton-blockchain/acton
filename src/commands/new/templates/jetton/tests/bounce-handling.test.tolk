import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"
import "@acton/types/message"

import "@contracts/messages"
import "@wrappers/JettonMinter.gen"
import "@wrappers/JettonWallet.gen"
import "@wrappers/utils"

import "test-utils"

get fun `test bounce: minter restores supply on internal_transfer bounce`() {
    val (minter, deployer, _) = setupTest();
    val supplyBefore = minter.getTotalSupply();
    val mintAmount = grams("2");
    val forwardPayload = beginCell().storeMaybeRef(null).endCell().beginParse();

    val mintMsg = createMessage({
        bounce: false,
        value: grams("0.315"),
        dest: minter.address,
        body: MintNewJettons {
            queryId: 0,
            mintRecipient: deployer.address,
            tonAmount: grams("0.3"),
            internalTransferMsg: InternalTransferStep {
                queryId: 0,
                jettonAmount: mintAmount,
                transferInitiator: null,
                sendExcessesTo: minter.address,
                forwardTonAmount: grams("0.05"),
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
        value: grams("0.95"),
        dest: minter.address,
        body: bouncedBody,
    }).bounced();
    testing.processSingleTraceStep(wallet.address, bouncedMsg);

    expect(minter.getTotalSupply()).toEqual(supplyBefore);
}

get fun `test bounce: wallet restores balance on internal_transfer bounce`() {
    val (minter, deployer, notDeployer) = setupTest();
    val senderWallet = mintTo(minter, deployer, deployer.address, grams("5"));
    val balanceBefore = senderWallet.getJettonBalance();
    val forwardPayload = beginCell().storeMaybeRef(null).endCell().beginParse();

    val transferMsg = createMessage({
        bounce: true,
        value: grams("0.3"),
        dest: senderWallet.address,
        body: AskToTransfer {
            queryId: 0,
            jettonAmount: grams("2"),
            transferRecipient: notDeployer.address,
            sendExcessesTo: deployer.address,
            customPayload: null,
            forwardTonAmount: 0,
            forwardPayload,
        },
    });

    val res = testing.processSingleTraceStep(deployer.address, transferMsg);
    expect(senderWallet.getJettonBalance()).toEqual(balanceBefore - grams("2"));

    val outMsg = res.outMessages.at<InternalTransferStep>(0);
    val outBody = outMsg.loadBody();
    val bouncedBody = beginCell()
        .storeUint(0xffffffff, 32)
        .storeSlice(outBody.toCell().beginParse())
        .endCell();
    val receiverWallet = userWallet(minter, notDeployer.address);
    val bouncedMsg = createMessage({
        bounce: false,
        value: grams("0.95"),
        dest: senderWallet.address,
        body: bouncedBody,
    }).bounced();
    testing.processSingleTraceStep(receiverWallet.address, bouncedMsg);

    expect(senderWallet.getJettonBalance()).toEqual(balanceBefore);
}

get fun `test bounce: wallet restores balance on burn_notification bounce`() {
    val (minter, deployer, _) = setupTest();
    val wallet = mintTo(minter, deployer, deployer.address, grams("4"));
    val balanceBefore = wallet.getJettonBalance();

    val burnMsg = createMessage({
        bounce: false,
        value: grams("0.2"),
        dest: wallet.address,
        body: AskToBurn {
            queryId: 0,
            jettonAmount: grams("1"),
            sendExcessesTo: deployer.address,
            customPayload: null,
        },
    });
    val res = testing.processSingleTraceStep(deployer.address, burnMsg);
    expect(wallet.getJettonBalance()).toEqual(balanceBefore - grams("1"));

    val outMsg = res.outMessages.at<BurnNotificationForMinter>(0);
    val outBody = outMsg.loadBody();
    val bouncedBody = beginCell()
        .storeUint(0xffffffff, 32)
        .storeSlice(outBody.toCell().beginParse())
        .endCell();
    val bouncedMsg = createMessage({
        bounce: false,
        value: grams("0.95"),
        dest: wallet.address,
        body: bouncedBody,
    }).bounced();
    testing.processSingleTraceStep(minter.address, bouncedMsg);

    expect(wallet.getJettonBalance()).toEqual(balanceBefore);
}
