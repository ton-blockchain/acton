import "@acton/crypto"
import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"
import "@acton/types/big_array"
import "@wrappers/WalletV5.gen"
import "@wrappers/utils"
import "@contracts/walletv5/errors"
import "@contracts/walletv5/messages"

get fun `test w5: Send a simple transfer`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, _) = deployWalletWithKey(keyPair);

    val testReceiver = randomAddress("test-receiver");
    val forwardValue = grams("0.001");
    val receiverBalanceBefore = testing.getAccountBalance(testReceiver);

    var outActions: array<SendAction> = [];
    outActions.push(sendMsgAction(testReceiver, forwardValue));

    val actions = ActionsList {
        outActions,
        extraActions: [],
    };
    val body = createExternalSignedBody(keyPair, DEFAULT_SUBWALLET_ID, 0, actions);

    val receipt = walletV5.sendExternalSigned(body).unwrap();

    expect(receipt.size()).toEqual(2);

    expect(receipt).toHaveTx({
        from: walletV5.address,
        to: testReceiver,
        value: forwardValue,
    });

    val fee = receipt.at(1).tx.load().totalFees.grams;
    val receiverBalanceAfter = testing.getAccountBalance(testReceiver);
    expect(receiverBalanceAfter).toEqual(receiverBalanceBefore + forwardValue - fee);
}

get fun `test w5: Add an extension`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, _) = deployWalletWithKey(keyPair);

    val testExtension = randomAddress("test-extension");

    var extraActions: array<ExtraAction> = [];
    extraActions.push(AddExtensionExtraAction { addr: testExtension });

    val actions = ActionsList {
        outActions: [],
        extraActions,
    };
    val body = createExternalSignedBody(keyPair, DEFAULT_SUBWALLET_ID, 0, actions);

    val receipt = walletV5.sendExternalSigned(body).unwrap();

    expect(receipt.size()).toEqual(1);

    val ext = walletV5.getExtensions();
    val (_, extHash) = testExtension.getWorkchainAndHash();
    var expected: map<uint256, bool> = [];
    expected.set(extHash, true);
    expect(ext).toHaveLength(1);
    expect(ext.exists(extHash)).toEqual(true);
    expect(ext).toEqual(expected);
}

get fun `test w5: Send single transfer to a deployed wallet`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, _) = deployWalletWithKey(keyPair);

    // Use a treasury as receiver (framework can't read V5 wallet account state)
    val receiver = testing.treasury("receiver");
    val forwardValue = grams("0.001");
    val receiverBalanceBefore = testing.getAccountBalance(receiver.address);

    var outActions: array<SendAction> = [];
    outActions.push(sendMsgAction(receiver.address, forwardValue));

    val actions = ActionsList {
        outActions,
        extraActions: [],
    };
    val body = createExternalSignedBody(keyPair, DEFAULT_SUBWALLET_ID, 0, actions);

    val receipt = walletV5.sendExternalSigned(body).unwrap();

    expect(receipt.size()).toEqual(2);

    expect(receipt).toHaveTx({
        from: walletV5.address,
        to: receiver.address,
        value: forwardValue,
    });

    val fee = receipt.at(1).tx.load().totalFees.grams;
    val receiverBalanceAfter = testing.getAccountBalance(receiver.address);
    expect(receiverBalanceAfter).toEqual(receiverBalanceBefore + forwardValue - fee);
}

get fun `test w5: Send two transfers`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, _) = deployWalletWithKey(keyPair);

    val testReceiver1 = randomAddress("test-receiver-1");
    val forwardValue1 = grams("0.001");

    val testReceiver2 = testing.treasury("receiver-2");
    val forwardValue2 = grams("0.002");

    val receiver1BalanceBefore = testing.getAccountBalance(testReceiver1);
    val receiver2BalanceBefore = testing.getAccountBalance(testReceiver2.address);

    var outActions: array<SendAction> = [];
    outActions.push(sendMsgAction(testReceiver1, forwardValue1));
    outActions.push(sendMsgAction(testReceiver2.address, forwardValue2));

    val actions = ActionsList {
        outActions,
        extraActions: [],
    };
    val body = createExternalSignedBody(keyPair, DEFAULT_SUBWALLET_ID, 0, actions);

    val receipt = walletV5.sendExternalSigned(body).unwrap();

    expect(receipt.size()).toEqual(3);

    expect(receipt).toHaveTx({
        from: walletV5.address,
        to: testReceiver1,
        value: forwardValue1,
    });

    expect(receipt).toHaveTx({
        from: walletV5.address,
        to: testReceiver2.address,
        value: forwardValue2,
    });

    // C5 actions execute in reverse: receiver2 first (at index 1), receiver1 second (at index 2)
    val fee1 = receipt.at(2).tx.load().totalFees.grams;
    val fee2 = receipt.at(1).tx.load().totalFees.grams;

    val receiver1BalanceAfter = testing.getAccountBalance(testReceiver1);
    val receiver2BalanceAfter = testing.getAccountBalance(testReceiver2.address);
    expect(receiver1BalanceAfter).toEqual(receiver1BalanceBefore + forwardValue1 - fee1);
    expect(receiver2BalanceAfter).toEqual(receiver2BalanceBefore + forwardValue2 - fee2);
}

get fun `test w5: Add two extensions and do a transfer`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, _) = deployWalletWithKey(keyPair);

    val testExtension1 = randomAddress("test-extension-1");
    val testExtension2 = randomAddress("test-extension-2");
    val testReceiver = randomAddress("test-receiver");
    val forwardValue = grams("0.001");
    val receiverBalanceBefore = testing.getAccountBalance(testReceiver);

    var outActions: array<SendAction> = [];
    outActions.push(sendMsgAction(testReceiver, forwardValue));

    var extraActions: array<ExtraAction> = [];
    extraActions.push(AddExtensionExtraAction { addr: testExtension1 });
    extraActions.push(AddExtensionExtraAction { addr: testExtension2 });

    val actions = ActionsList { outActions, extraActions };
    val body = createExternalSignedBody(keyPair, DEFAULT_SUBWALLET_ID, 0, actions);

    val receipt = walletV5.sendExternalSigned(body).unwrap();

    expect(receipt.size()).toEqual(2);

    expect(receipt).toHaveTx({
        from: walletV5.address,
        to: testReceiver,
        value: forwardValue,
    });

    val fee = receipt.at(1).tx.load().totalFees.grams;
    val receiverBalanceAfter = testing.getAccountBalance(testReceiver);
    expect(receiverBalanceAfter).toEqual(receiverBalanceBefore + forwardValue - fee);

    val ext = walletV5.getExtensions();
    val (_, ext1Hash) = testExtension1.getWorkchainAndHash();
    val (_, ext2Hash) = testExtension2.getWorkchainAndHash();
    expect(ext).toHaveLength(2);
    expect(ext.exists(ext1Hash)).toEqual(true);
    expect(ext.exists(ext2Hash)).toEqual(true);
}

get fun `test w5: Remove extension`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, _) = deployWalletWithKey(keyPair);
    var seqno = 0;

    val testExtension = randomAddress("test-extension");

    // Add extension
    var extraActions1: array<ExtraAction> = [];
    extraActions1.push(AddExtensionExtraAction { addr: testExtension });
    val actions1 = ActionsList {
        outActions: [],
        extraActions: extraActions1,
    };
    val body1 = createExternalSignedBody(keyPair, DEFAULT_SUBWALLET_ID, seqno, actions1);
    walletV5.sendExternalSigned(body1);
    seqno += 1;

    val ext1 = walletV5.getExtensions();
    val (_, extHash) = testExtension.getWorkchainAndHash();
    expect(ext1).toHaveLength(1);
    expect(ext1.exists(extHash)).toEqual(true);

    // Remove extension
    var extraActions2: array<ExtraAction> = [];
    extraActions2.push(RemoveExtensionExtraAction { addr: testExtension });
    val actions2 = ActionsList {
        outActions: [],
        extraActions: extraActions2,
    };
    val body2 = createExternalSignedBody(keyPair, DEFAULT_SUBWALLET_ID, seqno, actions2);
    walletV5.sendExternalSigned(body2);

    val ext2 = walletV5.getExtensions();
    expect(ext2).toBeEmpty();
}

get fun `test w5: Should fail SetData action`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, _) = deployWalletWithKey(keyPair);

    // Pack SetData (opcode 0x1ff8ea0b) as an extra action — unsupported,
    // rejected by processExtraActions with Errors.UnsupportedAction (141).
    // Can't use ActionsList wrapper: ExtraAction is a union type, no unsafe cast available.
    val setDataAction = beginCell().storeUint(0x1ff8ea0b, 32).storeRef(createEmptyCell()).endCell();

    val actionsCell = beginCell()
        .storeUint(0, 1) // no c5 actions
        .storeUint(1, 1) // has extra actions
        .storeSlice(setDataAction.beginParse())
        .endCell();

    val payload = beginCell()
        .storeUint(0x7369676E, 32)
        .storeUint(DEFAULT_SUBWALLET_ID, 32)
        .storeUint(blockchain.now() + 1000, 32)
        .storeUint(0, 32)
        .storeSlice(actionsCell.beginParse())
        .endCell();

    val signature = crypto.sign(keyPair.privateKey, payload);
    val body = beginCell().storeSlice(payload.beginParse()).storeSlice(signature).endCell();

    val receipt = walletV5.sendExternalRaw(body);

    expect(receipt).toBeAccepted();
    expect(receipt.unwrap()).toHaveTx({ exitCode: Errors.UnsupportedAction });
    expect(walletV5.getSeqno()).toEqual(1);
}

get fun `test w5: Should fail SetCode action`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, _) = deployWalletWithKey(keyPair);

    // SetCode action (opcode 0xad4de08e) packed as C5 action
    val setCodeAction = beginCell()
        .storeRef(createEmptyCell()) // prev action (empty)
        .storeUint(0xad4de08e, 32) // action_set_code
        .storeUint(SEND_MODE_IGNORE_ERRORS, 8) // sendMode to pass external check
        .storeRef(createEmptyCell()) // code cell
        .endCell();

    val actionsCell = beginCell()
        .storeUint(1, 1) // has c5 actions
        .storeRef(setCodeAction)
        .storeUint(0, 1) // no extra actions
        .endCell();

    val payload = beginCell()
        .storeUint(0x7369676E, 32)
        .storeUint(DEFAULT_SUBWALLET_ID, 32)
        .storeUint(blockchain.now() + 1000, 32)
        .storeUint(0, 32)
        .storeSlice(actionsCell.beginParse())
        .endCell();

    val signature = crypto.sign(keyPair.privateKey, payload);
    val body = beginCell().storeSlice(payload.beginParse()).storeSlice(signature).endCell();

    val receipt = walletV5.sendExternalRaw(body);

    expect(receipt).toBeAccepted();
    expect(receipt.unwrap()).toHaveTx({ exitCode: Errors.InvalidC5 });
    expect(walletV5.getSeqno()).toEqual(1);
}

get fun `test w5: Should fail adding existing extension`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, _) = deployWalletWithKey(keyPair);
    var seqno = 0;

    val testExtension = randomAddress("test-extension");

    // Add extension first time
    var extraActions1: array<ExtraAction> = [];
    extraActions1.push(AddExtensionExtraAction { addr: testExtension });
    val actions1 = ActionsList {
        outActions: [],
        extraActions: extraActions1,
    };
    val body1 = createExternalSignedBody(keyPair, DEFAULT_SUBWALLET_ID, seqno, actions1);
    walletV5.sendExternalSigned(body1);
    seqno += 1;

    val ext1 = walletV5.getExtensions();
    val (_, extHash) = testExtension.getWorkchainAndHash();
    expect(ext1).toHaveLength(1);
    expect(ext1.exists(extHash)).toEqual(true);

    // Try adding same extension again — should fail with Errors.AddExtension (139)
    // Error happens after acceptExternalMessage, so tx is included
    var extraActions2: array<ExtraAction> = [];
    extraActions2.push(AddExtensionExtraAction { addr: testExtension });
    val actions2 = ActionsList {
        outActions: [],
        extraActions: extraActions2,
    };
    val body2 = createExternalSignedBody(keyPair, DEFAULT_SUBWALLET_ID, seqno, actions2);

    val receipt = walletV5.sendExternalSigned(body2).unwrap();

    expect(receipt.size()).toEqual(1);
    expect(receipt).toHaveTx({ exitCode: Errors.AddExtension });

    // Extension dict should still have 1 entry
    val ext2 = walletV5.getExtensions();
    expect(ext2).toHaveLength(1);
    expect(ext2.exists(extHash)).toEqual(true);
}

get fun `test w5: Should fail removing not existing extension`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, _) = deployWalletWithKey(keyPair);

    val testExtension = randomAddress("test-extension");

    var extraActions: array<ExtraAction> = [];
    extraActions.push(RemoveExtensionExtraAction { addr: testExtension });
    val actions = ActionsList {
        outActions: [],
        extraActions,
    };
    val body = createExternalSignedBody(keyPair, DEFAULT_SUBWALLET_ID, 0, actions);

    // Error happens after acceptExternalMessage, so tx is included
    val receipt = walletV5.sendExternalSigned(body).unwrap();

    expect(receipt.size()).toEqual(1);
    expect(receipt).toHaveTx({ exitCode: Errors.RemoveExtension });

    val ext = walletV5.getExtensions();
    expect(ext).toBeEmpty();
}

get fun `test w5: Should fail if signature is invalid wrong payload signed`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, _) = deployWalletWithKey(keyPair);

    val testReceiver = randomAddress("test-receiver");
    val forwardValue = grams("0.001");

    var outActions: array<SendAction> = [];
    outActions.push(sendMsgAction(testReceiver, forwardValue));
    val actions = ActionsList {
        outActions,
        extraActions: [],
    };

    // Build payload with correct data
    val payload = buildExternalSignedPayload(
        DEFAULT_SUBWALLET_ID,
        0,
        actions,
        blockchain.now() + 1000,
    );

    // Sign a DIFFERENT payload (wrong seqno)
    val fakePayload = buildExternalSignedPayload(
        DEFAULT_SUBWALLET_ID,
        1,
        actions,
        blockchain.now() + 1000,
    );

    val signature = crypto.sign(keyPair.privateKey, fakePayload.toCell()) as bytes64;
    val body = ExternalSignedBody { payload, signature };

    // Invalid signature → external message rejected before accept
    walletV5.sendExternalSigned(body);

    // Verify seqno didn't change
    expect(walletV5.getSeqno()).toEqual(0);
}

get fun `test w5: Should fail if signature is invalid wrong private key used`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, _) = deployWalletWithKey(keyPair);

    val testReceiver = randomAddress("test-receiver");
    val forwardValue = grams("0.001");

    var outActions: array<SendAction> = [];
    outActions.push(sendMsgAction(testReceiver, forwardValue));
    val actions = ActionsList {
        outActions,
        extraActions: [],
    };

    // Sign with a different key
    val fakeKeyPair = crypto.generateKeyPair();
    val body = createExternalSignedBody(fakeKeyPair, DEFAULT_SUBWALLET_ID, 0, actions);

    walletV5.sendExternalSigned(body);

    expect(walletV5.getSeqno()).toEqual(0);
}

get fun `test w5: Should fail if seqno is invalid`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, _) = deployWalletWithKey(keyPair);

    val testReceiver = randomAddress("test-receiver");
    val forwardValue = grams("0.001");

    var outActions: array<SendAction> = [];
    outActions.push(sendMsgAction(testReceiver, forwardValue));
    val actions = ActionsList {
        outActions,
        extraActions: [],
    };

    // Use wrong seqno (1 instead of 0)
    val body = createExternalSignedBody(keyPair, DEFAULT_SUBWALLET_ID, 1, actions);

    walletV5.sendExternalSigned(body);

    expect(walletV5.getSeqno()).toEqual(0);
}

get fun `test w5: Should fail if valid_until is expired`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, _) = deployWalletWithKey(keyPair);

    val testReceiver = randomAddress("test-receiver");
    val forwardValue = grams("0.001");

    var outActions: array<SendAction> = [];
    outActions.push(sendMsgAction(testReceiver, forwardValue));
    val actions = ActionsList {
        outActions,
        extraActions: [],
    };

    // Use expired validUntil (in the past)
    val body = createExternalSignedBody(keyPair, DEFAULT_SUBWALLET_ID, 0, actions, 1);

    walletV5.sendExternalSigned(body);

    expect(walletV5.getSeqno()).toEqual(0);
}

get fun `test w5: Should fail if valid_until equals now`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, _) = deployWalletWithKey(keyPair);

    val testReceiver = randomAddress("test-receiver");
    val forwardValue = grams("0.001");
    val receiverBalanceBefore = testing.getAccountBalance(testReceiver);

    var outActions: array<SendAction> = [];
    outActions.push(sendMsgAction(testReceiver, forwardValue));
    val actions = ActionsList {
        outActions,
        extraActions: [],
    };

    val body = createExternalSignedBody(
        keyPair,
        DEFAULT_SUBWALLET_ID,
        0,
        actions,
        blockchain.now(),
    );

    walletV5.sendExternalSigned(body);
    expect(walletV5.getSeqno()).toEqual(0);

    val receiverBalanceAfter = testing.getAccountBalance(testReceiver);
    expect(receiverBalanceAfter).toEqual(receiverBalanceBefore);
}

get fun `test w5: Should fail if walletId is wrong`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, _) = deployWalletWithKey(keyPair);

    val testReceiver = randomAddress("test-receiver");
    val forwardValue = grams("0.001");

    var outActions: array<SendAction> = [];
    outActions.push(sendMsgAction(testReceiver, forwardValue));
    val actions = ActionsList {
        outActions,
        extraActions: [],
    };

    // Use wrong walletId (1 instead of DEFAULT_SUBWALLET_ID)
    val body = createExternalSignedBody(keyPair, 1, 0, actions);

    walletV5.sendExternalSigned(body);

    expect(walletV5.getSeqno()).toEqual(0);
}

get fun `test w5: Should skip message if auth kind is wrong`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, _) = deployWalletWithKey(keyPair);

    val testReceiver = randomAddress("test-receiver");
    val forwardValue = grams("0.001");

    var outActions: array<SendAction> = [];
    outActions.push(sendMsgAction(testReceiver, forwardValue));
    val actions = ActionsList {
        outActions,
        extraActions: [],
    };

    // Use internal signed opcode (0x73696E74) instead of external (0x7369676E)
    val payload = buildSignedPayload(DEFAULT_SUBWALLET_ID, 0, actions, blockchain.now() + 1000);

    val signature = crypto.sign(keyPair.privateKey, payload.toCell()) as bytes64;
    val body = SignedBody { payload, signature };

    walletV5.sendExternalRaw(body.toCell());

    expect(walletV5.getSeqno()).toEqual(0);
}

get fun `test w5: Should skip message if auth kind not given`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, _) = deployWalletWithKey(keyPair);

    walletV5.sendExternalRaw(createEmptyCell());

    expect(walletV5.getSeqno()).toEqual(0);
}

get fun `test w5: Should skip message with simple text comment`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, _) = deployWalletWithKey(keyPair);

    // Simple text comment: 0x00000000 prefix followed by text bytes
    val body = beginCell()
        .storeUint(0, 32)
        .storeUint(0x48656C6C6F, 40) // "Hello"
        .endCell();

    walletV5.sendExternalRaw(body);

    expect(walletV5.getSeqno()).toEqual(0);
}

get fun `test w5: only_extension_can_change_signature_mode`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, _) = deployWalletWithKey(keyPair);

    var extraActions: array<ExtraAction> = [];
    extraActions.push(SetSignatureAllowedExtraAction { allowSignature: false });

    val actions = ActionsList {
        outActions: [],
        extraActions,
    };
    val body = createExternalSignedBody(keyPair, DEFAULT_SUBWALLET_ID, 0, actions);

    // Error happens after acceptExternalMessage, so tx is included with exit_code
    val receipt = walletV5.sendExternalSigned(body).unwrap();

    expect(receipt.size()).toEqual(1);
    expect(receipt).toHaveTx({ exitCode: Errors.OnlyExtensionCanChangeSignatureMode });

    expect(walletV5.isSignatureAllowed()).toEqual(true);
}
