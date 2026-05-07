import "@stdlib/gas-payments"
import "@acton/emulation/config"
import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"

import "@contracts/errors"
import "@contracts/fees-management"
import "@contracts/messages"
import "@wrappers/JettonWallet.gen"
import "@wrappers/utils"

import "test-utils"

get fun `test gas: transfer minimal value edge`() {
    val (minter, deployer, _) = setupTest();
    val wallet = mintTo(minter, deployer, deployer.address, DEFAULT_MINTED_JETTONS);

    val dest = randomAddress();
    val body = buildTransferBody(TEST_JETTON_UNIT, 0, dest, dest, makeTinyForwardPayload());

    val fwdFee = estimateBodyForwardFee(body);
    val minimalTonValue = calcMinimalTransferAmount(0, fwdFee) + MIN_EDGE_DELTA;

    val enough = createMessage({
        body,
        bounce: false,
        dest: wallet.address,
        value: minimalTonValue,
    });
    val enoughRes = net.send(deployer.address, enough);
    expect(enoughRes).toHaveSuccessfulTx<AskToTransfer>({
        from: deployer.address,
        to: wallet.address,
    });

    val notEnough = createMessage({
        body,
        bounce: false,
        dest: wallet.address,
        value: minimalTonValue - MIN_EDGE_DELTA,
    });
    val notEnoughRes = net.send(deployer.address, notEnough);
    expect(notEnoughRes).toHaveFailedTx<AskToTransfer>({
        from: deployer.address,
        to: wallet.address,
        exitCode: Errors.NotEnoughGas,
    });
}

get fun `test gas: transfer minimal value is message-size aware`() {
    val dest = randomAddress();

    val bodySmall = buildTransferBody(TEST_JETTON_UNIT, 10, dest, dest, makeTinyForwardPayload());
    val feeSmall = estimateBodyForwardFee(bodySmall);
    val minSmall = calcMinimalTransferAmount(10, feeSmall) + MIN_EDGE_DELTA;

    val bodyBig = buildTransferBody(
        TEST_JETTON_UNIT,
        10,
        dest,
        dest,
        beginCell().storeUint(10, 100).toSlice(),
    );
    val feeBig = estimateBodyForwardFee(bodyBig);
    val minBig = calcMinimalTransferAmount(10, feeBig) + MIN_EDGE_DELTA;

    expect(minSmall).toBeLess(minBig);
}

get fun `test gas: transfer with forward amount requires second forward fee`() {
    val (minter, deployer, _) = setupTest();
    val wallet = mintTo(minter, deployer, deployer.address, DEFAULT_MINTED_JETTONS);

    val dest = randomAddress();
    val body = buildTransferBody(
        TEST_JETTON_UNIT,
        DEFAULT_FORWARD_TON_AMOUNT,
        dest,
        dest,
        makeRichForwardPayload(),
    );

    val fwdFee = estimateBodyForwardFee(body);
    val minWithoutForward = calcMinimalTransferAmount(0, fwdFee) + MIN_EDGE_DELTA;
    val minMissingSecondFee = minWithoutForward + DEFAULT_FORWARD_TON_AMOUNT;
    val minWithForward = calcMinimalTransferAmount(DEFAULT_FORWARD_TON_AMOUNT, fwdFee) +
    MIN_EDGE_DELTA;

    expect(minWithForward).toEqual(minWithoutForward + DEFAULT_FORWARD_TON_AMOUNT + fwdFee);

    val missingFeeMsg = createMessage({
        body,
        bounce: false,
        dest: wallet.address,
        value: minMissingSecondFee,
    });
    val missingFeeRes = net.send(deployer.address, missingFeeMsg);
    expect(missingFeeRes).toHaveFailedTx<AskToTransfer>({
        from: deployer.address,
        to: wallet.address,
        exitCode: Errors.NotEnoughGas,
    });

    val enoughMsg = createMessage({
        body,
        bounce: false,
        dest: wallet.address,
        value: minWithForward,
    });
    val enoughRes = net.send(deployer.address, enoughMsg);
    expect(enoughRes).toHaveSuccessfulTx<AskToTransfer>({
        from: deployer.address,
        to: wallet.address,
    });

    val edgeFailMsg = createMessage({
        body,
        bounce: false,
        dest: wallet.address,
        value: minWithForward - MIN_EDGE_DELTA,
    });
    val edgeFailRes = net.send(deployer.address, edgeFailMsg);
    expect(edgeFailRes).toHaveFailedTx<AskToTransfer>({
        from: deployer.address,
        to: wallet.address,
        exitCode: Errors.NotEnoughGas,
    });
}

get fun `test gas: transfer forward fees should follow config`() {
    val (minter, deployer, _) = setupTest();
    val wallet = mintTo(minter, deployer, deployer.address, DEFAULT_MINTED_JETTONS);

    val dest = randomAddress();
    val body = buildTransferBody(
        TEST_JETTON_UNIT,
        DEFAULT_FORWARD_TON_AMOUNT,
        dest,
        dest,
        makeRichForwardPayload(),
    );

    val fwdFeeBefore = estimateBodyForwardFee(body);
    val minimalBefore = calcMinimalTransferAmount(DEFAULT_FORWARD_TON_AMOUNT, fwdFeeBefore) +
    MIN_EDGE_DELTA;

    var config = testing.getConfig();
    var msgPrices = config.getMsgForwardPrices(BASECHAIN);
    msgPrices.bitPrice *= CONFIG_SCALE_X10;
    msgPrices.cellPrice *= CONFIG_SCALE_X10;
    config.setMsgForwardPrices(msgPrices, BASECHAIN);
    expect(testing.setConfig(config)).toEqual(true);

    val fwdFeeAfter = estimateBodyForwardFee(body);
    var minimalAfter = calcMinimalTransferAmount(DEFAULT_FORWARD_TON_AMOUNT, fwdFeeAfter) +
    MIN_EDGE_DELTA;

    expect(fwdFeeBefore).toBeLess(fwdFeeAfter);
    expect(minimalBefore).toBeLess(minimalAfter);

    var foundEnough = 0;
    var attempts = 0;
    while (attempts < TRANSFER_MIN_PROBE_LIMIT && foundEnough == 0) {
        val probe = createMessage({
            body,
            bounce: true,
            dest: wallet.address,
            value: minimalAfter,
        });
        val probeRes = net.send(deployer.address, probe);
        val okTx = probeRes.findTransaction<AskToTransfer>({
            from: deployer.address,
            to: wallet.address,
            success: true,
            exitCode: 0,
        });

        if (okTx == null) {
            minimalAfter += 1;
            attempts += 1;
        } else {
            foundEnough = 1;
        }
    }
    expect(foundEnough).toEqual(1);

    val enoughMsg = createMessage({
        body,
        bounce: false,
        dest: wallet.address,
        value: minimalAfter,
    });
    val enoughRes = net.send(deployer.address, enoughMsg);
    expect(enoughRes).toHaveSuccessfulTx<AskToTransfer>({
        from: deployer.address,
        to: wallet.address,
    });

    val failMsg = createMessage({
        body,
        bounce: false,
        dest: wallet.address,
        value: minimalAfter - MIN_EDGE_DELTA,
    });
    val failRes = net.send(deployer.address, failMsg);
    expect(failRes).toHaveFailedTx<AskToTransfer>({
        from: deployer.address,
        to: wallet.address,
        exitCode: Errors.NotEnoughGas,
    });
}

get fun `test gas: transfer gas fee should follow config`() {
    val (minter, deployer, _) = setupTest();
    val wallet = mintTo(minter, deployer, deployer.address, DEFAULT_MINTED_JETTONS);

    val dest = randomAddress();
    val body = buildTransferBody(
        TEST_JETTON_UNIT,
        DEFAULT_FORWARD_TON_AMOUNT,
        dest,
        dest,
        makeRichForwardPayload(),
    );

    val fwdFee = estimateBodyForwardFee(body);
    val minimalBefore = calcMinimalTransferAmount(DEFAULT_FORWARD_TON_AMOUNT, fwdFee) +
    MIN_EDGE_DELTA;

    var config = testing.getConfig();
    var gasPrices = config.getGasPrices(BASECHAIN);
    var gasPricesExtended = gasPrices.other;
    gasPricesExtended.gasPrice *= GAS_SCALE_X3;
    gasPrices.other = gasPricesExtended;
    config.setGasPrices(gasPrices, BASECHAIN);
    expect(testing.setConfig(config)).toEqual(true);

    val minimalAfter = calcMinimalTransferAmount(DEFAULT_FORWARD_TON_AMOUNT, fwdFee) +
    MIN_EDGE_DELTA;
    expect(minimalBefore).toBeLess(minimalAfter);

    val enough = createMessage({
        body,
        bounce: false,
        dest: wallet.address,
        value: minimalAfter,
    });
    val enoughRes = net.send(deployer.address, enough);
    expect(enoughRes).toHaveSuccessfulTx<AskToTransfer>({
        from: deployer.address,
        to: wallet.address,
    });

    val fail = createMessage({
        body,
        bounce: false,
        dest: wallet.address,
        value: minimalAfter - MIN_EDGE_DELTA,
    });
    val failRes = net.send(deployer.address, fail);
    expect(failRes).toHaveFailedTx<AskToTransfer>({
        from: deployer.address,
        to: wallet.address,
        exitCode: Errors.NotEnoughGas,
    });
}

get fun `test gas: transfer storage fee should follow config`() {
    val (minter, deployer, _) = setupTest();
    val wallet = mintTo(minter, deployer, deployer.address, DEFAULT_MINTED_JETTONS);

    val dest = randomAddress();
    val body = buildTransferBody(
        TEST_JETTON_UNIT,
        DEFAULT_FORWARD_TON_AMOUNT,
        dest,
        dest,
        makeRichForwardPayload(),
    );

    val fwdFee = estimateBodyForwardFee(body);
    val minimalBefore = calcMinimalTransferAmount(DEFAULT_FORWARD_TON_AMOUNT, fwdFee) +
    MIN_EDGE_DELTA;

    var config = testing.getConfig();
    var storagePrices = config.getStoragePrices();
    var initialStoragePrices = storagePrices.getInitial();
    initialStoragePrices.bitPrice *= CONFIG_SCALE_X10;
    initialStoragePrices.cellPrice *= CONFIG_SCALE_X10;
    storagePrices.setInitial(initialStoragePrices);
    config.setStoragePrices(storagePrices);
    expect(testing.setConfig(config)).toEqual(true);

    val minimalAfter = calcMinimalTransferAmount(DEFAULT_FORWARD_TON_AMOUNT, fwdFee) +
    MIN_EDGE_DELTA;
    expect(minimalBefore).toBeLess(minimalAfter);

    val enough = createMessage({
        body,
        bounce: false,
        dest: wallet.address,
        value: minimalAfter,
    });
    val enoughRes = net.send(deployer.address, enough);
    expect(enoughRes).toHaveSuccessfulTx<AskToTransfer>({
        from: deployer.address,
        to: wallet.address,
    });

    val fail = createMessage({
        body,
        bounce: false,
        dest: wallet.address,
        value: minimalAfter - MIN_EDGE_DELTA,
    });
    val failRes = net.send(deployer.address, fail);
    expect(failRes).toHaveFailedTx<AskToTransfer>({
        from: deployer.address,
        to: wallet.address,
        exitCode: Errors.NotEnoughGas,
    });
}

get fun `test gas: forward ton transfer should fail when fees are insufficient`() {
    val (minter, deployer, notDeployer) = setupTest();
    val senderWallet = mintTo(minter, deployer, deployer.address, ton("2"));

    val res = senderWallet.sendTransfer(
        deployer.address,
        LOW_TRANSFER_MSG_VALUE,
        ton("1"),
        notDeployer.address,
        deployer.address,
        null,
        ton("0.5"),
        null,
    );
    expect(res).toHaveFailedTx<AskToTransfer>({
        from: deployer.address,
        to: senderWallet.address,
        exitCode: Errors.NotEnoughGas,
    });
}

get fun `test gas: burn minimal value edge`() {
    val (minter, deployer, _) = setupTest();
    val wallet = mintTo(minter, deployer, deployer.address, DEFAULT_MINTED_JETTONS);

    val minimalTonValue = calcMinimalBurnAmount() + MIN_EDGE_DELTA;

    val enough = wallet.sendAskToBurn(
        deployer.address,
        0,
        TEST_JETTON_UNIT,
        deployer.address,
        null,
        { bounce: false, value: minimalTonValue },
    );
    expect(enough).toHaveSuccessfulTx<AskToBurn>({
        from: deployer.address,
        to: wallet.address,
    });

    val fail = wallet.sendAskToBurn(
        deployer.address,
        0,
        TEST_JETTON_UNIT,
        deployer.address,
        null,
        { bounce: false, value: minimalTonValue - MIN_EDGE_DELTA },
    );
    expect(fail).toHaveFailedTx<AskToBurn>({
        from: deployer.address,
        to: wallet.address,
        exitCode: Errors.NotEnoughGas,
    });
}

get fun `test gas: burn custom payload should not affect minimal value`() {
    val (minter, deployer, _) = setupTest();
    val wallet = mintTo(minter, deployer, deployer.address, DEFAULT_MINTED_JETTONS);

    val customPayload = beginCell()
        .storeMaybeRef(beginCell().storeUint(0x123456789abcdef, 128).endCell())
        .endCell();

    val minimalTonValue = calcMinimalBurnAmount() + MIN_EDGE_DELTA;

    val enough = wallet.sendAskToBurn(
        deployer.address,
        0,
        TEST_JETTON_UNIT,
        deployer.address,
        customPayload,
        { bounce: false, value: minimalTonValue },
    );
    expect(enough).toHaveSuccessfulTx<AskToBurn>({
        from: deployer.address,
        to: wallet.address,
    });

    val fail = wallet.sendAskToBurn(
        deployer.address,
        0,
        TEST_JETTON_UNIT,
        deployer.address,
        customPayload,
        { bounce: false, value: minimalTonValue - MIN_EDGE_DELTA },
    );
    expect(fail).toHaveFailedTx<AskToBurn>({
        from: deployer.address,
        to: wallet.address,
        exitCode: Errors.NotEnoughGas,
    });
}

get fun `test gas: burn forward fee should follow config`() {
    val (minter, deployer, _) = setupTest();
    val wallet = mintTo(minter, deployer, deployer.address, DEFAULT_MINTED_JETTONS);

    val minimalBefore = calcMinimalBurnAmount() + MIN_EDGE_DELTA;

    val enoughBefore = wallet.sendAskToBurn(
        deployer.address,
        0,
        TEST_JETTON_UNIT,
        deployer.address,
        null,
        { bounce: false, value: minimalBefore },
    );
    expect(enoughBefore).toHaveSuccessfulTx<AskToBurn>({
        from: deployer.address,
        to: wallet.address,
    });

    var config = testing.getConfig();
    var msgPrices = config.getMsgForwardPrices(BASECHAIN);
    msgPrices.bitPrice *= CONFIG_SCALE_X10;
    msgPrices.cellPrice *= CONFIG_SCALE_X10;
    config.setMsgForwardPrices(msgPrices, BASECHAIN);
    expect(testing.setConfig(config)).toEqual(true);

    val minimalAfter = calcMinimalBurnAmount() + MIN_EDGE_DELTA;
    expect(minimalBefore).toBeLess(minimalAfter);

    val enoughAfter = wallet.sendAskToBurn(
        deployer.address,
        0,
        TEST_JETTON_UNIT,
        deployer.address,
        null,
        { bounce: false, value: minimalAfter },
    );
    expect(enoughAfter).toHaveSuccessfulTx<AskToBurn>({
        from: deployer.address,
        to: wallet.address,
    });

    val failAfter = wallet.sendAskToBurn(
        deployer.address,
        0,
        TEST_JETTON_UNIT,
        deployer.address,
        null,
        { bounce: false, value: minimalAfter - MIN_EDGE_DELTA },
    );
    expect(failAfter).toHaveFailedTx<AskToBurn>({
        from: deployer.address,
        to: wallet.address,
        exitCode: Errors.NotEnoughGas,
    });
}

get fun `test gas: burn gas fee should follow config`() {
    val (minter, deployer, _) = setupTest();
    val wallet = mintTo(minter, deployer, deployer.address, DEFAULT_MINTED_JETTONS);

    val minimalBefore = calcMinimalBurnAmount() + MIN_EDGE_DELTA;

    val enoughBefore = wallet.sendAskToBurn(
        deployer.address,
        0,
        TEST_JETTON_UNIT,
        deployer.address,
        null,
        { bounce: false, value: minimalBefore },
    );
    expect(enoughBefore).toHaveSuccessfulTx<AskToBurn>({
        from: deployer.address,
        to: wallet.address,
    });

    var config = testing.getConfig();
    var gasPrices = config.getGasPrices(BASECHAIN);
    var gasPricesExtended = gasPrices.other;
    gasPricesExtended.gasPrice *= GAS_SCALE_X3;
    gasPrices.other = gasPricesExtended;
    config.setGasPrices(gasPrices, BASECHAIN);
    expect(testing.setConfig(config)).toEqual(true);

    val minimalAfter = calcMinimalBurnAmount() + MIN_EDGE_DELTA;
    expect(minimalBefore).toBeLess(minimalAfter);

    val enoughAfter = wallet.sendAskToBurn(
        deployer.address,
        0,
        TEST_JETTON_UNIT,
        deployer.address,
        null,
        { bounce: false, value: minimalAfter },
    );
    expect(enoughAfter).toHaveSuccessfulTx<AskToBurn>({
        from: deployer.address,
        to: wallet.address,
    });

    val failAfter = wallet.sendAskToBurn(
        deployer.address,
        0,
        TEST_JETTON_UNIT,
        deployer.address,
        null,
        { bounce: false, value: minimalAfter - MIN_EDGE_DELTA },
    );
    expect(failAfter).toHaveFailedTx<AskToBurn>({
        from: deployer.address,
        to: wallet.address,
        exitCode: Errors.NotEnoughGas,
    });
}

get fun `test gas: burn should fail when sent tons are clearly insufficient`() {
    val (minter, deployer, _) = setupTest();
    val wallet = mintTo(minter, deployer, deployer.address, ton("1"));

    val res = wallet.sendAskToBurn(
        deployer.address,
        0,
        ton("1"),
        deployer.address,
        null,
        { bounce: false, value: ton("0.001") },
    );
    expect(res).toHaveFailedTx<AskToBurn>({
        from: deployer.address,
        to: wallet.address,
        exitCode: Errors.NotEnoughGas,
    });
}

get fun `test gas: wallet storage fee should be measurable`() {
    val (minter, deployer, _) = setupTest();
    val wallet = mintTo(minter, deployer, deployer.address, ton("1"));

    val state = testing.getAccountState(wallet.address);
    expect(state).toBeNotNull();
    if (state == null) {
        return;
    }

    val (cells, bits, _) = state!.toCell().calculateSizeStrict(4096);
    val storageFee = calculateStorageFee(BASECHAIN, MIN_STORAGE_DURATION, bits, cells);

    expect(storageFee).toBeGreater(0);
    expect(calculateJettonWalletMinStorageFee()).toBeGreater(0);
}
