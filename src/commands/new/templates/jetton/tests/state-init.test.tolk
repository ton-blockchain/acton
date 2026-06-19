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

const MAX_JETTON_VALUE = 0xffffffffffffffffffffffffffffff

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
    val (accountCells, accountBits, _) = state!.toCell().calculateSizeStrict(4096);
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
