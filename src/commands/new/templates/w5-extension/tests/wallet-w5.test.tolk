import "@acton/crypto"
import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"
import "@acton/types/big_array"
import "@wrappers/WalletV5.gen"
import "@wrappers/utils"
import "@contracts/walletv5/errors"
import "@contracts/walletv5/messages"
import "@contracts/walletv5/storage"

// ==================== Basic ====================

get fun `test w5: should receive basic transfer`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, _) = deployWalletWithKey(keyPair);

    val sender = testing.treasury("sender");

    // Simple value transfer (no body)
    val receipt1 = walletV5.sendAny(
        sender.address,
        createEmptyCell(),
        { value: grams("1"), bounce: false },
    );
    expect(receipt1).toHaveSuccessfulTx({
        from: sender.address,
        to: walletV5.address,
        exitCode: 0,
    });

    // Transfer with text comment
    val textBody: string = "Hey, bruh!";
    val receipt2 = walletV5.sendAny(
        sender.address,
        textBody.toCell(),
        { value: grams("1"), bounce: false },
    );
    expect(receipt2).toHaveSuccessfulTx({
        from: sender.address,
        to: walletV5.address,
        exitCode: 0,
    });

    // Transfer with random non-wallet opcode
    val randomBody = beginCell()
        .storeUint(0xdeadbeef, 32)
        .storeUint(blockchain.now(), 64)
        .endCell();
    val receipt3 = walletV5.sendAny(
        sender.address,
        randomBody,
        { value: grams("1"), bounce: false },
    );
    expect(receipt3).toHaveSuccessfulTx({
        from: sender.address,
        to: walletV5.address,
        exitCode: 0,
    });
}

// ==================== Actions: External ====================

get fun `test w5: ext should send up to 255 messages`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, _) = deployWalletWithKey(keyPair);

    val testReceiver = testing.treasury("receiver");

    var outActions: array<SendAction> = [];
    var i = 0;
    while (i < 255) {
        val innerMsg = createMessage({
            bounce: false,
            value: grams("0.001"),
            dest: testReceiver.address,
            body: beginCell().storeUint(i, 32).endCell(),
        });
        outActions.push(SendAction {
            mode: SEND_MODE_PAY_FEES_SEPARATELY | SEND_MODE_IGNORE_ERRORS,
            messageCell: innerMsg.messageCell,
        });
        i += 1;
    }

    val actions = ActionsList {
        outActions,
        extraActions: [],
    };
    val body = createExternalSignedBody(keyPair, DEFAULT_SUBWALLET_ID, 0, actions);

    val receipt = walletV5.sendExternalSigned(body).unwrap();

    // 1 wallet tx + 255 message txs
    expect(receipt.size()).toEqual(256);
}

get fun `test w5: ext should reject send modes without IGNORE_ERRORS`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, _) = deployWalletWithKey(keyPair);

    val testReceiver = randomAddress("test-receiver");

    // Send with mode=0 (no IGNORE_ERRORS) — should fail for external
    val innerMsg = createMessage({
        bounce: false,
        value: grams("0.001"),
        dest: testReceiver,
    });
    var outActions: array<SendAction> = [];
    outActions.push(SendAction {
        mode: SEND_MODE_PAY_FEES_SEPARATELY, // no IGNORE_ERRORS
        messageCell: innerMsg.messageCell,
    });

    val actions = ActionsList {
        outActions,
        extraActions: [],
    };
    val body = createExternalSignedBody(keyPair, DEFAULT_SUBWALLET_ID, 0, actions);

    // External message with mode without IGNORE_ERRORS should be rejected
    // The error happens after accept, so tx is included
    val receipt = walletV5.sendExternalSigned(body).unwrap();

    expect(receipt).toHaveTx({
        exitCode: Errors.ExternalSendMessageMustHaveIgnoreErrorsSendMode,
    });
}

get fun `test w5: ext empty action list should increase seqno`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, _) = deployWalletWithKey(keyPair);

    expect(walletV5.getSeqno()).toEqual(0);

    val actions = ActionsList {
        outActions: [],
        extraActions: [],
    };
    val body = createExternalSignedBody(keyPair, DEFAULT_SUBWALLET_ID, 0, actions);
    walletV5.sendExternalSigned(body);

    expect(walletV5.getSeqno()).toEqual(1);
}

get fun `test w5: ext should add remove send in one go`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, _) = deployWalletWithKey(keyPair);

    val testExtension = randomAddress("test-extension");
    val testReceiver = randomAddress("test-receiver");
    val forwardValue = grams("0.001");

    var outActions: array<SendAction> = [];
    outActions.push(sendMsgAction(testReceiver, forwardValue));

    var extraActions: array<ExtraAction> = [];
    extraActions.push(AddExtensionExtraAction { addr: testExtension });

    val actions = ActionsList { outActions, extraActions };
    val body = createExternalSignedBody(keyPair, DEFAULT_SUBWALLET_ID, 0, actions);

    val receipt = walletV5.sendExternalSigned(body).unwrap();

    expect(receipt).toHaveTx({
        from: walletV5.address,
        to: testReceiver,
        value: forwardValue,
    });

    val ext = walletV5.getExtensions();
    val (_, extHash) = testExtension.getWorkchainAndHash();
    expect(ext).toHaveLength(1);
    expect(ext.exists(extHash)).toEqual(true);
    expect(walletV5.getSeqno()).toEqual(1);
}

get fun `test w5: ext only_extension_can_change_signature_mode`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, _) = deployWalletWithKey(keyPair);

    var extraActions: array<ExtraAction> = [];
    extraActions.push(SetSignatureAllowedExtraAction { allowSignature: false });

    val actions = ActionsList {
        outActions: [],
        extraActions,
    };
    val body = createExternalSignedBody(keyPair, DEFAULT_SUBWALLET_ID, 0, actions);

    val receipt = walletV5.sendExternalSigned(body).unwrap();

    expect(receipt).toHaveTx({ exitCode: Errors.OnlyExtensionCanChangeSignatureMode });

    expect(walletV5.isSignatureAllowed()).toEqual(true);
}

get fun `test w5: ext should be able to send external-out message`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, _) = deployWalletWithKey(keyPair);

    val testPayload = 12345;
    val testBody = beginCell().storeUint(testPayload, 32).endCell();

    // Construct external address: addr_extern$01 + len(32) + payload
    val extAddrBuilder = beginCell().storeUint(1, 2).storeUint(32, 9).storeUint(testPayload, 32);

    val extOutMsg = createExternalLogMessage({ dest: extAddrBuilder, body: testBody });

    var outActions: array<SendAction> = [];
    outActions.push(SendAction {
        mode: SEND_MODE_REGULAR | SEND_MODE_IGNORE_ERRORS,
        messageCell: extOutMsg.messageCell,
    });

    val actions = ActionsList {
        outActions,
        extraActions: [],
    };
    val body = createExternalSignedBody(keyPair, DEFAULT_SUBWALLET_ID, 0, actions);

    val receipt = walletV5.sendExternalSigned(body).unwrap();

    expect(receipt).toHaveTx({ exitCode: 0 });

    expect(walletV5.getSeqno()).toEqual(1);
}

get fun `test w5: ext should not be able to install extension from different chain`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, _) = deployWalletWithKey(keyPair);

    // Create a masterchain address (workchain -1)
    val mcExtension = randomAddress("mc-extension", -1);

    // Fund the masterchain address so it exists
    val funder = testing.treasury("funder");
    val fundMsg = createMessage({
        bounce: false,
        value: grams("1"),
        dest: mcExtension,
    });
    net.send(funder.address, fundMsg);

    // Try to add masterchain extension to basechain wallet via external signed
    var extraActions: array<ExtraAction> = [];
    extraActions.push(AddExtensionExtraAction { addr: mcExtension });

    val actions = ActionsList {
        outActions: [],
        extraActions,
    };
    val body = createExternalSignedBody(keyPair, DEFAULT_SUBWALLET_ID, 0, actions);

    val receipt = walletV5.sendExternalSigned(body).unwrap();

    expect(receipt).toHaveTx({ exitCode: Errors.ExtensionWrongWorkchain });

    // Extension should NOT have been added
    val ext = walletV5.getExtensions();
    expect(ext).toHaveLength(0);
}

// ==================== Actions: Internal ====================

get fun `test w5: int should send up to 255 messages`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);

    val testReceiver = testing.treasury("receiver");

    var outActions: array<SendAction> = [];
    var i = 0;
    while (i < 255) {
        val innerMsg = createMessage({
            bounce: false,
            value: grams("0.001"),
            dest: testReceiver.address,
            body: beginCell().storeUint(i, 32).endCell(),
        });
        outActions.push(SendAction {
            mode: SEND_MODE_PAY_FEES_SEPARATELY,
            messageCell: innerMsg.messageCell,
        });
        i += 1;
    }

    val actions = ActionsList {
        outActions,
        extraActions: [],
    };
    val body = createSignedBody(keyPair, DEFAULT_SUBWALLET_ID, 0, actions);

    val receipt = walletV5.sendSignedBody(deployer.address, body, { value: grams("10") });

    // deployer->wallet tx + 255 out message txs
    expect(receipt).toHaveSuccessfulTx({
        from: deployer.address,
        to: walletV5.address,
        exitCode: 0,
    });

    expect(walletV5.getSeqno()).toEqual(1);
}

get fun `test w5: int should send messages with various send modes`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);
    var seqno = 0;

    val testReceiver = testing.treasury("receiver");

    // Internal should work with IGNORE_ERRORS
    var outActions1: array<SendAction> = [];
    val innerMsg1 = createMessage({
        bounce: false,
        value: grams("0.001"),
        dest: testReceiver.address,
    });
    outActions1.push(SendAction {
        mode: SEND_MODE_PAY_FEES_SEPARATELY | SEND_MODE_IGNORE_ERRORS,
        messageCell: innerMsg1.messageCell,
    });
    val body1 = createSignedBody(
        keyPair,
        DEFAULT_SUBWALLET_ID,
        seqno,
        ActionsList {
            outActions: outActions1,
            extraActions: [],
        },
    );
    val receipt1 = walletV5.sendSignedBody(deployer.address, body1, { value: grams("0.1") });
    expect(receipt1).toHaveSuccessfulTx({
        from: deployer.address,
        to: walletV5.address,
        exitCode: 0,
    });
    seqno += 1;

    // Internal should work WITHOUT IGNORE_ERRORS (unlike external)
    var outActions2: array<SendAction> = [];
    val innerMsg2 = createMessage({
        bounce: false,
        value: grams("0.001"),
        dest: testReceiver.address,
    });
    outActions2.push(SendAction {
        mode: SEND_MODE_PAY_FEES_SEPARATELY,
        messageCell: innerMsg2.messageCell,
    });
    val body2 = createSignedBody(
        keyPair,
        DEFAULT_SUBWALLET_ID,
        seqno,
        ActionsList {
            outActions: outActions2,
            extraActions: [],
        },
    );
    val receipt2 = walletV5.sendSignedBody(deployer.address, body2, { value: grams("0.1") });
    expect(receipt2).toHaveSuccessfulTx({
        from: deployer.address,
        to: walletV5.address,
        exitCode: 0,
    });
    seqno += 1;

    expect(walletV5.getSeqno()).toEqual(seqno);
}

get fun `test w5: int empty action list should increase seqno`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);

    expect(walletV5.getSeqno()).toEqual(0);

    val actions = ActionsList {
        outActions: [],
        extraActions: [],
    };
    val body = createSignedBody(keyPair, DEFAULT_SUBWALLET_ID, 0, actions);
    walletV5.sendSignedBody(deployer.address, body, { value: grams("0.1") });

    expect(walletV5.getSeqno()).toEqual(1);
}

get fun `test w5: int should add remove send in one go`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);

    val testExtension = randomAddress("test-extension");
    val testReceiver = randomAddress("test-receiver");
    val forwardValue = grams("0.001");

    var outActions: array<SendAction> = [];
    outActions.push(sendMsgAction(testReceiver, forwardValue));

    var extraActions: array<ExtraAction> = [];
    extraActions.push(AddExtensionExtraAction { addr: testExtension });

    val actions = ActionsList { outActions, extraActions };
    val body = createSignedBody(keyPair, DEFAULT_SUBWALLET_ID, 0, actions);

    val receipt = walletV5.sendSignedBody(deployer.address, body, { value: grams("0.1") });

    expect(receipt).toHaveTx({
        from: walletV5.address,
        to: testReceiver,
        value: forwardValue,
    });

    val ext = walletV5.getExtensions();
    val (_, extHash) = testExtension.getWorkchainAndHash();
    expect(ext).toHaveLength(1);
    expect(ext.exists(extHash)).toEqual(true);
    expect(walletV5.getSeqno()).toEqual(1);
}

get fun `test w5: int only_extension_can_change_signature_mode`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);

    var extraActions: array<ExtraAction> = [];
    extraActions.push(SetSignatureAllowedExtraAction { allowSignature: false });

    val actions = ActionsList {
        outActions: [],
        extraActions,
    };
    val body = createSignedBody(keyPair, DEFAULT_SUBWALLET_ID, 0, actions);

    val receipt = walletV5.sendSignedBody(deployer.address, body, { value: grams("0.1") });

    expect(receipt).toHaveTx({ exitCode: Errors.OnlyExtensionCanChangeSignatureMode });

    expect(walletV5.isSignatureAllowed()).toEqual(true);
}

// ==================== Actions: Extension ====================

get fun `test w5: ext_action should send up to 255 messages`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);
    var seqno = 0;

    // Add extension
    addExtensionSigned(walletV5, deployer.address, deployer.address, keyPair, seqno);
    seqno += 1;

    val testReceiver = testing.treasury("receiver");

    var outActions: array<SendAction> = [];
    var i = 0;
    while (i < 255) {
        val innerMsg = createMessage({
            bounce: false,
            value: grams("0.001"),
            dest: testReceiver.address,
            body: beginCell().storeUint(i, 32).endCell(),
        });
        outActions.push(SendAction {
            mode: SEND_MODE_PAY_FEES_SEPARATELY | SEND_MODE_IGNORE_ERRORS,
            messageCell: innerMsg.messageCell,
        });
        i += 1;
    }

    val actionsBody = ActionsList {
        outActions,
        extraActions: [],
    };

    val receipt = walletV5.sendExtensionActionBody(
        deployer.address,
        actionsBody,
        0,
        { value: grams("10") },
    );

    // extension->wallet tx + 255 out message txs
    expect(receipt).toHaveSuccessfulTx({
        from: deployer.address,
        to: walletV5.address,
        exitCode: 0,
    });
}

get fun `test w5: ext_action should send messages with various modes`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);
    var seqno = 0;

    // Add extension
    addExtensionSigned(walletV5, deployer.address, deployer.address, keyPair, seqno);
    seqno += 1;

    val testReceiver = testing.treasury("receiver");

    // Extension should work with IGNORE_ERRORS
    var outActions1: array<SendAction> = [];
    val innerMsg1 = createMessage({
        bounce: false,
        value: grams("0.001"),
        dest: testReceiver.address,
    });
    outActions1.push(SendAction {
        mode: SEND_MODE_PAY_FEES_SEPARATELY | SEND_MODE_IGNORE_ERRORS,
        messageCell: innerMsg1.messageCell,
    });
    val actionsBody1 = ActionsList {
        outActions: outActions1,
        extraActions: [],
    };
    val receipt1 = walletV5.sendExtensionActionBody(
        deployer.address,
        actionsBody1,
        0,
        { value: grams("0.1") },
    );
    expect(receipt1).toHaveSuccessfulTx({
        from: deployer.address,
        to: walletV5.address,
        exitCode: 0,
    });

    // Extension should work WITHOUT IGNORE_ERRORS (like internal)
    var outActions2: array<SendAction> = [];
    val innerMsg2 = createMessage({
        bounce: false,
        value: grams("0.001"),
        dest: testReceiver.address,
    });
    outActions2.push(SendAction {
        mode: SEND_MODE_PAY_FEES_SEPARATELY,
        messageCell: innerMsg2.messageCell,
    });
    val actionsBody2 = ActionsList {
        outActions: outActions2,
        extraActions: [],
    };
    val receipt2 = walletV5.sendExtensionActionBody(
        deployer.address,
        actionsBody2,
        0,
        { value: grams("0.1") },
    );
    expect(receipt2).toHaveSuccessfulTx({
        from: deployer.address,
        to: walletV5.address,
        exitCode: 0,
    });
}

get fun `test w5: ext_action from wrong workchain should be ignored`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);
    var seqno = 0;

    // Add deployer as extension
    addExtensionSigned(walletV5, deployer.address, deployer.address, keyPair, seqno);
    seqno += 1;

    // Create an address with same hash but different workchain (-1 = masterchain)
    val (_, deployerHash) = deployer.address.getWorkchainAndHash();
    val mcAddress = beginCell()
        .storeUint(4, 3)
        .storeInt(-1, 8)
        .storeUint(deployerHash, 256)
        .toSlice() as address;

    // Send extension action from masterchain address (same hash, different workchain)
    val testReceiver = randomAddress("test-receiver");
    val forwardValue = grams("0.001");

    var outActions: array<SendAction> = [];
    outActions.push(sendMsgAction(testReceiver, forwardValue));
    val actionsBody = ActionsList {
        outActions,
        extraActions: [],
    };

    val receipt = walletV5.sendExtensionActionBody(
        mcAddress,
        actionsBody,
        0,
        { value: grams("0.1") },
    );

    // Should be silently ignored — no transfer, no error
    expect(receipt).toHaveSuccessfulTx({
        from: mcAddress,
        to: walletV5.address,
        exitCode: 0,
    });
    expect(receipt).toNotHaveTx({
        from: walletV5.address,
        to: testReceiver,
        value: forwardValue,
    });
}

get fun `test w5: ext_action add remove send in one go`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);
    var seqno = 0;

    // Add extension
    addExtensionSigned(walletV5, deployer.address, deployer.address, keyPair, seqno);
    seqno += 1;

    val newExtension = randomAddress("new-extension");
    val testReceiver = randomAddress("test-receiver");
    val forwardValue = grams("0.001");

    var outActions: array<SendAction> = [];
    outActions.push(sendMsgAction(testReceiver, forwardValue));

    var extraActions: array<ExtraAction> = [];
    extraActions.push(AddExtensionExtraAction { addr: newExtension });

    val actionsBody = ActionsList { outActions, extraActions };

    val receipt = walletV5.sendExtensionActionBody(
        deployer.address,
        actionsBody,
        0,
        { value: grams("0.1") },
    );

    expect(receipt).toHaveTx({
        from: walletV5.address,
        to: testReceiver,
        value: forwardValue,
    });

    val ext = walletV5.getExtensions();
    val (_, newExtHash) = newExtension.getWorkchainAndHash();
    expect(ext).toHaveLength(2);
    expect(ext.exists(newExtHash)).toEqual(true);
    // Seqno should not change for extension actions
    expect(walletV5.getSeqno()).toEqual(seqno);
}

// ==================== Malformed action list ====================

get fun `test w5: action list exceeding 255 elements should be rejected`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);
    var seqno = 0;

    val testReceiver = testing.treasury("receiver");

    var outActions = BigArray<SendAction>.createEmpty();
    var i = 0;
    while (i < 256) {
        val innerMsg = createMessage({
            bounce: false,
            value: grams("0.001"),
            dest: testReceiver.address,
            body: beginCell().storeUint(i, 32).endCell(),
        });
        outActions.push(SendAction {
            mode: SEND_MODE_PAY_FEES_SEPARATELY | SEND_MODE_IGNORE_ERRORS,
            messageCell: innerMsg.messageCell,
        });
        i += 1;
    }

    // Build C5 action list manually since ActionsList expects array, not BigArray
    var c5 = createEmptyCell();
    var j = outActions.size() - 1;
    while (j >= 0) {
        val action = outActions.get(j);
        c5 = beginCell()
            .storeRef(c5)
            .storeUint(0x0ec3c86d, 32)
            .storeUint(action.mode, 8)
            .storeRef(action.messageCell)
            .endCell();
        j -= 1;
    }
    // Pack as: Maybe<ref(TlbOutActionsList)>=1 ref(c5) hasExtraActions=0
    val bigActionsCell = beginCell().storeUint(1, 1).storeRef(c5).storeUint(0, 1).endCell();

    // Via external: error after accept, tx included
    val extPayloadCell = beginCell()
        .storeUint(0x7369676E, 32)
        .storeUint(DEFAULT_SUBWALLET_ID, 32)
        .storeUint(blockchain.now() + 1000, 32)
        .storeUint(seqno, 32)
        .storeSlice(bigActionsCell.beginParse())
        .endCell();
    val extSignature = crypto.sign(keyPair.privateKey, extPayloadCell);
    val extRawBody = beginCell()
        .storeSlice(extPayloadCell.beginParse())
        .storeSlice(extSignature)
        .endCell();
    val extReceipt = walletV5.sendExternalRaw(extRawBody);
    expect(extReceipt).toBeAccepted();
    expect(extReceipt.unwrap()).toHaveTx({ exitCode: Errors.InvalidC5 });
    seqno += 1;

    // Via internal: should bounce
    val intPayloadCell = beginCell()
        .storeUint(0x73696E74, 32)
        .storeUint(DEFAULT_SUBWALLET_ID, 32)
        .storeUint(blockchain.now() + 1000, 32)
        .storeUint(seqno, 32)
        .storeSlice(bigActionsCell.beginParse())
        .endCell();
    val intSignature = crypto.sign(keyPair.privateKey, intPayloadCell);
    val intRawBody = beginCell()
        .storeSlice(intPayloadCell.beginParse())
        .storeSlice(intSignature)
        .endCell();
    val intReceipt = walletV5.sendAny(deployer.address, intRawBody, { value: grams("10") });
    expect(intReceipt).toHaveTx({ exitCode: Errors.InvalidC5 });

    // Via extension
    addExtensionSigned(walletV5, deployer.address, deployer.address, keyPair, seqno);
    seqno += 1;

    val extActBody = bigActionsCell;
    val extActReceipt = walletV5.sendExtensionActionRaw(
        deployer.address,
        extActBody,
        0,
        { value: grams("10") },
    );
    expect(extActReceipt).toHaveTx({ exitCode: Errors.InvalidC5 });
}

// ==================== Signature auth ====================

get fun `test w5: extension should be able to set signature mode`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);
    var seqno = 0;

    // Add extension
    addExtensionSigned(walletV5, deployer.address, deployer.address, keyPair, seqno);
    seqno += 1;

    val seqnoBefore = walletV5.getSeqno();
    expect(walletV5.isSignatureAllowed()).toEqual(true);

    // Disable sig auth via extension
    var extraActions1: array<ExtraAction> = [];
    extraActions1.push(SetSignatureAllowedExtraAction { allowSignature: false });
    val actionsBody1 = ActionsList {
        outActions: [],
        extraActions: extraActions1,
    };

    val receipt1 = walletV5.sendExtensionActionBody(
        deployer.address,
        actionsBody1,
        0,
        { value: grams("0.1") },
    );
    expect(receipt1).toHaveSuccessfulTx({
        from: deployer.address,
        to: walletV5.address,
        exitCode: 0,
    });
    expect(walletV5.isSignatureAllowed()).toEqual(false);
    expect(walletV5.getSeqno()).toEqual(seqnoBefore);

    // Re-enable sig auth via extension
    var extraActions2: array<ExtraAction> = [];
    extraActions2.push(SetSignatureAllowedExtraAction { allowSignature: true });
    val actionsBody2 = ActionsList {
        outActions: [],
        extraActions: extraActions2,
    };

    val receipt2 = walletV5.sendExtensionActionBody(
        deployer.address,
        actionsBody2,
        0,
        { value: grams("0.1") },
    );
    expect(receipt2).toHaveSuccessfulTx({
        from: deployer.address,
        to: walletV5.address,
        exitCode: 0,
    });
    expect(walletV5.isSignatureAllowed()).toEqual(true);
    expect(walletV5.getSeqno()).toEqual(seqnoBefore);
}

get fun `test w5: should reject sig auth change via internal signed request`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);

    var extraActions: array<ExtraAction> = [];
    extraActions.push(SetSignatureAllowedExtraAction { allowSignature: false });

    val actions = ActionsList {
        outActions: [],
        extraActions,
    };
    val body = createSignedBody(keyPair, DEFAULT_SUBWALLET_ID, 0, actions);

    val receipt = walletV5.sendSignedBody(deployer.address, body, { value: grams("0.1") });

    expect(receipt).toHaveTx({ exitCode: Errors.OnlyExtensionCanChangeSignatureMode });

    expect(walletV5.isSignatureAllowed()).toEqual(true);
}

get fun `test w5: should not accept signed external when sig auth is disabled`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);
    var seqno = 0;

    // Add extension and disable sig auth
    addExtensionSigned(walletV5, deployer.address, deployer.address, keyPair, seqno);
    seqno += 1;

    var extraActions: array<ExtraAction> = [];
    extraActions.push(SetSignatureAllowedExtraAction { allowSignature: false });
    val actionsBody = ActionsList {
        outActions: [],
        extraActions,
    };
    walletV5.sendExtensionActionBody(deployer.address, actionsBody, 0, { value: grams("0.1") });

    expect(walletV5.isSignatureAllowed()).toEqual(false);

    // Try signed external — should be rejected before accept
    val testReceiver = randomAddress("test-receiver");
    var outActions: array<SendAction> = [];
    outActions.push(sendMsgAction(testReceiver, grams("0.001")));
    val actions = ActionsList {
        outActions,
        extraActions: [],
    };
    val extBody = createExternalSignedBody(keyPair, DEFAULT_SUBWALLET_ID, seqno, actions);

    // External with disabled sig should be rejected (no tx)
    val receipt = walletV5.sendExternalSigned(extBody);
    expect(receipt).toBeNotAccepted();
    expect(walletV5.getSeqno()).toEqual(seqno);
}

get fun `test w5: should not accept signed internal when sig auth is disabled`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);
    var seqno = 0;

    // Add extension and disable sig auth
    addExtensionSigned(walletV5, deployer.address, deployer.address, keyPair, seqno);
    seqno += 1;

    var extraActions: array<ExtraAction> = [];
    extraActions.push(SetSignatureAllowedExtraAction { allowSignature: false });
    val actionsBody = ActionsList {
        outActions: [],
        extraActions,
    };
    walletV5.sendExtensionActionBody(deployer.address, actionsBody, 0, { value: grams("0.1") });

    expect(walletV5.isSignatureAllowed()).toEqual(false);

    // Try signed internal — should error
    val testReceiver = randomAddress("test-receiver");
    var outActions: array<SendAction> = [];
    outActions.push(sendMsgAction(testReceiver, grams("0.001")));
    val actions = ActionsList {
        outActions,
        extraActions: [],
    };
    val intBody = createSignedBody(keyPair, DEFAULT_SUBWALLET_ID, seqno, actions);

    val receipt = walletV5.sendSignedBody(deployer.address, intBody, { value: grams("0.1") });

    expect(receipt).toHaveTx({ exitCode: Errors.SignatureDisabled });
}

get fun `test w5: extension should add another ext when sig auth is disabled`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);
    var seqno = 0;

    // Add extension and disable sig auth
    addExtensionSigned(walletV5, deployer.address, deployer.address, keyPair, seqno);
    seqno += 1;

    var extraActions0: array<ExtraAction> = [];
    extraActions0.push(SetSignatureAllowedExtraAction { allowSignature: false });
    val actionsBody0 = ActionsList {
        outActions: [],
        extraActions: extraActions0,
    };
    walletV5.sendExtensionActionBody(deployer.address, actionsBody0, 0, { value: grams("0.1") });

    expect(walletV5.isSignatureAllowed()).toEqual(false);

    // Add another extension
    val newExtension = randomAddress("new-extension");
    var extraActions1: array<ExtraAction> = [];
    extraActions1.push(AddExtensionExtraAction { addr: newExtension });
    val actionsBody1 = ActionsList {
        outActions: [],
        extraActions: extraActions1,
    };

    val receipt = walletV5.sendExtensionActionBody(
        deployer.address,
        actionsBody1,
        0,
        { value: grams("0.1") },
    );

    expect(receipt).toHaveSuccessfulTx({
        from: deployer.address,
        to: walletV5.address,
        exitCode: 0,
    });

    val ext = walletV5.getExtensions();
    val (_, newExtHash) = newExtension.getWorkchainAndHash();
    expect(ext).toHaveLength(2);
    expect(ext.exists(newExtHash)).toEqual(true);
}

get fun `test w5: should remove ext if sig disabled and at least one left`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);
    var seqno = 0;

    // Add extension
    addExtensionSigned(walletV5, deployer.address, deployer.address, keyPair, seqno);
    seqno += 1;

    // Disable sig auth and add another extension
    val anotherExtension = randomAddress("another-ext");
    var extraActions0: array<ExtraAction> = [];
    extraActions0.push(SetSignatureAllowedExtraAction { allowSignature: false });
    extraActions0.push(AddExtensionExtraAction { addr: anotherExtension });
    val actionsBody0 = ActionsList {
        outActions: [],
        extraActions: extraActions0,
    };
    walletV5.sendExtensionActionBody(deployer.address, actionsBody0, 0, { value: grams("0.1") });

    expect(walletV5.isSignatureAllowed()).toEqual(false);
    val ext1 = walletV5.getExtensions();
    expect(ext1).toHaveLength(2);

    // Remove one extension (not the last one) — should succeed
    var extraActions1: array<ExtraAction> = [];
    extraActions1.push(RemoveExtensionExtraAction { addr: anotherExtension });
    val actionsBody1 = ActionsList {
        outActions: [],
        extraActions: extraActions1,
    };

    val receipt = walletV5.sendExtensionActionBody(
        deployer.address,
        actionsBody1,
        0,
        { value: grams("0.1") },
    );

    expect(receipt).toHaveSuccessfulTx({
        from: deployer.address,
        to: walletV5.address,
        exitCode: 0,
    });

    val ext2 = walletV5.getExtensions();
    expect(ext2).toHaveLength(1);
    val (_, anotherHash) = anotherExtension.getWorkchainAndHash();
    expect(ext2.exists(anotherHash)).toEqual(false);
}

get fun `test w5: withdraw funds on deploy with sig auth disabled`() {
    // Deploy wallet with sig auth disabled (incorrect deployment scenario)
    val keyPair = crypto.generateKeyPair();

    val storage = Storage {
        isSignatureAllowed: false,
        seqno: 0,
        subwalletId: DEFAULT_SUBWALLET_ID,
        publicKey: keyPair.publicKey,
        extensions: [],
    };

    val (walletV5, deployer) = deployWallet(storage);

    expect(walletV5.isSignatureAllowed()).toEqual(false);

    // v5r1: when extensions dict is empty, signed messages are allowed even with sig_allowed=false
    // (safety net for accidental misconfigured deploy)
    val testReceiver = randomAddress("receiver");
    var outActions: array<SendAction> = [];
    outActions.push(sendMsgAction(testReceiver, grams("0.001")));
    val actions = ActionsList {
        outActions,
        extraActions: [],
    };
    var seqno = 0;

    // Via internal signed
    val intBody = createSignedBody(keyPair, DEFAULT_SUBWALLET_ID, seqno, actions);
    val intReceipt = walletV5.sendSignedBody(deployer.address, intBody, { value: grams("0.1") });
    expect(intReceipt).toHaveSuccessfulTx({
        from: deployer.address,
        to: walletV5.address,
        exitCode: 0,
    });
    expect(intReceipt).toHaveTx({ from: walletV5.address, to: testReceiver });
    seqno += 1;

    // Via external signed
    val extBody = createExternalSignedBody(keyPair, DEFAULT_SUBWALLET_ID, seqno, actions);
    val extReceipt = walletV5.sendExternalSigned(extBody).unwrap();
    expect(extReceipt).toHaveTx({ exitCode: 0 });
    seqno += 1;

    expect(walletV5.getSeqno()).toEqual(seqno);
}

// ==================== Malformed action: extra data/refs ====================

// Helper: build a raw signed request cell from a raw actions cell
fun buildRawExternalRequest(
    keyPair: KeyPair,
    subwalletId: int,
    seqno: int,
    rawActionsCell: cell,
): cell {
    val payloadCell = beginCell()
        .storeUint(0x7369676E, 32)
        .storeUint(subwalletId, 32)
        .storeUint(blockchain.now() + 1000, 32)
        .storeUint(seqno, 32)
        .storeSlice(rawActionsCell.beginParse())
        .endCell();
    val signature = crypto.sign(keyPair.privateKey, payloadCell);
    return beginCell().storeSlice(payloadCell.beginParse()).storeSlice(signature).endCell();
}

fun buildRawInternalRequest(
    keyPair: KeyPair,
    subwalletId: int,
    seqno: int,
    rawActionsCell: cell,
): cell {
    val payloadCell = beginCell()
        .storeUint(0x73696E74, 32)
        .storeUint(subwalletId, 32)
        .storeUint(blockchain.now() + 1000, 32)
        .storeUint(seqno, 32)
        .storeSlice(rawActionsCell.beginParse())
        .endCell();
    val signature = crypto.sign(keyPair.privateKey, payloadCell);
    return beginCell().storeSlice(payloadCell.beginParse()).storeSlice(signature).endCell();
}

get fun `test w5: should reject action list with extra data or refs`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);
    var seqno = 0;

    val testReceiver = randomAddress("test-receiver");
    val innerMsg = createMessage({
        bounce: false,
        value: grams("0.001"),
        dest: testReceiver,
    });

    // Valid action node: ref(prev) | 0x0ec3c86d | mode:8 | ref(message)
    // As a standalone (no prev ref): 0x0ec3c86d | mode:8 | ref(message)
    val validActionBuilder = beginCell()
        .storeUint(0x0ec3c86d, 32)
        .storeUint(SEND_MODE_PAY_FEES_SEPARATELY, 8)
        .storeRef(innerMsg.messageCell);
    val validActionCell = validActionBuilder.endCell();

    // noRef: action data without the message ref
    var noRefSlice = validActionCell.beginParse();
    noRefSlice.loadRef(); // consume (skip) the ref
    val noRef = beginCell().storeSlice(noRefSlice).endCell();

    // excessiveData: valid action + 1 extra bit
    val excessiveData = beginCell()
        .storeSlice(validActionCell.beginParse())
        .storeUint(1, 1)
        .endCell();

    // truncated: valid action minus 1 bit
    var truncSlice = validActionCell.beginParse();
    val totalBits = truncSlice.remainingBitsCount();
    val truncated = beginCell().storeSlice(truncSlice.loadBits(totalBits - 1)).endCell();

    // extraRef: valid action + extra ref
    val extraRef = beginCell()
        .storeSlice(validActionCell.beginParse())
        .storeRef(beginCell().storeUint(0x0ec3c86d, 32).endCell())
        .endCell();

    val malformedPayloads: array<cell> = [excessiveData, truncated, extraRef, noRef];

    var i = 0;
    while (i < malformedPayloads.size()) {
        val payload = malformedPayloads.get(i);

        // Wrap as C5 action list node: ref(empty_prev) | payload_slice
        val actionList = beginCell()
            .storeRef(createEmptyCell())
            .storeSlice(payload.beginParse())
            .endCell();

        // Pack as: hasTlbOutActions=1 ref(actionList) hasExtraActions=0
        val rawActionsCell = beginCell()
            .storeUint(1, 1)
            .storeRef(actionList)
            .storeUint(0, 1)
            .endCell();

        // Via internal
        val intBody = buildRawInternalRequest(keyPair, DEFAULT_SUBWALLET_ID, seqno, rawActionsCell);
        val intReceipt = walletV5.sendAny(deployer.address, intBody, { value: grams("1") });
        expect(intReceipt).toHaveTx({ exitCode: Errors.InvalidC5 });

        // Via external (error after accept, tx included, seqno increments)
        val extBody = buildRawExternalRequest(keyPair, DEFAULT_SUBWALLET_ID, seqno, rawActionsCell);
        val extReceipt = walletV5.sendExternalRaw(extBody);
        expect(extReceipt).toBeAccepted();
        expect(extReceipt.unwrap()).toHaveTx({ exitCode: Errors.InvalidC5 });
        seqno += 1;

        i += 1;
    }

    // Add extension and test via extension too
    addExtensionSigned(walletV5, deployer.address, deployer.address, keyPair, seqno);
    seqno += 1;

    i = 0;
    while (i < malformedPayloads.size()) {
        val payload = malformedPayloads.get(i);

        val actionList = beginCell()
            .storeRef(createEmptyCell())
            .storeSlice(payload.beginParse())
            .endCell();

        val rawActionsCell = beginCell()
            .storeUint(1, 1)
            .storeRef(actionList)
            .storeUint(0, 1)
            .endCell();

        val extActReceipt = walletV5.sendExtensionActionRaw(
            deployer.address,
            rawActionsCell,
            0,
            { value: grams("1") },
        );
        expect(extActReceipt).toHaveTx({ exitCode: Errors.InvalidC5 });

        i += 1;
    }

    // Verify valid action still works
    val validC5 = beginCell()
        .storeRef(createEmptyCell())
        .storeSlice(validActionCell.beginParse())
        .endCell();
    val validRawActions = beginCell().storeUint(1, 1).storeRef(validC5).storeUint(0, 1).endCell();
    val validIntBody = buildRawInternalRequest(
        keyPair,
        DEFAULT_SUBWALLET_ID,
        seqno,
        validRawActions,
    );
    val validReceipt = walletV5.sendAny(deployer.address, validIntBody, { value: grams("1") });
    expect(validReceipt).toHaveTx({
        from: walletV5.address,
        to: testReceiver,
        value: grams("0.001"),
    });
}

// ==================== Send with init state ====================

get fun `test w5: ext should be able to send message with init state`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, _) = deployWalletWithKey(keyPair);

    // Create a new wallet with a different subwallet ID
    val newStorage = Storage {
        isSignatureAllowed: true,
        seqno: 0,
        subwalletId: 42,
        publicKey: keyPair.publicKey,
        extensions: [],
    };
    val newWallet = WalletV5.fromLibraryStorage(newStorage);

    // Send a message with init state to deploy the new wallet
    val deployMsg = createMessage({
        bounce: false,
        value: grams("1"),
        dest: { stateInit: newWallet.stateInit! },
    });

    var outActions: array<SendAction> = [];
    outActions.push(SendAction {
        mode: SEND_MODE_PAY_FEES_SEPARATELY | SEND_MODE_IGNORE_ERRORS,
        messageCell: deployMsg.messageCell,
    });

    val actions = ActionsList {
        outActions,
        extraActions: [],
    };
    val body = createExternalSignedBody(keyPair, DEFAULT_SUBWALLET_ID, 0, actions);

    val receipt = walletV5.sendExternalSigned(body).unwrap();

    expect(receipt).toHaveTx({ exitCode: 0 });

    // Verify the new wallet was deployed and has correct state
    expect(newWallet.getSeqno()).toEqual(0);
    expect(newWallet.getSubwalletId()).toEqual(42);
    expect(newWallet.isSignatureAllowed()).toEqual(true);
    expect(walletV5.getSeqno()).toEqual(1);
}

// ==================== Cross-chain: internal & extension ====================

get fun `test w5: int should not be able to install extension from different chain`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);

    val mcExtension = randomAddress("mc-extension-int", -1);

    var extraActions: array<ExtraAction> = [];
    extraActions.push(AddExtensionExtraAction { addr: mcExtension });

    val actions = ActionsList {
        outActions: [],
        extraActions,
    };
    val body = createSignedBody(keyPair, DEFAULT_SUBWALLET_ID, 0, actions);

    val receipt = walletV5.sendSignedBody(deployer.address, body, { value: grams("0.1") });

    expect(receipt).toHaveTx({ exitCode: Errors.ExtensionWrongWorkchain });

    val ext = walletV5.getExtensions();
    expect(ext).toHaveLength(0);
}

get fun `test w5: ext_action should not be able to install extension from different chain`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);
    var seqno = 0;

    // Add deployer as extension
    addExtensionSigned(walletV5, deployer.address, deployer.address, keyPair, seqno);
    seqno += 1;

    val mcExtension = randomAddress("mc-extension-ext-act", -1);

    var extraActions: array<ExtraAction> = [];
    extraActions.push(AddExtensionExtraAction { addr: mcExtension });

    val actionsBody = ActionsList {
        outActions: [],
        extraActions,
    };

    val receipt = walletV5.sendExtensionActionBody(
        deployer.address,
        actionsBody,
        0,
        { value: grams("0.1") },
    );

    expect(receipt).toHaveTx({ exitCode: Errors.ExtensionWrongWorkchain });

    // Only the deployer extension should exist
    val ext = walletV5.getExtensions();
    expect(ext).toHaveLength(1);
}

get fun `test w5: ext_action should not be able to remove extension from different chain`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);
    addExtensionSigned(walletV5, deployer.address, deployer.address, keyPair, 0);

    val (_, deployerHash) = deployer.address.getWorkchainAndHash();
    val mcAlias = beginCell()
        .storeUint(4, 3)
        .storeInt(-1, 8)
        .storeUint(deployerHash, 256)
        .toSlice() as address;

    var extraActions: array<ExtraAction> = [];
    extraActions.push(RemoveExtensionExtraAction { addr: mcAlias });

    val actionsBody = ActionsList {
        outActions: [],
        extraActions,
    };

    val receipt = walletV5.sendExtensionActionBody(
        deployer.address,
        actionsBody,
        0,
        { value: grams("0.1") },
    );

    expect(receipt).toHaveTx({ exitCode: Errors.ExtensionWrongWorkchain });

    val ext = walletV5.getExtensions();
    expect(ext).toHaveLength(1);
    expect(ext.exists(deployerHash)).toEqual(true);
}

// ==================== External-out: internal & extension ====================

get fun `test w5: int should be able to send external-out message`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);

    val testPayload = 54321;
    val testBody = beginCell().storeUint(testPayload, 32).endCell();

    val extAddrBuilder = beginCell().storeUint(1, 2).storeUint(32, 9).storeUint(testPayload, 32);
    val extOutMsg = createExternalLogMessage({ dest: extAddrBuilder, body: testBody });

    var outActions: array<SendAction> = [];
    outActions.push(SendAction {
        mode: SEND_MODE_REGULAR,
        messageCell: extOutMsg.messageCell,
    });

    val actions = ActionsList {
        outActions,
        extraActions: [],
    };
    val body = createSignedBody(keyPair, DEFAULT_SUBWALLET_ID, 0, actions);

    val receipt = walletV5.sendSignedBody(deployer.address, body, { value: grams("0.1") });

    expect(receipt).toHaveSuccessfulTx({
        from: deployer.address,
        to: walletV5.address,
        exitCode: 0,
    });

    expect(walletV5.getSeqno()).toEqual(1);
}

get fun `test w5: ext_action should be able to send external-out message`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);
    var seqno = 0;

    addExtensionSigned(walletV5, deployer.address, deployer.address, keyPair, seqno);
    seqno += 1;

    val testPayload = 99999;
    val testBody = beginCell().storeUint(testPayload, 32).endCell();

    val extAddrBuilder = beginCell().storeUint(1, 2).storeUint(32, 9).storeUint(testPayload, 32);
    val extOutMsg = createExternalLogMessage({ dest: extAddrBuilder, body: testBody });

    var outActions: array<SendAction> = [];
    outActions.push(SendAction {
        mode: SEND_MODE_REGULAR,
        messageCell: extOutMsg.messageCell,
    });

    val actionsBody = ActionsList {
        outActions,
        extraActions: [],
    };

    val receipt = walletV5.sendExtensionActionBody(
        deployer.address,
        actionsBody,
        0,
        { value: grams("0.1") },
    );

    expect(receipt).toHaveSuccessfulTx({
        from: deployer.address,
        to: walletV5.address,
        exitCode: 0,
    });
}
