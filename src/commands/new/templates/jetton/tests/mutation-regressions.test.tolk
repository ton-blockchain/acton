import "@acton/build"
import "@acton/emulation/config"
import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"
import "@acton/types/message"
import "@acton/types/out_actions"
import "@acton/types/transaction"
import "@contracts/errors"
import "@contracts/fees-management"
import "@contracts/messages"
import "@contracts/storage"
import "@wrappers/JettonMinter.gen"
import "@wrappers/JettonWallet.gen"
import "@wrappers/utils"
import "test-utils"

fun expectSendActionMode(
    res: SendResultList,
    account: address,
    actionIndex: int,
    expectedMode: int,
) {
    val item = findSendResultForAccount(res, account);
    expect(item).toBeNotNull();
    if (item == null) {
        return;
    }

    val action = item.allOutActions().getSendMessageAt(actionIndex);
    expect(action).toBeNotNull();
    if (action == null) {
        return;
    }

    expect(action.mode).toEqual(expectedMode);
}

fun makeBadEitherPayload(): slice {
    val payloadRef = beginCell().endCell();
    return beginCell().storeUint(1, 1).storeRef(payloadRef).storeUint(1, 1).endCell().beginParse();
}

fun estimateMinimalMintTonAmount(
    minterAddress: address,
    recipient: address,
    forwardTonAmount: int,
    forwardPayload: slice,
    jettonAmount: int = TEST_JETTON_UNIT,
): int {
    var tonAmount = 0;
    var previousTonAmount = -1;

    while (tonAmount != previousTonAmount) {
        previousTonAmount = tonAmount;
        val body = MintNewJettons {
            queryId: 0,
            mintRecipient: recipient,
            tonAmount,
            internalTransferMsg: InternalTransferStep {
                queryId: 0,
                jettonAmount,
                transferInitiator: null,
                sendExcessesTo: minterAddress,
                forwardTonAmount,
                forwardPayload,
            }.toCell(),
        };
        val fwdFee = estimateMessageForwardFee(
            body,
            minterAddress,
            false,
            tonAmount + grams("0.015"),
        );
        tonAmount = calcMinimalTransferAmount(forwardTonAmount, fwdFee) + MIN_EDGE_DELTA;
    }

    return tonAmount;
}

fun mintAmountMeetsExpectedThreshold(
    minterAddress: address,
    recipient: address,
    forwardTonAmount: int,
    tonAmount: int,
    forwardPayload: slice,
    jettonAmount: int = TEST_JETTON_UNIT,
): bool {
    val body = MintNewJettons {
        queryId: 0,
        mintRecipient: recipient,
        tonAmount,
        internalTransferMsg: InternalTransferStep {
            queryId: 0,
            jettonAmount,
            transferInitiator: null,
            sendExcessesTo: minterAddress,
            forwardTonAmount,
            forwardPayload,
        }.toCell(),
    };
    val fwdFee = estimateMessageForwardFee(body, minterAddress, false, tonAmount + grams("0.015"));
    return tonAmount > calcMinimalTransferAmount(forwardTonAmount, fwdFee);
}

fun findExactExpectedMintTonAmount(
    minterAddress: address,
    recipient: address,
    forwardTonAmount: int,
    upperBound: int,
    forwardPayload: slice,
): int {
    var low = 0;
    var high = upperBound;

    while (
        !mintAmountMeetsExpectedThreshold(
            minterAddress,
            recipient,
            forwardTonAmount,
            high,
            forwardPayload,
        )
    ) {
        high += MIN_EDGE_DELTA;
    }

    while (low + 1 < high) {
        val mid = (low + high) / 2;
        if (
            mintAmountMeetsExpectedThreshold(
                minterAddress,
                recipient,
                forwardTonAmount,
                mid,
                forwardPayload,
            )
        ) {
            high = mid;
        } else {
            low = mid;
        }
    }

    return high;
}

get fun `test mutation: direct wallet internal transfer should persist balance and action modes`() {
    val startBalance = grams("2");
    val (wallet, owner, minter, _) = setupDirectWallet(startBalance);

    val res = net.send(
        minter.address,
        createMessage({
            bounce: true,
            value: grams("0.4"),
            dest: wallet.address,
            body: InternalTransferStep {
                queryId: 0,
                jettonAmount: grams("1"),
                transferInitiator: null,
                sendExcessesTo: owner.address,
                forwardTonAmount: DEFAULT_FORWARD_TON_AMOUNT,
                forwardPayload: makeRichForwardPayload(),
            },
        }),
    );
    expect(res).toHaveSuccessfulTx<InternalTransferStep>({
        from: minter.address,
        to: wallet.address,
    });
    expect(wallet.getJettonBalance()).toEqual(startBalance + grams("1"));

    val walletTx = findSendResultForAccount(res, wallet.address);
    expect(walletTx).toBeNotNull();
    if (walletTx == null) {
        return;
    }
    expect(walletTx.allOutActions().at(1).kind()).toEqual("reserve-currency");
    expectSendActionMode(
        res,
        wallet.address,
        0,
        SEND_MODE_CARRY_ALL_BALANCE + SEND_MODE_IGNORE_ERRORS,
    );
    expectSendActionMode(
        res,
        wallet.address,
        2,
        SEND_MODE_PAY_FEES_SEPARATELY + SEND_MODE_BOUNCE_ON_ACTION_FAIL,
    );

    val excessTx = res.findTransaction<ReturnExcessesBack>({
        from: wallet.address,
        to: owner.address,
        success: true,
        exitCode: 0,
    });
    expect(excessTx).toBeNotNull();
    expect(excessTx!.loadInMsg<ReturnExcessesBack>().info.toInternal().value.grams).toBeGreater(0);
}

get fun `test mutation: direct wallet should accept sibling transfers and reject outsiders`() {
    val senderOwner = testing.treasury("direct-wallet-sender-owner");
    val minter = testing.treasury("direct-wallet-pair-minter");
    val receiverOwner = testing.treasury("direct-wallet-receiver-owner");
    val outsider = testing.treasury("direct-wallet-pair-outsider");

    val senderWallet = deployWalletFromStorage(
        WalletStorage {
            jettonBalance: 0,
            ownerAddress: senderOwner.address,
            minterAddress: minter.address,
        },
        senderOwner.address,
    );
    val receiverWallet = deployWalletFromStorage(
        WalletStorage {
            jettonBalance: 0,
            ownerAddress: receiverOwner.address,
            minterAddress: minter.address,
        },
        receiverOwner.address,
    );

    val ok = net.send(
        senderWallet.address,
        createMessage({
            bounce: true,
            value: grams("0.3"),
            dest: receiverWallet.address,
            body: InternalTransferStep {
                queryId: 0,
                jettonAmount: grams("2"),
                transferInitiator: senderOwner.address,
                sendExcessesTo: null,
                forwardTonAmount: 0,
                forwardPayload: makeTinyForwardPayload(),
            },
        }),
    );
    expect(ok).toHaveSuccessfulTx<InternalTransferStep>({
        from: senderWallet.address,
        to: receiverWallet.address,
    });
    expect(receiverWallet.getJettonBalance()).toEqual(grams("2"));

    val bad = net.send(
        outsider.address,
        createMessage({
            bounce: true,
            value: grams("0.3"),
            dest: receiverWallet.address,
            body: InternalTransferStep {
                queryId: 0,
                jettonAmount: grams("1"),
                transferInitiator: senderOwner.address,
                sendExcessesTo: null,
                forwardTonAmount: 0,
                forwardPayload: makeTinyForwardPayload(),
            },
        }),
    );
    expect(bad).toHaveFailedTx<InternalTransferStep>({
        from: outsider.address,
        to: receiverWallet.address,
        exitCode: Errors.NotValidWallet,
    });
    expect(receiverWallet.getJettonBalance()).toEqual(grams("2"));
}

get fun `test mutation: direct wallet zero forward GRAM should not notify owner`() {
    val (wallet, owner, minter, _) = setupDirectWallet();

    val res = net.send(
        minter.address,
        createMessage({
            bounce: true,
            value: grams("0.3"),
            dest: wallet.address,
            body: InternalTransferStep {
                queryId: 0,
                jettonAmount: grams("1"),
                transferInitiator: null,
                sendExcessesTo: null,
                forwardTonAmount: 0,
                forwardPayload: makeTinyForwardPayload(),
            },
        }),
    );
    expect(res).toHaveSuccessfulTx<InternalTransferStep>({
        from: minter.address,
        to: wallet.address,
    });
    expect(res).toNotHaveTx<TransferNotificationForRecipient>({
        from: wallet.address,
        to: owner.address,
    });
}

get fun `test mutation: direct wallet exact-balance transfer should persist and use deploy mode`() {
    val startBalance = grams("3");
    val (wallet, owner, _, _) = setupDirectWallet(startBalance);
    val recipientOwner = testing.treasury("direct-wallet-transfer-recipient");

    val res = wallet.sendTransferPayloadInline(
        owner.address,
        DEFAULT_TRANSFER_MSG_VALUE,
        startBalance,
        recipientOwner.address,
        owner.address,
        null,
        0,
        createEmptySlice(),
    );
    expect(res).toHaveSuccessfulTx<AskToTransfer>({ from: owner.address, to: wallet.address });
    expect(wallet.getJettonBalance()).toEqual(0);
    expectSendActionMode(
        res,
        wallet.address,
        0,
        SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE + SEND_MODE_BOUNCE_ON_ACTION_FAIL,
    );
}

get fun `test mutation: direct wallet should reject non-owner transfers`() {
    val (wallet, _, _, outsider) = setupDirectWallet(grams("3"));
    val receiver = testing.treasury("direct-wallet-non-owner-receiver");

    val res = wallet.sendTransferPayloadInline(
        outsider.address,
        DEFAULT_TRANSFER_MSG_VALUE,
        grams("1"),
        receiver.address,
        outsider.address,
        null,
        0,
        createEmptySlice(),
    );
    expect(res).toHaveFailedTx<AskToTransfer>({
        from: outsider.address,
        to: wallet.address,
        exitCode: Errors.NotOwner,
    });
}

get fun `test mutation: direct wallet should reject overspending jettons`() {
    val owner = testing.treasury("direct-wallet-overspend-owner");
    val minter = testing.treasury("direct-wallet-overspend-minter");
    val recipientOwner = testing.treasury("direct-wallet-overspend-recipient");
    val wallet = deployWalletFromStorage(
        WalletStorage {
            jettonBalance: grams("2"),
            ownerAddress: owner.address,
            minterAddress: minter.address,
        },
        owner.address,
    );
    val recipientWallet = deployWalletFromStorage(
        WalletStorage {
            jettonBalance: 0,
            ownerAddress: recipientOwner.address,
            minterAddress: minter.address,
        },
        recipientOwner.address,
    );

    val res = wallet.sendTransferPayloadInline(
        owner.address,
        DEFAULT_TRANSFER_MSG_VALUE,
        grams("5"),
        recipientOwner.address,
        owner.address,
        null,
        0,
        createEmptySlice(),
    );
    expect(res).toHaveFailedTx<AskToTransfer>({
        from: owner.address,
        to: wallet.address,
        exitCode: Errors.BalanceError,
    });
    expect(wallet.getJettonBalance()).toEqual(grams("2"));
    expect(recipientWallet.getJettonBalance()).toEqual(0);
}

get fun `test mutation: direct wallet should reject masterchain recipients`() {
    val (wallet, owner, _, _) = setupDirectWallet(grams("3"));
    val masterAddr = address.fromWorkchainAndHash(-1, 0);

    val res = wallet.sendTransferPayloadInline(
        owner.address,
        DEFAULT_TRANSFER_MSG_VALUE,
        grams("1"),
        masterAddr,
        owner.address,
        null,
        0,
        createEmptySlice(),
    );
    expect(res).toHaveFailedTx<AskToTransfer>({
        from: owner.address,
        to: wallet.address,
        exitCode: Errors.WrongWorkchain,
    });
}

get fun `test mutation: direct wallet should reject malformed forward payload`() {
    val (wallet, owner, _, _) = setupDirectWallet(DEFAULT_MINTED_JETTONS);

    val res = net.send(
        owner.address,
        createMessage({
            bounce: true,
            value: DEFAULT_TRANSFER_MSG_VALUE,
            dest: wallet.address,
            body: buildTransferBody(
                grams("1"),
                0,
                owner.address,
                owner.address,
                makeBadEitherPayload(),
            ),
        }),
    );
    expect(res).toHaveFailedTx<AskToTransfer>({
        from: owner.address,
        to: wallet.address,
        exitCode: 9,
    });
}

get fun `test mutation: direct wallet unknown opcode should fail`() {
    val (wallet, owner, _, _) = setupDirectWallet(grams("1"));

    val res = net.send(
        owner.address,
        createMessage({
            bounce: true,
            value: grams("0.1"),
            dest: wallet.address,
            body: beginCell().storeUint(0x7fffffff, 32).storeUint(1, 64).endCell(),
        }),
    );
    expect(res).toHaveFailedTx({
        from: owner.address,
        to: wallet.address,
        exitCode: 0xFFFF,
    });
}

get fun `test mutation: direct wallet exact-balance burn should persist and use minter mode`() {
    val startBalance = grams("2");
    val (wallet, owner, _, _) = setupDirectWallet(startBalance);

    val res = wallet.sendBurn(
        owner.address,
        DEFAULT_BURN_MSG_VALUE,
        startBalance,
        owner.address,
        null,
    );
    expect(res).toHaveSuccessfulTx<AskToBurn>({ from: owner.address, to: wallet.address });
    expect(wallet.getJettonBalance()).toEqual(0);
    expectSendActionMode(
        res,
        wallet.address,
        0,
        SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE + SEND_MODE_BOUNCE_ON_ACTION_FAIL,
    );
}

get fun `test mutation: direct wallet should reject non-owner burns`() {
    val (wallet, _, _, outsider) = setupDirectWallet(grams("2"));

    val res = wallet.sendBurn(
        outsider.address,
        DEFAULT_BURN_MSG_VALUE,
        grams("1"),
        outsider.address,
        null,
    );
    expect(res).toHaveFailedTx<AskToBurn>({
        from: outsider.address,
        to: wallet.address,
        exitCode: Errors.NotOwner,
    });
}

get fun `test mutation: direct wallet should reject overburning jettons`() {
    val owner = testing.treasury("direct-wallet-overburn-owner");
    val minter = testing.treasury("direct-wallet-overburn-minter");
    val wallet = deployWalletFromStorage(
        WalletStorage {
            jettonBalance: grams("1"),
            ownerAddress: owner.address,
            minterAddress: minter.address,
        },
        owner.address,
    );

    val res = wallet.sendBurn(
        owner.address,
        DEFAULT_BURN_MSG_VALUE,
        grams("5"),
        owner.address,
        null,
    );
    expect(res).toHaveFailedTx<AskToBurn>({
        from: owner.address,
        to: wallet.address,
        exitCode: Errors.BalanceError,
    });
    expect(wallet.getJettonBalance()).toEqual(grams("1"));
}

get fun `test mutation: direct wallet bounced transfer should restore balance persistently`() {
    val owner = testing.treasury("direct-wallet-bounce-owner");
    val minter = testing.treasury("direct-wallet-bounce-minter");
    val recipientOwner = testing.treasury("direct-wallet-bounce-recipient");
    val senderWallet = deployWalletFromStorage(
        WalletStorage {
            jettonBalance: grams("3"),
            ownerAddress: owner.address,
            minterAddress: minter.address,
        },
        owner.address,
    );
    val receiverWallet = deployWalletFromStorage(
        WalletStorage {
            jettonBalance: 0,
            ownerAddress: recipientOwner.address,
            minterAddress: minter.address,
        },
        recipientOwner.address,
    );
    val forwardPayload = makeTinyForwardPayload();

    val transferMsg = createMessage({
        bounce: true,
        value: DEFAULT_TRANSFER_MSG_VALUE,
        dest: senderWallet.address,
        body: AskToTransfer {
            queryId: 0,
            jettonAmount: grams("2"),
            transferRecipient: recipientOwner.address,
            sendExcessesTo: owner.address,
            customPayload: null,
            forwardTonAmount: 0,
            forwardPayload,
        },
    });
    val res = testing.processSingleTraceStep(owner.address, transferMsg);
    expect(senderWallet.getJettonBalance()).toEqual(grams("1"));

    val outMsg = res.outMessages.at<InternalTransferStep>(0);
    val outBody = outMsg.loadBody();
    val bouncedBody = beginCell()
        .storeUint(0xffffffff, 32)
        .storeSlice(outBody.toCell().beginParse())
        .endCell();
    val bouncedMsg = createMessage({
        bounce: false,
        value: grams("0.95"),
        dest: senderWallet.address,
        body: bouncedBody,
    }).bounced();
    testing.processSingleTraceStep(receiverWallet.address, bouncedMsg);

    expect(senderWallet.getJettonBalance()).toEqual(grams("3"));
}

get fun `test mutation: direct wallet transfer minimal value edge`() {
    val (wallet, owner, _, _) = setupDirectWallet(DEFAULT_MINTED_JETTONS);
    val dest = randomAddress();
    val body = buildTransferBody(TEST_JETTON_UNIT, 0, dest, dest, makeTinyForwardPayload());

    val fwdFee = estimateBodyForwardFee(body);
    val minimalTonValue = calcMinimalTransferAmount(0, fwdFee) + MIN_EDGE_DELTA;

    val enough = net.send(
        owner.address,
        createMessage({
            body,
            bounce: false,
            dest: wallet.address,
            value: minimalTonValue,
        }),
    );
    expect(enough).toHaveSuccessfulTx<AskToTransfer>({ from: owner.address, to: wallet.address });

    val fail = net.send(
        owner.address,
        createMessage({
            body,
            bounce: false,
            dest: wallet.address,
            value: minimalTonValue - MIN_EDGE_DELTA,
        }),
    );
    expect(fail).toHaveFailedTx<AskToTransfer>({
        from: owner.address,
        to: wallet.address,
        exitCode: Errors.NotEnoughGas,
    });
}

get fun `test mutation: direct wallet transfer with forward amount requires second fee`() {
    val (wallet, owner, _, _) = setupDirectWallet(DEFAULT_MINTED_JETTONS);
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

    val missingFee = net.send(
        owner.address,
        createMessage({
            body,
            bounce: false,
            dest: wallet.address,
            value: minMissingSecondFee,
        }),
    );
    expect(missingFee).toHaveFailedTx<AskToTransfer>({
        from: owner.address,
        to: wallet.address,
        exitCode: Errors.NotEnoughGas,
    });

    val enough = net.send(
        owner.address,
        createMessage({
            body,
            bounce: false,
            dest: wallet.address,
            value: minWithForward,
        }),
    );
    expect(enough).toHaveSuccessfulTx<AskToTransfer>({ from: owner.address, to: wallet.address });
}

get fun `test mutation: direct wallet transfer gas and storage fees should follow config`() {
    val (wallet, owner, _, _) = setupDirectWallet(DEFAULT_MINTED_JETTONS);
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

    val enough = net.send(
        owner.address,
        createMessage({
            body,
            bounce: false,
            dest: wallet.address,
            value: minimalAfter,
        }),
    );
    expect(enough).toHaveSuccessfulTx<AskToTransfer>({ from: owner.address, to: wallet.address });

    val fail = net.send(
        owner.address,
        createMessage({
            body,
            bounce: false,
            dest: wallet.address,
            value: minimalBefore,
        }),
    );
    expect(fail).toHaveFailedTx<AskToTransfer>({
        from: owner.address,
        to: wallet.address,
        exitCode: Errors.NotEnoughGas,
    });
}

get fun `test mutation: direct wallet burn minimal value edge`() {
    val (wallet, owner, _, _) = setupDirectWallet(DEFAULT_MINTED_JETTONS);
    val minimalTonValue = calcMinimalBurnAmount() + MIN_EDGE_DELTA;

    val enough = wallet.sendBurn(
        owner.address,
        minimalTonValue,
        TEST_JETTON_UNIT,
        owner.address,
        null,
    );
    expect(enough).toHaveSuccessfulTx<AskToBurn>({ from: owner.address, to: wallet.address });

    val fail = wallet.sendBurn(
        owner.address,
        minimalTonValue - MIN_EDGE_DELTA,
        TEST_JETTON_UNIT,
        owner.address,
        null,
    );
    expect(fail).toHaveFailedTx<AskToBurn>({
        from: owner.address,
        to: wallet.address,
        exitCode: Errors.NotEnoughGas,
    });
}

get fun `test mutation: direct wallet burn fees should follow config`() {
    val (wallet, owner, _, _) = setupDirectWallet(DEFAULT_MINTED_JETTONS);
    val minimalBefore = calcMinimalBurnAmount() + MIN_EDGE_DELTA;

    var config = testing.getConfig();
    var msgPrices = config.getMsgForwardPrices(BASECHAIN);
    msgPrices.bitPrice *= CONFIG_SCALE_X10;
    msgPrices.cellPrice *= CONFIG_SCALE_X10;
    config.setMsgForwardPrices(msgPrices, BASECHAIN);

    var gasPrices = config.getGasPrices(BASECHAIN);
    var gasPricesExtended = gasPrices.other;
    gasPricesExtended.gasPrice *= GAS_SCALE_X3;
    gasPrices.other = gasPricesExtended;
    config.setGasPrices(gasPrices, BASECHAIN);
    expect(testing.setConfig(config)).toEqual(true);

    val minimalAfter = calcMinimalBurnAmount() + MIN_EDGE_DELTA;
    expect(minimalBefore).toBeLess(minimalAfter);

    val enough = wallet.sendBurn(
        owner.address,
        minimalAfter,
        TEST_JETTON_UNIT,
        owner.address,
        null,
    );
    expect(enough).toHaveSuccessfulTx<AskToBurn>({ from: owner.address, to: wallet.address });

    val fail = wallet.sendBurn(
        owner.address,
        minimalAfter - MIN_EDGE_DELTA,
        TEST_JETTON_UNIT,
        owner.address,
        null,
    );
    expect(fail).toHaveFailedTx<AskToBurn>({
        from: owner.address,
        to: wallet.address,
        exitCode: Errors.NotEnoughGas,
    });
}

get fun `test mutation: minter should report mintable and emit set-code action on upgrade`() {
    val (minter, deployer, _) = setupTest();
    expect(minter.getJettonData().mintable).toEqual(true);

    val upgradedSupply = grams("42");
    val upgradedUri = "https://upgraded.meta.example/jetton.json";
    val newData = MinterStorage {
        totalSupply: upgradedSupply,
        adminAddress: deployer.address,
        nextAdminAddress: null,
        metadata: buildTestMetadata(upgradedUri),
    }.toCell();
    val newCode = build("JettonMinter");

    val res = minter.sendUpgrade(deployer.address, newCode, newData);
    expect(res).toHaveSuccessfulTx<UpgradeMinterCode>({
        from: deployer.address,
        to: minter.address,
    });
    expect(hasSetCodeAction(res, minter.address)).toEqual(true);
}

get fun `test mutation: minter unknown opcode should fail`() {
    val (minter, deployer, _) = setupTest();

    val res = net.send(
        deployer.address,
        createMessage({
            bounce: true,
            value: grams("0.1"),
            dest: minter.address,
            body: beginCell().storeUint(0x7fffffff, 32).storeUint(1, 64).endCell(),
        }),
    );
    expect(res).toHaveFailedTx({
        from: deployer.address,
        to: minter.address,
        exitCode: 0xFFFF,
    });
}

get fun `test mutation: minter should reject masterchain mint recipients`() {
    val (minter, deployer, _) = setupTest();
    val masterAddr = address.fromWorkchainAndHash(-1, 0);
    val supplyBefore = minter.getTotalSupply();

    val res = minter.sendMint(
        deployer.address,
        masterAddr,
        grams("1"),
        DEFAULT_FORWARD_TON_AMOUNT,
        grams("0.3"),
    );
    expect(res).toHaveFailedTx<MintNewJettons>({
        from: deployer.address,
        to: minter.address,
        exitCode: Errors.WrongWorkchain,
    });
    expect(minter.getTotalSupply()).toEqual(supplyBefore);
}

get fun `test mutation: minter should reject malformed mint forward payload`() {
    val (minter, deployer, _) = setupTest();

    val badInternalTransfer = InternalTransferStep {
        queryId: 0,
        jettonAmount: grams("1"),
        transferInitiator: null,
        sendExcessesTo: minter.address,
        forwardTonAmount: 0,
        forwardPayload: makeBadEitherPayload(),
    }.toCell();

    val res = net.send(
        deployer.address,
        createMessage({
            bounce: false,
            value: grams("0.4"),
            dest: minter.address,
            body: MintNewJettons {
                queryId: 0,
                mintRecipient: deployer.address,
                tonAmount: grams("0.3"),
                internalTransferMsg: badInternalTransfer,
            },
        }),
    );
    expect(res).toHaveFailedTx<MintNewJettons>({
        from: deployer.address,
        to: minter.address,
        exitCode: 9,
    });
}

get fun `test mutation: minter discovery and burn paths should use expected send modes`() {
    val (minter, deployer, _) = setupTest();
    val wallet = mintTo(minter, deployer, deployer.address, grams("2"));

    val discovery = minter.sendDiscovery(deployer.address, deployer.address, true);
    expectSendActionMode(
        discovery,
        minter.address,
        0,
        SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE + SEND_MODE_BOUNCE_ON_ACTION_FAIL,
    );

    val burn = wallet.sendBurn(
        deployer.address,
        DEFAULT_BURN_MSG_VALUE,
        grams("1"),
        deployer.address,
        null,
    );
    expectSendActionMode(
        burn,
        minter.address,
        0,
        SEND_MODE_IGNORE_ERRORS + SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE,
    );
}

get fun `test mutation: minter mint minimal value edge and deploy mode`() {
    val (minter, deployer, _) = setupTest();
    val recipient = testing.treasury("minter-mutation-recipient");
    val forwardPayload = makeTinyForwardPayload();
    val minimalUpperBound = estimateMinimalMintTonAmount(
        minter.address,
        recipient.address,
        0,
        forwardPayload,
    );
    val minimalTonAmount = findExactExpectedMintTonAmount(
        minter.address,
        recipient.address,
        0,
        minimalUpperBound,
        forwardPayload,
    );

    val enough = minter.sendMint(
        deployer.address,
        recipient.address,
        TEST_JETTON_UNIT,
        0,
        minimalTonAmount,
    );
    expect(enough).toHaveSuccessfulTx<MintNewJettons>({
        from: deployer.address,
        to: minter.address,
    });
    expectSendActionMode(
        enough,
        minter.address,
        0,
        SEND_MODE_PAY_FEES_SEPARATELY + SEND_MODE_BOUNCE_ON_ACTION_FAIL,
    );

    val minterTx = findSendResultForAccount(enough, minter.address);
    expect(minterTx).toBeNotNull();
    if (minterTx == null) {
        return;
    }
    expect(minterTx.allOutActions().at(1).kind()).toEqual("reserve-currency");

    val fail = minter.sendMint(
        deployer.address,
        recipient.address,
        TEST_JETTON_UNIT,
        0,
        minimalTonAmount - MIN_EDGE_DELTA,
    );
    expect(fail).toHaveFailedTx<MintNewJettons>({
        from: deployer.address,
        to: minter.address,
        exitCode: Errors.NotEnoughGas,
    });
}

get fun `test mutation: minter mint fees should follow forward gas and storage config`() {
    val (minter, deployer, _) = setupTest();
    val recipient = testing.treasury("minter-fee-recipient");
    val customPayload = makeRichCustomPayload();
    val forwardPayload = beginCell().storeMaybeRef(customPayload).endCell().beginParse();
    val minimalBeforeUpperBound = estimateMinimalMintTonAmount(
        minter.address,
        recipient.address,
        DEFAULT_FORWARD_TON_AMOUNT,
        forwardPayload,
    );
    val minimalBefore = findExactExpectedMintTonAmount(
        minter.address,
        recipient.address,
        DEFAULT_FORWARD_TON_AMOUNT,
        minimalBeforeUpperBound,
        forwardPayload,
    );

    var config = testing.getConfig();
    var msgPrices = config.getMsgForwardPrices(BASECHAIN);
    msgPrices.bitPrice *= CONFIG_SCALE_X10;
    msgPrices.cellPrice *= CONFIG_SCALE_X10;
    config.setMsgForwardPrices(msgPrices, BASECHAIN);

    var gasPrices = config.getGasPrices(BASECHAIN);
    var gasPricesExtended = gasPrices.other;
    gasPricesExtended.gasPrice *= GAS_SCALE_X3;
    gasPrices.other = gasPricesExtended;
    config.setGasPrices(gasPrices, BASECHAIN);

    var storagePrices = config.getStoragePrices();
    var initialStoragePrices = storagePrices.getInitial();
    initialStoragePrices.bitPrice *= CONFIG_SCALE_X10;
    initialStoragePrices.cellPrice *= CONFIG_SCALE_X10;
    storagePrices.setInitial(initialStoragePrices);
    config.setStoragePrices(storagePrices);
    expect(testing.setConfig(config)).toEqual(true);

    val minimalAfterUpperBound = estimateMinimalMintTonAmount(
        minter.address,
        recipient.address,
        DEFAULT_FORWARD_TON_AMOUNT,
        forwardPayload,
    );
    expect(minimalBefore).toBeLess(minimalAfterUpperBound);

    val enough = minter.sendMint(
        deployer.address,
        recipient.address,
        TEST_JETTON_UNIT,
        DEFAULT_FORWARD_TON_AMOUNT,
        minimalAfterUpperBound + grams("0.01"),
        customPayload,
    );
    expect(enough).toHaveSuccessfulTx<MintNewJettons>({
        from: deployer.address,
        to: minter.address,
    });

    val fail = minter.sendMint(
        deployer.address,
        recipient.address,
        TEST_JETTON_UNIT,
        DEFAULT_FORWARD_TON_AMOUNT,
        minimalBefore,
        customPayload,
    );
    expect(fail).toHaveFailedTx<MintNewJettons>({
        from: deployer.address,
        to: minter.address,
        exitCode: Errors.NotEnoughGas,
    });
}
