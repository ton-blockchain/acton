import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"
import "@acton/types/transaction"
import "@contracts/messages"
import "@wrappers/JettonMinter.gen"
import "@wrappers/JettonWallet.gen"
import "@wrappers/utils"
import "test-utils"

const JETTON_E2E_NOW = 1000
const JETTON_E2E_MINT_AMOUNT = grams("25")
const JETTON_E2E_TRANSFER_AMOUNT = grams("7")
const JETTON_E2E_BURN_AMOUNT = grams("3")
const JETTON_E2E_FORWARD_AMOUNT = DEFAULT_FORWARD_TON_AMOUNT
const JETTON_E2E_TRANSFER_VALUE = DEFAULT_TRANSFER_MSG_VALUE
const JETTON_E2E_BURN_VALUE = DEFAULT_BURN_MSG_VALUE
const JETTON_E2E_FORWARD_MARKER = 0x4a455432
const JETTON_E2E_NEW_METADATA_URI = "ipfs://jetton-v2-e2e/metadata.json"

struct JettonE2eContext {
    minter: JettonMinter
    admin: Treasury
    nextAdmin: Treasury
    recipient: Treasury
    senderWallet: JettonWallet
    recipientWallet: JettonWallet
}

fun jettonE2eForwardPayload(): cell {
    return beginCell()
        .storeUint(JETTON_E2E_FORWARD_MARKER, 32)
        .storeSlice("jetton-v2-e2e-transfer".beginParse())
        .endCell();
}

fun deployMinterStage(): JettonE2eContext {
    testing.setNow(JETTON_E2E_NOW);

    val (minter, admin, _) = setupTest();
    val nextAdmin = testing.treasury("jetton-e2e-next-admin");
    val recipient = testing.treasury("jetton-e2e-recipient");
    val senderWallet = userWallet(minter, admin.address);
    val recipientWallet = userWallet(minter, recipient.address);

    val data = minter.getJettonData();
    expect(data.totalSupply).toEqual(0);
    expect(data.mintable).toEqual(true);
    expect(data.adminAddress).toEqual(admin.address);
    expect(senderWallet.getJettonBalance()).toEqual(0);

    return {
        minter,
        admin,
        nextAdmin,
        recipient,
        senderWallet,
        recipientWallet,
    };
}

fun mintToAdminStage(ctx: JettonE2eContext) {
    val supplyBefore = ctx.minter.getTotalSupply();
    val mint = ctx
        .minter
        .sendMint(
            ctx.admin.address,
            ctx.admin.address,
            JETTON_E2E_MINT_AMOUNT,
            grams("0.05"),
            grams("0.3"),
        );

    expect(mint).toHaveSuccessfulTx<MintNewJettons>({
        from: ctx.admin.address,
        to: ctx.minter.address,
    });
    expect(mint).toHaveSuccessfulDeploy({ to: ctx.senderWallet.address });
    expect(ctx.senderWallet.getJettonBalance()).toEqual(JETTON_E2E_MINT_AMOUNT);
    expect(ctx.minter.getTotalSupply()).toEqual(supplyBefore + JETTON_E2E_MINT_AMOUNT);
}

type Payload = PayloadInline | PayloadInRef

fun transferToRecipientStage(ctx: JettonE2eContext) {
    val forwardPayload = jettonE2eForwardPayload();
    val transfer = ctx
        .senderWallet
        .sendAskToTransfer(
            ctx.admin.address,
            JETTON_E2E_TRANSFER_VALUE,
            JETTON_E2E_TRANSFER_AMOUNT,
            ctx.recipient.address,
            ctx.admin.address,
            null,
            JETTON_E2E_FORWARD_AMOUNT,
            Payload.fromCell(forwardPayload),
        );

    expect(transfer).toHaveSuccessfulTx<AskToTransfer>({
        from: ctx.admin.address,
        to: ctx.senderWallet.address,
    });
    expect(transfer).toHaveSuccessfulDeploy({ to: ctx.recipientWallet.address });
    expect(transfer).toHaveTx<TransferNotificationForRecipient>({
        from: ctx.recipientWallet.address,
        to: ctx.recipient.address,
        value: JETTON_E2E_FORWARD_AMOUNT,
    });

    expect(ctx.senderWallet.getJettonBalance()).toEqual(
        JETTON_E2E_MINT_AMOUNT - JETTON_E2E_TRANSFER_AMOUNT,
    );
    expect(ctx.recipientWallet.getJettonBalance()).toEqual(JETTON_E2E_TRANSFER_AMOUNT);
}

fun burnFromRecipientStage(ctx: JettonE2eContext) {
    val supplyBefore = ctx.minter.getTotalSupply();
    val burn = ctx
        .recipientWallet
        .sendBurn(
            ctx.recipient.address,
            JETTON_E2E_BURN_VALUE,
            JETTON_E2E_BURN_AMOUNT,
            ctx.recipient.address,
            null,
        );

    expect(burn).toHaveSuccessfulTx<AskToBurn>({
        from: ctx.recipient.address,
        to: ctx.recipientWallet.address,
    });
    expect(burn).toHaveSuccessfulTx<BurnNotificationForMinter>({
        from: ctx.recipientWallet.address,
        to: ctx.minter.address,
    });
    expect(ctx.recipientWallet.getJettonBalance()).toEqual(
        JETTON_E2E_TRANSFER_AMOUNT - JETTON_E2E_BURN_AMOUNT,
    );
    expect(ctx.minter.getTotalSupply()).toEqual(supplyBefore - JETTON_E2E_BURN_AMOUNT);
}

fun updateMetadataAndDiscoverWalletStage(ctx: JettonE2eContext) {
    val newMetadata = buildTestMetadata(JETTON_E2E_NEW_METADATA_URI);
    val metadata = ctx.minter.sendChangeMetadata(ctx.admin.address, newMetadata);

    expect(metadata).toHaveSuccessfulTx<ChangeMinterMetadata>({
        from: ctx.admin.address,
        to: ctx.minter.address,
    });
    expect(ctx.minter.getContent().hash()).toEqual(newMetadata.hash());

    val discovery = ctx.minter.sendDiscovery(ctx.admin.address, ctx.recipient.address, true);
    val response = discovery.findTransaction<ResponseWalletAddress>({
        from: ctx.minter.address,
        to: ctx.admin.address,
    });
    expect(response).toBeNotNull();

    val body = response!.loadBody<ResponseWalletAddress>();
    expect(body.jettonWalletAddress).toEqual(ctx.recipientWallet.address);
    expect(body.ownerAddress).toBeNotNull();
    if (body.ownerAddress != null) {
        expect(body.ownerAddress.load()).toEqual(ctx.recipient.address);
    }
}

fun handOffAdminStage(ctx: JettonE2eContext) {
    val change = ctx.minter.sendChangeAdmin(ctx.admin.address, ctx.nextAdmin.address);
    expect(change).toHaveSuccessfulTx<ChangeMinterAdmin>({
        from: ctx.admin.address,
        to: ctx.minter.address,
    });
    expect(ctx.minter.getAdminAddress()).toEqual(ctx.admin.address);
    expect(ctx.minter.getNextAdminAddress()).toEqual(ctx.nextAdmin.address);

    val claim = ctx.minter.sendClaimAdmin(ctx.nextAdmin.address);
    expect(claim).toHaveSuccessfulTx<ClaimMinterAdmin>({
        from: ctx.nextAdmin.address,
        to: ctx.minter.address,
    });
    expect(ctx.minter.getAdminAddress()).toEqual(ctx.nextAdmin.address);
    expect(ctx.minter.getNextAdminAddress()).toEqual(null);
}

get fun `test jetton e2e bench: stage 1 deploy minter`() {
    deployMinterStage();
}

get fun `test jetton e2e bench: stage 2 mint to admin wallet`() {
    val ctx = deployMinterStage();
    mintToAdminStage(ctx);
}

get fun `test jetton e2e bench: stage 3 transfer to recipient wallet`() {
    val ctx = deployMinterStage();
    mintToAdminStage(ctx);
    transferToRecipientStage(ctx);
}

get fun `test jetton e2e bench: stage 4 burn recipient jettons`() {
    val ctx = deployMinterStage();
    mintToAdminStage(ctx);
    transferToRecipientStage(ctx);
    burnFromRecipientStage(ctx);
}

get fun `test jetton e2e bench: stage 5 full minter and wallet flow`() {
    val ctx = deployMinterStage();
    mintToAdminStage(ctx);
    transferToRecipientStage(ctx);
    burnFromRecipientStage(ctx);
    updateMetadataAndDiscoverWalletStage(ctx);
    handOffAdminStage(ctx);
}
