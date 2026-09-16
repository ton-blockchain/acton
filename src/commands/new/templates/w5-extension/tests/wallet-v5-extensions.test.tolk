import "@acton/crypto"
import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"
import "@acton/types/big_array"
import "@wrappers/WalletV5.gen"
import "@wrappers/utils"
import "@contracts/walletv5/messages"

get fun `test w5: do a transfer from extension`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);
    var seqno = 0;

    // Add extension via signed message (like TS)
    addExtensionSigned(walletV5, deployer.address, deployer.address, keyPair, seqno);
    seqno += 1;

    val testReceiver = randomAddress("test-receiver");
    val forwardValue = grams("0.001");
    val receiverBalanceBefore = testing.getAccountBalance(testReceiver);

    var outActions: array<SendAction> = [];
    outActions.push(sendMsgAction(testReceiver, forwardValue));

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

    expect(receipt.size()).toEqual(2);

    expect(receipt).toHaveSuccessfulTx({ from: deployer.address, to: walletV5.address });

    expect(receipt).toHaveTx({
        from: walletV5.address,
        to: testReceiver,
        value: forwardValue,
    });

    val fee = receipt.at(1).tx.load().totalFees.grams;
    val receiverBalanceAfter = testing.getAccountBalance(testReceiver);
    expect(receiverBalanceAfter).toEqual(receiverBalanceBefore + forwardValue - fee);
}

get fun `test w5: do two transfers from extension and add other extension`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);
    var seqno = 0;

    // Add extension via signed message
    addExtensionSigned(walletV5, deployer.address, deployer.address, keyPair, seqno);
    seqno += 1;

    val testOtherExtension = randomAddress("other-extension");
    val testReceiver1 = randomAddress("test-receiver-1");
    val testReceiver2 = randomAddress("test-receiver-2");
    val forwardValue1 = grams("0.001");
    val forwardValue2 = grams("0.0012");

    val receiver1BalanceBefore = testing.getAccountBalance(testReceiver1);
    val receiver2BalanceBefore = testing.getAccountBalance(testReceiver2);

    var outActions: array<SendAction> = [];
    outActions.push(sendMsgAction(testReceiver1, forwardValue1));
    outActions.push(sendMsgAction(testReceiver2, forwardValue2));

    var extraActions: array<ExtraAction> = [];
    extraActions.push(AddExtensionExtraAction { addr: testOtherExtension });

    val actionsBody = ActionsList { outActions, extraActions };

    val receipt = walletV5.sendExtensionActionBody(
        deployer.address,
        actionsBody,
        0,
        { value: grams("0.1") },
    );

    expect(receipt.size()).toEqual(3);

    expect(receipt).toHaveSuccessfulTx({ from: deployer.address, to: walletV5.address });

    expect(receipt).toHaveTx({
        from: walletV5.address,
        to: testReceiver1,
        value: forwardValue1,
    });

    expect(receipt).toHaveTx({
        from: walletV5.address,
        to: testReceiver2,
        value: forwardValue2,
    });

    val fee1 = receipt.at(1).tx.load().totalFees.grams;
    val fee2 = receipt.at(2).tx.load().totalFees.grams;

    val receiver1BalanceAfter = testing.getAccountBalance(testReceiver1);
    val receiver2BalanceAfter = testing.getAccountBalance(testReceiver2);
    expect(receiver1BalanceAfter).toEqual(receiver1BalanceBefore + forwardValue1 - fee1);
    expect(receiver2BalanceAfter).toEqual(receiver2BalanceBefore + forwardValue2 - fee2);

    // Check extensions dict has both
    val ext = walletV5.getExtensions();
    val (_, deployerHash) = deployer.address.getWorkchainAndHash();
    val (_, otherExtHash) = testOtherExtension.getWorkchainAndHash();
    expect(ext).toHaveLength(2);
    expect(ext.exists(deployerHash)).toEqual(true);
    expect(ext.exists(otherExtHash)).toEqual(true);
}

get fun `test w5: add and remove other extension from extension`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);
    var seqno = 0;

    // Add extension via signed message
    addExtensionSigned(walletV5, deployer.address, deployer.address, keyPair, seqno);
    seqno += 1;

    val otherExtension = randomAddress("other-extension");

    // Add other extension from extension
    var extraActions1: array<ExtraAction> = [];
    extraActions1.push(AddExtensionExtraAction { addr: otherExtension });
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

    expect(receipt1.size()).toEqual(1);

    expect(receipt1).toHaveSuccessfulTx({ from: deployer.address, to: walletV5.address });

    // Verify both extensions exist
    val ext1 = walletV5.getExtensions();
    val (_, deployerHash) = deployer.address.getWorkchainAndHash();
    val (_, otherHash) = otherExtension.getWorkchainAndHash();
    expect(ext1).toHaveLength(2);
    expect(ext1.exists(deployerHash)).toEqual(true);
    expect(ext1.exists(otherHash)).toEqual(true);

    // Remove other extension
    var extraActions2: array<ExtraAction> = [];
    extraActions2.push(RemoveExtensionExtraAction { addr: otherExtension });
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

    expect(receipt2.size()).toEqual(1);

    expect(receipt2).toHaveSuccessfulTx({ from: deployer.address, to: walletV5.address });

    // Verify only deployer extension remains
    val ext2 = walletV5.getExtensions();
    expect(ext2).toHaveLength(1);
    expect(ext2.exists(deployerHash)).toEqual(true);
    expect(ext2.exists(otherHash)).toEqual(false);
}

get fun `test w5: extension removes itself`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);
    var seqno = 0;

    // Add extension via signed message
    addExtensionSigned(walletV5, deployer.address, deployer.address, keyPair, seqno);
    seqno += 1;

    var extraActions: array<ExtraAction> = [];
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

    expect(receipt.size()).toEqual(1);

    expect(receipt).toHaveSuccessfulTx({ from: deployer.address, to: walletV5.address });

    // Extensions dict should be empty
    val ext = walletV5.getExtensions();
    val (_, deployerHash) = deployer.address.getWorkchainAndHash();
    expect(ext).toBeEmpty();
    expect(ext.exists(deployerHash)).toEqual(false);
}

get fun `test w5: extension must be in dict to pass authorization`() {
    // Deploy without any extensions
    val keyPair = crypto.generateKeyPair();
    val (walletV5, _) = deployWalletWithKey(keyPair);

    val sender = testing.treasury("sender");
    val testReceiver = randomAddress("test-receiver");
    val forwardValue = grams("0.001");
    val receiverBalanceBefore = testing.getAccountBalance(testReceiver);

    var outActions: array<SendAction> = [];
    outActions.push(sendMsgAction(testReceiver, forwardValue));
    val actionsBody = ActionsList {
        outActions,
        extraActions: [],
    };

    val receipt = walletV5.sendExtensionActionBody(
        sender.address,
        actionsBody,
        0,
        { value: grams("0.1") },
    );

    expect(receipt.size()).toEqual(1);

    // Should NOT have transfer to testReceiver
    expect(receipt).toNotHaveTx({
        from: walletV5.address,
        to: testReceiver,
        value: forwardValue,
    });

    // Note that some random contract may have deposited funds with this prefix,
    // so we accept the funds silently instead of throwing an error (wallet v4 does the same).
    expect(receipt).toHaveSuccessfulTx({
        from: sender.address,
        to: walletV5.address,
        exitCode: 0,
    });

    val receiverBalanceAfter = testing.getAccountBalance(testReceiver);
    expect(receiverBalanceAfter).toEqual(receiverBalanceBefore);
}

get fun `test w5: disallow signature auth and do a transfer from extension`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);
    var seqno = 0;

    // Add extension via signed message
    addExtensionSigned(walletV5, deployer.address, deployer.address, keyPair, seqno);
    seqno += 1;

    // Disable signature auth from extension
    var extraActions0: array<ExtraAction> = [];
    extraActions0.push(SetSignatureAllowedExtraAction { allowSignature: false });
    val actionsBody0 = ActionsList {
        outActions: [],
        extraActions: extraActions0,
    };

    walletV5.sendExtensionActionBody(deployer.address, actionsBody0, 0, { value: grams("0.1") });

    expect(walletV5.isSignatureAllowed()).toEqual(false);

    // Do a transfer from extension
    val testReceiver = randomAddress("test-receiver");
    val forwardValue = grams("0.001");
    val receiverBalanceBefore = testing.getAccountBalance(testReceiver);

    var outActions: array<SendAction> = [];
    outActions.push(sendMsgAction(testReceiver, forwardValue));
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

    expect(receipt.size()).toEqual(2);

    expect(receipt).toHaveSuccessfulTx({ from: deployer.address, to: walletV5.address });

    expect(receipt).toHaveTx({
        from: walletV5.address,
        to: testReceiver,
        value: forwardValue,
    });

    val fee = receipt.at(1).tx.load().totalFees.grams;
    val receiverBalanceAfter = testing.getAccountBalance(testReceiver);
    expect(receiverBalanceAfter).toEqual(receiverBalanceBefore + forwardValue - fee);
}

get fun `test w5: disallow sig re-allow and self-delete by extension then do signed transfer`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);
    var seqno = 0;

    // Add extension via signed message
    addExtensionSigned(walletV5, deployer.address, deployer.address, keyPair, seqno);
    seqno += 1;

    // Disable signature auth from extension
    var extraActions0: array<ExtraAction> = [];
    extraActions0.push(SetSignatureAllowedExtraAction { allowSignature: false });
    val actionsBody0 = ActionsList {
        outActions: [],
        extraActions: extraActions0,
    };
    walletV5.sendExtensionActionBody(deployer.address, actionsBody0, 0, { value: grams("0.1") });

    expect(walletV5.isSignatureAllowed()).toEqual(false);

    // Re-allow signature auth and remove self as extension
    var extraActions1: array<ExtraAction> = [];
    extraActions1.push(SetSignatureAllowedExtraAction { allowSignature: true });
    extraActions1.push(RemoveExtensionExtraAction { addr: deployer.address });
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

    expect(receipt.size()).toEqual(1);

    expect(receipt).toHaveSuccessfulTx({
        from: deployer.address,
        to: walletV5.address,
        exitCode: 0,
    });

    expect(walletV5.isSignatureAllowed()).toEqual(true);

    // Extensions should be empty
    val ext = walletV5.getExtensions();
    val (_, deployerHash) = deployer.address.getWorkchainAndHash();
    expect(ext.exists(deployerHash)).toEqual(false);

    // Seqno should match (only 1 signed message was sent to add extension)
    val contractSeqno = walletV5.getSeqno();
    expect(contractSeqno).toEqual(seqno);

    // Do a signed transfer to verify signature auth works after re-enabling
    val testReceiver = randomAddress("test-receiver");
    val forwardValue = grams("0.001");
    val receiverBalanceBefore = testing.getAccountBalance(testReceiver);

    var outActions: array<SendAction> = [];
    outActions.push(sendMsgAction(testReceiver, forwardValue));
    val actionsBody2 = ActionsList {
        outActions,
        extraActions: [],
    };

    val signedBody = createSignedBody(keyPair, DEFAULT_SUBWALLET_ID, seqno, actionsBody2);

    val receipt2 = walletV5.sendSignedBody(deployer.address, signedBody, { value: grams("0.1") });

    expect(receipt2.size()).toEqual(2);

    expect(receipt2).toHaveSuccessfulTx({ from: deployer.address, to: walletV5.address });

    expect(receipt2).toHaveTx({
        from: walletV5.address,
        to: testReceiver,
        value: forwardValue,
    });

    val fee = receipt2.at(1).tx.load().totalFees.grams;
    val receiverBalanceAfter = testing.getAccountBalance(testReceiver);
    expect(receiverBalanceAfter).toEqual(receiverBalanceBefore + forwardValue - fee);
}

get fun `test w5: add ext disallow sig re-allow and self-delete by extension then do signed transfer`() {
    val keyPair = crypto.generateKeyPair();
    val (walletV5, deployer) = deployWalletWithKey(keyPair);
    var seqno = 0;

    // Add extension via signed message
    addExtensionSigned(walletV5, deployer.address, deployer.address, keyPair, seqno);
    seqno += 1;

    expect(walletV5.isSignatureAllowed()).toEqual(true);

    // Disable signature auth from extension
    var extraActions0: array<ExtraAction> = [];
    extraActions0.push(SetSignatureAllowedExtraAction { allowSignature: false });
    val actionsBody0 = ActionsList {
        outActions: [],
        extraActions: extraActions0,
    };

    val receipt0 = walletV5.sendExtensionActionBody(
        deployer.address,
        actionsBody0,
        0,
        { value: grams("0.1") },
    );

    expect(receipt0.size()).toEqual(1);

    expect(receipt0).toHaveSuccessfulTx({
        from: deployer.address,
        to: walletV5.address,
        exitCode: 0,
    });

    expect(walletV5.isSignatureAllowed()).toEqual(false);

    // Re-allow signature auth and remove self as extension
    var extraActions1: array<ExtraAction> = [];
    extraActions1.push(SetSignatureAllowedExtraAction { allowSignature: true });
    extraActions1.push(RemoveExtensionExtraAction { addr: deployer.address });
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

    expect(receipt1.size()).toEqual(1);

    expect(receipt1).toHaveSuccessfulTx({
        from: deployer.address,
        to: walletV5.address,
        exitCode: 0,
    });

    expect(walletV5.isSignatureAllowed()).toEqual(true);

    // Seqno should be 1 (one signed message was sent to add extension)
    expect(walletV5.getSeqno()).toEqual(seqno);

    // Do a signed transfer to verify signature auth works after re-enabling
    val testReceiver = randomAddress("test-receiver");
    val forwardValue = grams("0.001");
    val receiverBalanceBefore = testing.getAccountBalance(testReceiver);

    var outActions: array<SendAction> = [];
    outActions.push(sendMsgAction(testReceiver, forwardValue));
    val actionsBody2 = ActionsList {
        outActions,
        extraActions: [],
    };

    val signedBody = createSignedBody(keyPair, DEFAULT_SUBWALLET_ID, seqno, actionsBody2);

    val receipt2 = walletV5.sendSignedBody(deployer.address, signedBody, { value: grams("0.1") });

    expect(receipt2.size()).toEqual(2);

    expect(receipt2).toHaveSuccessfulTx({ from: deployer.address, to: walletV5.address });

    expect(receipt2).toHaveTx({
        from: walletV5.address,
        to: testReceiver,
        value: forwardValue,
    });

    val fee = receipt2.at(1).tx.load().totalFees.grams;
    val receiverBalanceAfter = testing.getAccountBalance(testReceiver);
    expect(receiverBalanceAfter).toEqual(receiverBalanceBefore + forwardValue - fee);
}
