import "@acton/crypto"
import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"

import "@contracts/types"
import "@contracts/w5-types"
import "@wrappers/SimpleExtension.gen"
import "@wrappers/WalletV5.gen"
import "@wrappers/utils"

const EXTENSION_DEPLOY_VALUE = ton("1")
const SUBSCRIPTION_AMOUNT = ton("0.25")
const PAYMENT_INTERVAL = 3600
const TEST_START_TIME = 1000

get fun `test extension: installs into wallet v5`() {
    val (wallet, _, extension) = setupInstalledExtension();

    val extensions = wallet.getExtensions();
    val (_, extensionHash) = extension.address.getWorkchainAndHash();

    expect(extensions).toHaveLength(1);
    expect(extensions.exists(extensionHash)).toEqual(true);

    val info = extension.extensionInfo();
    expect(info.walletAddress).toEqual(wallet.address);
    expect(info.subscriptionAmount).toEqual(SUBSCRIPTION_AMOUNT);
    expect(info.lastPaymentTime).toBeNull();
    expect(info.paymentTimeInterval).toEqual(PAYMENT_INTERVAL);
}

get fun `test extension: collects payment from installed wallet`() {
    val (wallet, _, extension) = setupInstalledExtension();

    val receipt = collectPayment(extension, 7)!;

    expect(receipt).toHaveSuccessfulTx<W5ExtensionActionRequest>({
        from: extension.address,
        to: wallet.address,
    });
    expect(receipt).toHaveSuccessfulTx<ReceivePaymentFromWallet>({
        from: wallet.address,
        to: extension.address,
        value: SUBSCRIPTION_AMOUNT,
    });

    val info = extension.extensionInfo();
    expect(info.lastPaymentTime == null).toEqual(false);
}

get fun `test extension: enforces payment interval`() {
    val (_, _, extension) = setupInstalledExtension();

    val firstReceipt = collectPayment(extension, 1)!;
    expect(firstReceipt).toHaveSuccessfulTx<ReceivePaymentFromWallet>({
        to: extension.address,
        value: SUBSCRIPTION_AMOUNT,
    });

    val blockedReceipt = collectPayment(extension, 2);
    expect(blockedReceipt).toBeNull();

    testing.setNow(TEST_START_TIME + PAYMENT_INTERVAL + 1);

    val allowedReceipt = collectPayment(extension, 3)!;
    expect(allowedReceipt).toHaveSuccessfulTx<ReceivePaymentFromWallet>({
        to: extension.address,
        value: SUBSCRIPTION_AMOUNT,
    });
}

get fun `test admin: cancels subscription and removes extension from wallet`() {
    val (wallet, admin, extension) = setupInstalledExtension();

    val receipt = extension.sendCancelSubscription(admin.address, 11, { value: ton("0.1") });

    expect(receipt).toHaveSuccessfulTx<CancelSubscription>({
        from: admin.address,
        to: extension.address,
    });
    expect(receipt).toHaveSuccessfulTx<W5ExtensionActionRequest>({
        from: extension.address,
        to: wallet.address,
    });

    val extensions = wallet.getExtensions();
    val (_, extensionHash) = extension.address.getWorkchainAndHash();
    expect(extensions.exists(extensionHash)).toEqual(false);
}

fun setupInstalledExtension(): (WalletV5, Treasury, SimpleExtension) {
    testing.setNow(TEST_START_TIME);

    val keyPair = crypto.generateKeyPair();
    val (wallet, admin) = deployWalletWithKey(keyPair);

    val extension = SimpleExtension.fromStorage({
        walletAddress: wallet.address,
        admin: admin.address,
        subscriptionAmount: SUBSCRIPTION_AMOUNT,
        lastPaymentTime: null,
        paymentTimeInterval: PAYMENT_INTERVAL,
    });
    val deployReceipt = extension.deploy(
        admin.address,
        { value: EXTENSION_DEPLOY_VALUE, bounce: false },
    );
    expect(deployReceipt).toHaveSuccessfulDeploy({ to: extension.address });

    val installReceipt = addExtensionSigned(wallet, admin.address, extension.address, keyPair, 0);
    expect(installReceipt).toHaveSuccessfulTx({ from: admin.address, to: wallet.address });

    return (wallet, admin, extension);
}

fun collectPayment(extension: SimpleExtension, queryId: uint64): SendResultList? {
    return extension.sendExternalCollectPaymentFromWallet(queryId);
}
