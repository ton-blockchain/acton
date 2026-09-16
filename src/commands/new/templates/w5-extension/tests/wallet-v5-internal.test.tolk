import "@acton/crypto"
import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"
import "@wrappers/WalletV5.gen"
import "@wrappers/utils"
import "@contracts/walletv5/errors"
import "@contracts/walletv5/messages"

get fun `test w5: Send a simple transfer`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);

    val testReceiver = randomAddress("test-receiver");
    val forwardValue = grams("0.001");
    val receiverBalanceBefore = testing.getAccountBalance(testReceiver);

    var outActions: array<SendAction> = [];
    outActions.push(sendMsgAction(testReceiver, forwardValue));

    val actions = ActionsList {
        outActions,
        extraActions: [],
    };
    val body = createSignedBody(keyPair, DEFAULT_SUBWALLET_ID, 0, actions);

    val receipt = walletV5.sendSignedBody(deployer.address, body, { value: grams("0.1") });

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
    val (walletV5, deployer) = deployWalletWithKey(keyPair);

    val testExtension = randomAddress("test-extension");

    var extraActions: array<ExtraAction> = [];
    extraActions.push(AddExtensionExtraAction { addr: testExtension });

    val actions = ActionsList {
        outActions: [],
        extraActions,
    };
    val body = createSignedBody(keyPair, DEFAULT_SUBWALLET_ID, 0, actions);

    walletV5.sendSignedBody(deployer.address, body, { value: grams("0.1") });

    val ext = walletV5.getExtensions();
    val (_, extHash) = testExtension.getWorkchainAndHash();
    var expected: map<uint256, bool> = [];
    expected.set(extHash, true);
    expect(ext).toHaveLength(1);
    expect(ext.exists(extHash)).toEqual(true);
    expect(ext).toEqual(expected);
}

get fun `test w5: Send two transfers`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);

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
    val body = createSignedBody(keyPair, DEFAULT_SUBWALLET_ID, 0, actions);

    val receipt = walletV5.sendSignedBody(deployer.address, body, { value: grams("0.1") });

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

    // C5 actions execute in reverse: receiver2 first, receiver1 second
    val fee1 = receipt.at(2).tx.load().totalFees.grams;
    val fee2 = receipt.at(1).tx.load().totalFees.grams;

    val receiver1BalanceAfter = testing.getAccountBalance(testReceiver1);
    val receiver2BalanceAfter = testing.getAccountBalance(testReceiver2.address);
    expect(receiver1BalanceAfter).toEqual(receiver1BalanceBefore + forwardValue1 - fee1);
    expect(receiver2BalanceAfter).toEqual(receiver2BalanceBefore + forwardValue2 - fee2);
}

get fun `test w5: Add two extensions and do a transfer`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);

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
    val body = createSignedBody(keyPair, DEFAULT_SUBWALLET_ID, 0, actions);

    val receipt = walletV5.sendSignedBody(deployer.address, body, { value: grams("0.1") });

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
    val (walletV5, deployer) = deployWalletWithKey(keyPair);
    var seqno = 0;

    val testExtension = randomAddress("test-extension");

    // Add extension
    var extraActions1: array<ExtraAction> = [];
    extraActions1.push(AddExtensionExtraAction { addr: testExtension });
    val actions1 = ActionsList {
        outActions: [],
        extraActions: extraActions1,
    };
    val body1 = createSignedBody(keyPair, DEFAULT_SUBWALLET_ID, seqno, actions1);
    walletV5.sendSignedBody(deployer.address, body1, { value: grams("0.1") });
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
    val body2 = createSignedBody(keyPair, DEFAULT_SUBWALLET_ID, seqno, actions2);
    walletV5.sendSignedBody(deployer.address, body2, { value: grams("0.1") });

    val ext2 = walletV5.getExtensions();
    expect(ext2).toBeEmpty();
}

get fun `test w5: Should fail SetData action`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);

    // Pack SetData (opcode 0x1ff8ea0b) as an extra action — unsupported
    val setDataAction = beginCell().storeUint(0x1ff8ea0b, 32).storeRef(createEmptyCell()).endCell();

    val actionsCell = beginCell()
        .storeUint(0, 1) // no c5 actions
        .storeUint(1, 1) // has extra actions
        .storeSlice(setDataAction.beginParse())
        .endCell();

    val payload = beginCell()
        .storeUint(0x73696E74, 32)
        .storeUint(DEFAULT_SUBWALLET_ID, 32)
        .storeUint(blockchain.now() + 1000, 32)
        .storeUint(0, 32)
        .storeSlice(actionsCell.beginParse())
        .endCell();

    val signature = crypto.sign(keyPair.privateKey, payload);
    val body = beginCell().storeSlice(payload.beginParse()).storeSlice(signature).endCell();

    val receipt = walletV5.sendAny(deployer.address, body, { value: grams("0.1") });

    expect(receipt).toHaveTx({ exitCode: Errors.UnsupportedAction });
}

get fun `test w5: Should fail SetCode action`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);

    // SetCode action (opcode 0xad4de08e) packed as C5 action
    val setCodeAction = beginCell()
        .storeRef(createEmptyCell()) // prev action (empty)
        .storeUint(0xad4de08e, 32) // action_set_code
        .storeUint(SEND_MODE_IGNORE_ERRORS, 8)
        .storeRef(createEmptyCell()) // code cell
        .endCell();

    val actionsCell = beginCell()
        .storeUint(1, 1) // has c5 actions
        .storeRef(setCodeAction)
        .storeUint(0, 1) // no extra actions
        .endCell();

    val payload = beginCell()
        .storeUint(0x73696E74, 32)
        .storeUint(DEFAULT_SUBWALLET_ID, 32)
        .storeUint(blockchain.now() + 1000, 32)
        .storeUint(0, 32)
        .storeSlice(actionsCell.beginParse())
        .endCell();

    val signature = crypto.sign(keyPair.privateKey, payload);
    val body = beginCell().storeSlice(payload.beginParse()).storeSlice(signature).endCell();

    val receipt = walletV5.sendAny(deployer.address, body, { value: grams("0.1") });

    expect(receipt).toHaveTx({ exitCode: Errors.InvalidC5 });
}

get fun `test w5: Should fail adding existing extension`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);
    var seqno = 0;

    val testExtension = randomAddress("test-extension");

    // Add extension first time
    var extraActions1: array<ExtraAction> = [];
    extraActions1.push(AddExtensionExtraAction { addr: testExtension });
    val actions1 = ActionsList {
        outActions: [],
        extraActions: extraActions1,
    };
    val body1 = createSignedBody(keyPair, DEFAULT_SUBWALLET_ID, seqno, actions1);
    walletV5.sendSignedBody(deployer.address, body1, { value: grams("0.1") });
    seqno += 1;

    val ext1 = walletV5.getExtensions();
    val (_, extHash) = testExtension.getWorkchainAndHash();
    expect(ext1).toHaveLength(1);
    expect(ext1.exists(extHash)).toEqual(true);

    // Try adding same extension again — should fail
    var extraActions2: array<ExtraAction> = [];
    extraActions2.push(AddExtensionExtraAction { addr: testExtension });
    val actions2 = ActionsList {
        outActions: [],
        extraActions: extraActions2,
    };
    val body2 = createSignedBody(keyPair, DEFAULT_SUBWALLET_ID, seqno, actions2);

    val receipt = walletV5.sendSignedBody(deployer.address, body2, { value: grams("0.1") });

    expect(receipt).toHaveTx({ exitCode: Errors.AddExtension });

    // Extension dict should still have 1 entry
    val ext2 = walletV5.getExtensions();
    expect(ext2).toHaveLength(1);
    expect(ext2.exists(extHash)).toEqual(true);
}

get fun `test w5: Should fail removing not existing extension`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);

    val testExtension = randomAddress("test-extension");

    var extraActions: array<ExtraAction> = [];
    extraActions.push(RemoveExtensionExtraAction { addr: testExtension });
    val actions = ActionsList {
        outActions: [],
        extraActions,
    };
    val body = createSignedBody(keyPair, DEFAULT_SUBWALLET_ID, 0, actions);

    val receipt = walletV5.sendSignedBody(deployer.address, body, { value: grams("0.1") });

    expect(receipt).toHaveTx({ exitCode: Errors.RemoveExtension });

    val ext = walletV5.getExtensions();
    expect(ext).toBeEmpty();
}

get fun `test w5: Should fail if signature is invalid wrong payload signed`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);

    val testReceiver = randomAddress("test-receiver");
    val forwardValue = grams("0.001");
    val receiverBalanceBefore = testing.getAccountBalance(testReceiver);

    var outActions: array<SendAction> = [];
    outActions.push(sendMsgAction(testReceiver, forwardValue));
    val actions = ActionsList {
        outActions,
        extraActions: [],
    };

    // Build payload with correct data
    val payload = buildSignedPayload(DEFAULT_SUBWALLET_ID, 0, actions, blockchain.now() + 1000);

    // Sign a DIFFERENT payload (wrong seqno)
    val fakePayload = buildSignedPayload(DEFAULT_SUBWALLET_ID, 1, actions, blockchain.now() + 1000);

    val signature = crypto.sign(keyPair.privateKey, fakePayload.toCell()) as bytes64;
    val body = SignedBody { payload, signature };

    val receipt = walletV5.sendSignedBody(deployer.address, body, { value: grams("0.1") });

    // Invalid signature → wallet silently accepts (exit 0), no actions executed
    expect(receipt).toHaveSuccessfulTx({
        from: deployer.address,
        to: walletV5.address,
        exitCode: 0,
    });

    expect(receipt).toNotHaveTx({
        from: walletV5.address,
        to: testReceiver,
        value: forwardValue,
    });

    val receiverBalanceAfter = testing.getAccountBalance(testReceiver);
    expect(receiverBalanceAfter).toEqual(receiverBalanceBefore);
}

get fun `test w5: Should fail if signature is invalid wrong private key used`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);

    val testReceiver = randomAddress("test-receiver");
    val forwardValue = grams("0.001");
    val receiverBalanceBefore = testing.getAccountBalance(testReceiver);

    var outActions: array<SendAction> = [];
    outActions.push(sendMsgAction(testReceiver, forwardValue));
    val actions = ActionsList {
        outActions,
        extraActions: [],
    };

    // Sign with a different key
    val fakeKeyPair = crypto.generateKeyPair();
    val body = createSignedBody(fakeKeyPair, DEFAULT_SUBWALLET_ID, 0, actions);

    val receipt = walletV5.sendSignedBody(deployer.address, body, { value: grams("0.1") });

    expect(receipt).toHaveSuccessfulTx({
        from: deployer.address,
        to: walletV5.address,
        exitCode: 0,
    });

    expect(receipt).toNotHaveTx({
        from: walletV5.address,
        to: testReceiver,
        value: forwardValue,
    });

    val receiverBalanceAfter = testing.getAccountBalance(testReceiver);
    expect(receiverBalanceAfter).toEqual(receiverBalanceBefore);
}

get fun `test w5: Should fail if seqno is invalid`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);

    val testReceiver = randomAddress("test-receiver");
    val forwardValue = grams("0.001");
    val receiverBalanceBefore = testing.getAccountBalance(testReceiver);

    var outActions: array<SendAction> = [];
    outActions.push(sendMsgAction(testReceiver, forwardValue));
    val actions = ActionsList {
        outActions,
        extraActions: [],
    };

    // Use wrong seqno (1 instead of 0)
    val body = createSignedBody(keyPair, DEFAULT_SUBWALLET_ID, 1, actions);

    val receipt = walletV5.sendSignedBody(deployer.address, body, { value: grams("0.1") });

    expect(receipt).toHaveTx({ exitCode: Errors.InvalidSeqno });

    expect(receipt).toNotHaveTx({
        from: walletV5.address,
        to: testReceiver,
        value: forwardValue,
    });

    val receiverBalanceAfter = testing.getAccountBalance(testReceiver);
    expect(receiverBalanceAfter).toEqual(receiverBalanceBefore);
}

get fun `test w5: Should fail if valid_until is expired`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);

    val testReceiver = randomAddress("test-receiver");
    val forwardValue = grams("0.001");
    val receiverBalanceBefore = testing.getAccountBalance(testReceiver);

    var outActions: array<SendAction> = [];
    outActions.push(sendMsgAction(testReceiver, forwardValue));
    val actions = ActionsList {
        outActions,
        extraActions: [],
    };

    // Use expired validUntil (in the past)
    val payload = buildSignedPayload(DEFAULT_SUBWALLET_ID, 0, actions, 1);

    val signature = crypto.sign(keyPair.privateKey, payload.toCell()) as bytes64;
    val body = SignedBody { payload, signature };

    val receipt = walletV5.sendSignedBody(deployer.address, body, { value: grams("0.1") });

    expect(receipt).toHaveTx({ exitCode: Errors.Expired });

    val receiverBalanceAfter = testing.getAccountBalance(testReceiver);
    expect(receiverBalanceAfter).toEqual(receiverBalanceBefore);
}

get fun `test w5: Should fail if valid_until equals now`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);

    val testReceiver = randomAddress("test-receiver");
    val forwardValue = grams("0.001");
    val receiverBalanceBefore = testing.getAccountBalance(testReceiver);

    var outActions: array<SendAction> = [];
    outActions.push(sendMsgAction(testReceiver, forwardValue));
    val actions = ActionsList {
        outActions,
        extraActions: [],
    };

    val payload = buildSignedPayload(DEFAULT_SUBWALLET_ID, 0, actions, blockchain.now());

    val signature = crypto.sign(keyPair.privateKey, payload.toCell()) as bytes64;
    val body = SignedBody { payload, signature };

    val receipt = walletV5.sendSignedBody(deployer.address, body, { value: grams("0.1") });

    expect(receipt).toHaveTx({ exitCode: Errors.Expired });

    val receiverBalanceAfter = testing.getAccountBalance(testReceiver);
    expect(receiverBalanceAfter).toEqual(receiverBalanceBefore);
}

get fun `test w5: Should fail if walletId is wrong`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);

    val testReceiver = randomAddress("test-receiver");
    val forwardValue = grams("0.001");
    val receiverBalanceBefore = testing.getAccountBalance(testReceiver);

    var outActions: array<SendAction> = [];
    outActions.push(sendMsgAction(testReceiver, forwardValue));
    val actions = ActionsList {
        outActions,
        extraActions: [],
    };

    // Use wrong walletId (1 instead of DEFAULT_SUBWALLET_ID)
    val body = createSignedBody(keyPair, 1, 0, actions);

    val receipt = walletV5.sendSignedBody(deployer.address, body, { value: grams("0.1") });

    expect(receipt).toHaveTx({ exitCode: Errors.InvalidWalletId });

    val receiverBalanceAfter = testing.getAccountBalance(testReceiver);
    expect(receiverBalanceAfter).toEqual(receiverBalanceBefore);
}

get fun `test w5: Should skip message if auth kind is wrong`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);

    val testReceiver = randomAddress("test-receiver");
    val forwardValue = grams("0.001");

    var outActions: array<SendAction> = [];
    outActions.push(sendMsgAction(testReceiver, forwardValue));
    val actions = ActionsList {
        outActions,
        extraActions: [],
    };

    // Use external signed opcode (0x7369676E) instead of internal (0x73696E74)
    val payload = buildExternalSignedPayload(
        DEFAULT_SUBWALLET_ID,
        0,
        actions,
        blockchain.now() + 1000,
    );

    val signature = crypto.sign(keyPair.privateKey, payload.toCell()) as bytes64;
    val body = ExternalSignedBody { payload, signature };

    walletV5.sendAny(deployer.address, body.toCell(), { value: grams("0.1") });

    expect(walletV5.getSeqno()).toEqual(0);
}

get fun `test w5: Should skip message if auth kind not given`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);

    walletV5.sendAny(deployer.address, createEmptyCell(), { value: grams("0.1") });

    expect(walletV5.getSeqno()).toEqual(0);
}

fun buildSignedInternalBodyWithPayloadBits(keyPair: KeyPair, payloadBits: int): cell {
    val payload = beginCell().storeUint(0x73696E74, 32).storeUint(0, payloadBits - 32).endCell();
    val signature = crypto.sign(keyPair.privateKey, payload);
    return beginCell().storeSlice(payload.beginParse()).storeSlice(signature).endCell();
}

get fun `test w5: Should not revert on short sint messages`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);

    val body = beginCell().storeUint(0x73696E74, 32).endCell();

    val receipt = walletV5.sendAny(deployer.address, body, { value: grams("0.1") });

    expect(receipt).toHaveSuccessfulTx({
        from: deployer.address,
        to: walletV5.address,
        exitCode: 0,
    });
}

get fun `test w5: Should ignore sint messages shorter than signed header boundary`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);

    val body = buildSignedInternalBodyWithPayloadBits(keyPair, 127);

    val receipt = walletV5.sendAny(deployer.address, body, { value: grams("0.1") });

    expect(receipt).toHaveSuccessfulTx({
        from: deployer.address,
        to: walletV5.address,
        exitCode: 0,
    });

    expect(walletV5.getSeqno()).toEqual(0);
}

get fun `test w5: Should not revert on long incorrect sint messages`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);

    val body = beginCell()
        .storeUint(0x73696E74, 32)
        .storeUint(0, 256)
        .storeUint(0, 256)
        .storeUint(0, 145) // total 657 zero bits after opcode
        .endCell();

    val receipt = walletV5.sendAny(deployer.address, body, { value: grams("0.1") });

    expect(receipt).toHaveSuccessfulTx({
        from: deployer.address,
        to: walletV5.address,
        exitCode: 0,
    });
}

get fun `test w5: Should skip message with simple text comment`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);

    val body: string = "Hello";

    val receipt = walletV5.sendAny(deployer.address, body.toCell(), { value: grams("0.1") });

    expect(receipt).toHaveSuccessfulTx({
        from: deployer.address,
        to: walletV5.address,
        exitCode: 0,
    });
}

get fun `test w5: Should skip message with longer text comment`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);

    val body: string = "Hello world Hello world Hello world Hello world Hello world Hello world Hello world Hello world Hello world Hello world Hello world Hello world Hello world Hello world Hello world Hello world Hello world Hello world Hello world Hello world";

    val receipt = walletV5.sendAny(deployer.address, body.toCell(), { value: grams("0.1") });

    expect(receipt).toHaveSuccessfulTx({
        from: deployer.address,
        to: walletV5.address,
        exitCode: 0,
    });
}

get fun `test w5: Should fail disallowing signature auth with no exts`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);
    var seqno = 0;

    // Add extension via signed message
    addExtensionSigned(walletV5, deployer.address, deployer.address, keyPair, seqno);
    seqno += 1;

    // From extension: remove self + disable sig → error 142
    var extraActions: array<ExtraAction> = [];
    extraActions.push(RemoveExtensionExtraAction { addr: deployer.address });
    extraActions.push(SetSignatureAllowedExtraAction { allowSignature: false });
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

    expect(receipt).toHaveTx({ exitCode: Errors.DisableSignatureWhenExtensionsIsEmpty });

    expect(walletV5.isSignatureAllowed()).toEqual(true);
}

get fun `test w5: Should fail allowing signature auth when allowed`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);
    var seqno = 0;

    // Add extension via signed message
    addExtensionSigned(walletV5, deployer.address, deployer.address, keyPair, seqno);
    seqno += 1;

    // From extension: allow sig (already allowed) → error 143
    var extraActions: array<ExtraAction> = [];
    extraActions.push(SetSignatureAllowedExtraAction { allowSignature: true });
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

    expect(receipt).toHaveTx({ exitCode: Errors.ThisSignatureModeAlreadySet });

    expect(walletV5.isSignatureAllowed()).toEqual(true);
}

get fun `test w5: Should add ext and disallow signature auth`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);
    var seqno = 0;

    // Add extension via signed message
    addExtensionSigned(walletV5, deployer.address, deployer.address, keyPair, seqno);
    seqno += 1;

    val testExtension = randomAddress("test-extension");

    // From extension: add another ext + disable sig
    var extraActions: array<ExtraAction> = [];
    extraActions.push(AddExtensionExtraAction { addr: testExtension });
    extraActions.push(SetSignatureAllowedExtraAction { allowSignature: false });
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

    expect(receipt).toHaveSuccessfulTx({
        from: deployer.address,
        to: walletV5.address,
        exitCode: 0,
    });

    val ext = walletV5.getExtensions();
    val (_, extHash) = testExtension.getWorkchainAndHash();
    expect(ext).toHaveLength(2);
    expect(ext.exists(extHash)).toEqual(true);

    expect(walletV5.isSignatureAllowed()).toEqual(false);
    expect(walletV5.getSeqno()).toEqual(seqno);
}

get fun `test w5: Should add ext and disallow signature auth in separate txs`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);
    var seqno = 0;

    // Add extension via signed message
    addExtensionSigned(walletV5, deployer.address, deployer.address, keyPair, seqno);
    seqno += 1;

    val testExtension = randomAddress("test-extension");

    // From extension: add another extension
    var extraActions1: array<ExtraAction> = [];
    extraActions1.push(AddExtensionExtraAction { addr: testExtension });
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

    val ext = walletV5.getExtensions();
    val (_, extHash) = testExtension.getWorkchainAndHash();
    expect(ext).toHaveLength(2);
    expect(ext.exists(extHash)).toEqual(true);

    expect(walletV5.isSignatureAllowed()).toEqual(true);

    // From extension: disable sig
    var extraActions2: array<ExtraAction> = [];
    extraActions2.push(SetSignatureAllowedExtraAction { allowSignature: false });
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

    expect(walletV5.isSignatureAllowed()).toEqual(false);
    expect(walletV5.getSeqno()).toEqual(seqno);
}

get fun `test w5: Should add ext disallow sign allow sign remove ext in one tx then send in other`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);
    var seqno = 0;

    // Add extension via signed message
    addExtensionSigned(walletV5, deployer.address, deployer.address, keyPair, seqno);
    seqno += 1;

    val testExtension = randomAddress("test-extension");

    // From extension: add ext, disable sig, allow sig, remove ext
    var extraActions: array<ExtraAction> = [];
    extraActions.push(AddExtensionExtraAction { addr: testExtension });
    extraActions.push(SetSignatureAllowedExtraAction { allowSignature: false });
    extraActions.push(SetSignatureAllowedExtraAction { allowSignature: true });
    extraActions.push(RemoveExtensionExtraAction { addr: testExtension });
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

    expect(receipt).toHaveSuccessfulTx({
        from: deployer.address,
        to: walletV5.address,
        exitCode: 0,
    });

    expect(walletV5.isSignatureAllowed()).toEqual(true);
    expect(walletV5.getSeqno()).toEqual(seqno);

    // Now do a signed transfer
    val testReceiver = randomAddress("test-receiver");
    val forwardValue = grams("0.001");
    val receiverBalanceBefore = testing.getAccountBalance(testReceiver);

    var outActions: array<SendAction> = [];
    outActions.push(sendMsgAction(testReceiver, forwardValue));
    val actions2 = ActionsList {
        outActions,
        extraActions: [],
    };
    val signedBody = createSignedBody(keyPair, DEFAULT_SUBWALLET_ID, seqno, actions2);

    val receipt2 = walletV5.sendSignedBody(deployer.address, signedBody, { value: grams("0.1") });

    expect(receipt2).toHaveTx({
        from: walletV5.address,
        to: testReceiver,
        value: forwardValue,
    });

    val fee = receipt2.at(1).tx.load().totalFees.grams;
    val receiverBalanceAfter = testing.getAccountBalance(testReceiver);
    expect(receiverBalanceAfter).toEqual(receiverBalanceBefore + forwardValue - fee);
}

get fun `test w5: Should fail removing last extension with signature auth disabled`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);
    var seqno = 0;

    // Add extension via signed message
    addExtensionSigned(walletV5, deployer.address, deployer.address, keyPair, seqno);
    seqno += 1;

    val testExtension = randomAddress("test-extension");

    // From extension: add ext, disable sig, remove ext, remove self → error 144
    var extraActions: array<ExtraAction> = [];
    extraActions.push(AddExtensionExtraAction { addr: testExtension });
    extraActions.push(SetSignatureAllowedExtraAction { allowSignature: false });
    extraActions.push(RemoveExtensionExtraAction { addr: testExtension });
    extraActions.push(RemoveExtensionExtraAction { addr: deployer.address });
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

    expect(receipt).toHaveTx({ exitCode: Errors.RemoveLastExtensionWhenSignatureDisabled });

    expect(walletV5.isSignatureAllowed()).toEqual(true);
}

get fun `test w5: Should fail disallowing signature auth twice in tx`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);
    var seqno = 0;

    // Add extension via signed message
    addExtensionSigned(walletV5, deployer.address, deployer.address, keyPair, seqno);
    seqno += 1;

    val testExtension = randomAddress("test-extension");

    // From extension: add ext, disable sig, disable sig again → error 143
    var extraActions: array<ExtraAction> = [];
    extraActions.push(AddExtensionExtraAction { addr: testExtension });
    extraActions.push(SetSignatureAllowedExtraAction { allowSignature: false });
    extraActions.push(SetSignatureAllowedExtraAction { allowSignature: false });
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

    expect(receipt).toHaveTx({ exitCode: Errors.ThisSignatureModeAlreadySet });

    expect(walletV5.isSignatureAllowed()).toEqual(true);
}

get fun `test w5: Should add ext disallow sig auth then fail different signed tx`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);
    var seqno = 0;

    // Add extension via signed message
    addExtensionSigned(walletV5, deployer.address, deployer.address, keyPair, seqno);
    seqno += 1;

    val testExtension = randomAddress("test-extension");

    // From extension: add another ext + disable sig
    var extraActions: array<ExtraAction> = [];
    extraActions.push(AddExtensionExtraAction { addr: testExtension });
    extraActions.push(SetSignatureAllowedExtraAction { allowSignature: false });
    val actionsBody = ActionsList {
        outActions: [],
        extraActions,
    };

    val receipt1 = walletV5.sendExtensionActionBody(
        deployer.address,
        actionsBody,
        0,
        { value: grams("0.1") },
    );

    expect(receipt1).toHaveSuccessfulTx({
        from: deployer.address,
        to: walletV5.address,
        exitCode: 0,
    });

    expect(walletV5.isSignatureAllowed()).toEqual(false);
    expect(walletV5.getSeqno()).toEqual(seqno);

    // Try signed transfer → error 132 (signature disabled)
    val testReceiver = randomAddress("test-receiver");
    val forwardValue = grams("0.001");
    val receiverBalanceBefore = testing.getAccountBalance(testReceiver);

    var outActions: array<SendAction> = [];
    outActions.push(sendMsgAction(testReceiver, forwardValue));
    val actions2 = ActionsList {
        outActions,
        extraActions: [],
    };
    val signedBody = createSignedBody(keyPair, DEFAULT_SUBWALLET_ID, seqno, actions2);

    val receipt2 = walletV5.sendSignedBody(deployer.address, signedBody, { value: grams("0.1") });

    expect(receipt2).toHaveTx({ exitCode: Errors.SignatureDisabled });

    expect(receipt2).toNotHaveTx({
        from: walletV5.address,
        to: testReceiver,
        value: forwardValue,
    });

    val receiverBalanceAfter = testing.getAccountBalance(testReceiver);
    expect(receiverBalanceAfter).toEqual(receiverBalanceBefore);
}
