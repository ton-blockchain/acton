import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"
import "@contracts/errors"
import "@contracts/messages"
import "@wrappers/JettonMinter.gen"
import "@wrappers/JettonWallet.gen"
import "@wrappers/utils"
import "test-utils"

get fun `test validation: malformed forward payload should be rejected`() {
    val (minter, deployer, _) = setupTest();
    val wallet = mintTo(minter, deployer, deployer.address, DEFAULT_MINTED_JETTONS);

    val payloadRef = beginCell().endCell();
    val badPayload = beginCell()
        .storeUint(1, 1)
        .storeRef(payloadRef)
        .storeUint(1, 1)
        .endCell()
        .beginParse();

    val outMessage = createMessage({
        bounce: true,
        value: DEFAULT_TRANSFER_MSG_VALUE,
        dest: wallet.address,
        body: buildTransferBody(ton("1"), 0, deployer.address, deployer.address, badPayload),
    });
    val res = net.send(deployer.address, outMessage);
    expect(res).toHaveFailedTx<AskToTransfer>({
        from: deployer.address,
        to: wallet.address,
        exitCode: 9,
    });
}

get fun `test validation: malformed custom payload should be rejected`() {
    val (minter, deployer, notDeployer) = setupTest();
    val wallet = mintTo(minter, deployer, deployer.address, DEFAULT_MINTED_JETTONS);

    val sentAmount = ton("0.5");
    val forwardAmount = DEFAULT_FORWARD_TON_AMOUNT;
    val forwardPayload = beginCell().storeUint(0x1234567890abcdef, 128).endCell();
    val customPayload = beginCell().storeUint(0xfedcba0987654321, 128).endCell();

    val malformedMissingRef = beginCell()
        .storeUint(0x0f8a7ea5, 32)
        .storeUint(0, 64)
        .storeCoins(sentAmount)
        .storeAddress(notDeployer.address)
        .storeAddress(deployer.address)
        .storeUint(1, 1)
        .storeCoins(forwardAmount)
        .storeUint(1, 1)
        .storeRef(forwardPayload)
        .endCell();

    val failRes1 = sendRawTransferPayload(deployer.address, wallet.address, malformedMissingRef);
    expect(failRes1).toHaveTx<AskToTransfer>({
        from: deployer.address,
        to: wallet.address,
        success: false,
        aborted: true,
    });

    val malformedUnexpectedRef = beginCell()
        .storeUint(0x0f8a7ea5, 32)
        .storeUint(0, 64)
        .storeCoins(sentAmount)
        .storeAddress(notDeployer.address)
        .storeAddress(deployer.address)
        .storeUint(0, 1)
        .storeRef(customPayload)
        .storeCoins(forwardAmount)
        .storeUint(1, 1)
        .storeRef(forwardPayload)
        .endCell();

    val failRes2 = sendRawTransferPayload(deployer.address, wallet.address, malformedUnexpectedRef);
    expect(failRes2).toHaveTx<AskToTransfer>({
        from: deployer.address,
        to: wallet.address,
        success: false,
        aborted: true,
    });

    val validPayload = beginCell()
        .storeUint(0x0f8a7ea5, 32)
        .storeUint(0, 64)
        .storeCoins(sentAmount)
        .storeAddress(notDeployer.address)
        .storeAddress(deployer.address)
        .storeUint(1, 1)
        .storeRef(customPayload)
        .storeCoins(forwardAmount)
        .storeUint(1, 1)
        .storeRef(forwardPayload)
        .endCell();

    val okRes = sendRawTransferPayload(deployer.address, wallet.address, validPayload);
    expect(okRes).toHaveSuccessfulTx<AskToTransfer>({
        from: deployer.address,
        to: wallet.address,
    });
}

get fun `test validation: wallet cannot send to masterchain`() {
    val (minter, deployer, _) = setupTest();
    val wallet = mintTo(minter, deployer, deployer.address, ton("1"));
    val masterAddr = address.fromWorkchainAndHash(-1, 0);

    val res = wallet.sendTransfer(
        deployer.address,
        DEFAULT_TRANSFER_MSG_VALUE,
        ton("1"),
        masterAddr,
        deployer.address,
        null,
        DEFAULT_FORWARD_TON_AMOUNT,
        null,
    );
    expect(res).toHaveFailedTx<AskToTransfer>({
        from: deployer.address,
        to: wallet.address,
        exitCode: Errors.WrongWorkchain,
    });
}

get fun `test validation: wallet should reject internal transfer from unauthorized sender`() {
    val (minter, deployer, notDeployer) = setupTest();
    val wallet = mintTo(minter, deployer, deployer.address, ton("2"));
    val balanceBefore = wallet.getJettonBalance();

    val outMessage = createMessage({
        bounce: true,
        value: DEFAULT_TRANSFER_MSG_VALUE,
        dest: wallet.address,
        body: InternalTransferStep {
            queryId: 0,
            jettonAmount: ton("0.01"),
            transferInitiator: deployer.address,
            sendExcessesTo: deployer.address,
            forwardTonAmount: DEFAULT_FORWARD_TON_AMOUNT,
            forwardPayload: makeTinyForwardPayload(),
        },
    });
    val res = net.send(notDeployer.address, outMessage);
    expect(res).toHaveFailedTx<InternalTransferStep>({
        from: notDeployer.address,
        to: wallet.address,
        exitCode: Errors.NotValidWallet,
    });
    expect(wallet.getJettonBalance()).toEqual(balanceBefore);
}

get fun `test validation: minter accepts burn notification only from wallet`() {
    val (minter, deployer, notDeployer) = setupTest();
    val wallet = mintTo(minter, deployer, deployer.address, ton("2"));
    val wrongSender = testing.treasury("outsider");

    val badBurnNotify = createMessage({
        bounce: true,
        value: ton("0.2"),
        dest: minter.address,
        body: BurnNotificationForMinter {
            queryId: 0,
            jettonAmount: ton("1"),
            burnInitiator: deployer.address,
            sendExcessesTo: notDeployer.address,
        },
    });
    val res = net.send(wrongSender.address, badBurnNotify);
    expect(res).toHaveFailedTx<BurnNotificationForMinter>({
        from: wrongSender.address,
        to: minter.address,
        exitCode: Errors.NotValidWallet,
    });

    val burn = wallet.sendAskToBurn(
        deployer.address,
        0,
        ton("1"),
        deployer.address,
        null,
        { bounce: false, value: ton("0.2") },
    );
    expect(burn).toHaveSuccessfulTx<AskToBurn>({
        from: deployer.address,
        to: wallet.address,
    });
}

get fun `test validation: minter cannot force burn tokens`() {
    val (minter, deployer, notDeployer) = setupTest();
    val wallet = mintTo(minter, deployer, notDeployer.address, ton("3"));
    val balanceBefore = wallet.getJettonBalance();

    val burnMsg = createMessage({
        bounce: true,
        value: ton("1"),
        dest: wallet.address,
        body: AskToBurn {
            queryId: 0,
            jettonAmount: ton("1"),
            sendExcessesTo: null,
            customPayload: null,
        },
    });
    val res = net.send(minter.address, burnMsg);
    expect(res).toHaveFailedTx<AskToBurn>({
        from: minter.address,
        to: wallet.address,
        exitCode: Errors.NotOwner,
    });
    expect(wallet.getJettonBalance()).toEqual(balanceBefore);

    val ok = wallet.sendAskToBurn(
        notDeployer.address,
        0,
        ton("1"),
        notDeployer.address,
        null,
        { bounce: false, value: ton("0.2") },
    );
    expect(ok).toHaveSuccessfulTx<AskToBurn>({
        from: notDeployer.address,
        to: wallet.address,
    });
}

get fun `test validation: minter cannot force transfer tokens`() {
    val (minter, deployer, notDeployer) = setupTest();
    val wallet = mintTo(minter, deployer, notDeployer.address, ton("2"));
    val target = testing.treasury("receiver");
    val balanceBefore = wallet.getJettonBalance();

    val transferMsg = createMessage({
        bounce: true,
        value: ton("1"),
        dest: wallet.address,
        body: AskToTransfer {
            queryId: 0,
            jettonAmount: ton("1"),
            transferRecipient: target.address,
            sendExcessesTo: notDeployer.address,
            customPayload: null,
            forwardTonAmount: 0,
            forwardPayload: beginCell().storeMaybeRef(null).endCell().beginParse(),
        },
    });
    val res = net.send(minter.address, transferMsg);
    expect(res).toHaveFailedTx<AskToTransfer>({
        from: minter.address,
        to: wallet.address,
        exitCode: Errors.NotOwner,
    });
    expect(wallet.getJettonBalance()).toEqual(balanceBefore);

    val ok = wallet.sendTransfer(
        notDeployer.address,
        DEFAULT_TRANSFER_MSG_VALUE,
        ton("1"),
        target.address,
        notDeployer.address,
        null,
        0,
        null,
    );
    expect(ok).toHaveSuccessfulTx<AskToTransfer>({
        from: notDeployer.address,
        to: wallet.address,
    });
}
