import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"
import "@acton/tlb/either"
import "@acton/tlb/maybe"
import "@acton/types/transaction"
import "@contracts/fees-management"
import "@contracts/messages"
import "@contracts/storage"
import "@wrappers/JettonMinter.gen"
import "@wrappers/JettonWallet.gen"
import "@wrappers/utils"
import "test-utils"
import "../scripts/utils/common"

const MAX_JETTON_VALUE = 0xffffffffffffffffffffffffffffff

get fun `test state: minter deploy helper preserves sharding`() {
    val deployer = testing.treasury("sharded-minter-deployer");
    val minter = JettonMinter.fromStorage(
        {
            totalSupply: 0,
            adminAddress: deployer.address,
            nextAdminAddress: null,
            metadata: buildTestMetadata("link"),
        },
        { fixedPrefixLength: 8, closeTo: deployer.address },
    );

    val result = minter.sendDeploy(deployer.address, { value: grams("10") });
    expect(result).toHaveSuccessfulDeploy({ to: minter.address });
    expect(minter.getJettonData().adminAddress).toEqual(deployer.address);
}

get fun `test state: minter deploy helper preserves workchain`() {
    val deployer = testing.treasury("masterchain-minter-deployer");
    val minter = JettonMinter.fromStorage(
        {
            totalSupply: 0,
            adminAddress: deployer.address,
            nextAdminAddress: null,
            metadata: buildTestMetadata("link"),
        },
        null,
        MASTERCHAIN,
    );

    val result = minter.sendDeploy(deployer.address, { value: grams("10") });
    expect(result).toHaveSuccessfulDeploy({ to: minter.address });
    expect(minter.address.getWorkchain()).toEqual(MASTERCHAIN);
}

get fun `test metadata: script reader accepts inline and referenced snake data`() {
    val inline = beginCell().storeUint(0, 8).storeSlice("ACTON".beginParse()).endCell();
    val referenced = SnakeDataReply { string: "ACTON" }.toCell();
    val continuation = beginCell()
        .storeUint(0, 8)
        .storeSlice("AC".beginParse())
        .storeString("TON")
        .endCell();
    val empty = beginCell().storeUint(0, 8).endCell();

    var metadata = OnchainMetadataReply { contentDict: [] };
    metadata.contentDict.set("inline".sha256(), inline as Cell<SnakeDataReply>);
    metadata.contentDict.set("referenced".sha256(), referenced);
    metadata.contentDict.set("continuation".sha256(), continuation as Cell<SnakeDataReply>);
    metadata.contentDict.set("empty".sha256(), empty as Cell<SnakeDataReply>);

    expect(readOnchainMetadataField(metadata, "inline".sha256())).toEqual("ACTON");
    expect(readOnchainMetadataField(metadata, "referenced".sha256())).toEqual("ACTON");
    expect(readOnchainMetadataField(metadata, "continuation".sha256())).toEqual("ACTON");
    expect(readOnchainMetadataField(metadata, "empty".sha256())).toEqual("");
    expect(readOnchainMetadataField(metadata, "missing".sha256())).toBeNull();
}

get fun `test state: should mint max jetton value`() {
    val (minter, deployer, _) = setupTest();
    val wallet = userWallet(minter, deployer.address);

    val mintResult = minter.sendMint(
        deployer.address,
        deployer.address,
        MAX_JETTON_VALUE,
        grams("0.05"),
        grams("0.3"),
    );
    expect(mintResult).toHaveSuccessfulTx<MintNewJettons>({
        from: deployer.address,
        to: minter.address,
    });
    expect(mintResult).toHaveSuccessfulTx<InternalTransferStep>({
        from: minter.address,
        to: wallet.address,
    });

    expect(wallet.getJettonBalance()).toEqual(MAX_JETTON_VALUE);
    expect(minter.getTotalSupply()).toEqual(MAX_JETTON_VALUE);
}

get fun `test state: wallet storage and init-state bounds should match fee constants`() {
    val (minter, deployer, _) = setupTest();
    val recipient = randomAddress();
    val recipientWallet = userWallet(minter, recipient);

    val mintResult = minter.sendMint(
        deployer.address,
        recipient,
        TEST_JETTON_UNIT,
        0,
        grams("0.3"),
    );
    expect(mintResult).toHaveSuccessfulTx<InternalTransferStep>({
        from: minter.address,
        to: recipientWallet.address,
    });

    val state = testing.getAccountState(recipientWallet.address);
    expect(state).toBeNotNull();
    if (state == null) {
        return;
    }
    val (accountCells, accountBits, _) = state.toCell().calculateSizeStrict(4096);
    expect(accountBits).toBeGreater(0);
    expect(accountCells).toBeGreater(0);

    val walletStorageCell = WalletStorage {
        jettonBalance: TEST_JETTON_UNIT,
        ownerAddress: recipient,
        minterAddress: minter.address,
    }.toCell();
    val (walletStorageCells, walletStorageBits, _) = walletStorageCell.calculateSizeStrict(4096);
    expect(walletStorageBits).toBeLessOrEqual(STORAGE_SIZE_MAX_WALLET_BITS);
    expect(walletStorageCells).toBeLessOrEqual(STORAGE_SIZE_MAX_WALLET_CELLS);

    val transferTx = mintResult.findTransaction<InternalTransferStep>({
        from: minter.address,
        to: recipientWallet.address,
        success: true,
        exitCode: 0,
    });
    expect(transferTx).toBeNotNull();
    val inMsg = transferTx!.loadInMsg<InternalTransferStep>();
    if (inMsg.init is TlbNone) {
        return;
    }

    val initEither = inMsg.init.value;
    var initStateCell = beginCell().endCell();
    if (initEither is TlbEitherLeft<StateInit>) {
        initStateCell = initEither.value.toCell();
    } else {
        initStateCell = initEither.value;
    }
    val (initCells, initBits, _) = initStateCell.calculateSizeStrict(4096);

    expect(initBits).toBeGreater(0);
    expect(initCells).toBeGreater(0);
}
