import "@acton/build"
import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"
import "@acton/types/big_array"
import "@acton/types/out_actions"
import "@acton/types/transaction"
import "@contracts/jetton-utils"
import "@contracts/messages"
import "@contracts/storage"
import "@wrappers/JettonMinter.gen"
import "@wrappers/JettonWallet.gen"
import "@wrappers/utils"

fun buildTestMetadata(uri: string): cell {
    var metadata: OnchainMetadataReply = { contentDict: [] };
    metadata.contentDict.set("uri".sha256(), SnakeDataReply { string: uri }.toCell());
    return metadata.toCell();
}

// Common values used across tests (nanotons for TON amounts)
const TEST_JETTON_UNIT = 1
const DEFAULT_MINTED_JETTONS = ton("5") // 5 TON in jetton units
const DEFAULT_TRANSFER_MSG_VALUE = 300000000 // 0.3 TON
const LOW_TRANSFER_MSG_VALUE = 200000000 // 0.2 TON
const DEFAULT_BURN_MSG_VALUE = 200000000 // 0.2 TON
const DEFAULT_FORWARD_TON_AMOUNT = 50000000 // 0.05 TON
const MIN_EDGE_DELTA = 1
const CONFIG_SCALE_X10 = 10
const GAS_SCALE_X3 = 3
const TRANSFER_MIN_PROBE_LIMIT = 10000

fun makeTinyForwardPayload(): slice {
    return beginCell().storeUint(0, 1).toSlice();
}

fun makeRichCustomPayload(): cell {
    return beginCell().storeUint(0x123456789abcdef, 128).endCell();
}

fun makeRichForwardPayload(): slice {
    return beginCell().storeMaybeRef(makeRichCustomPayload()).endCell().beginParse();
}

fun buildTransferBody(
    jettonAmount: int,
    forwardTonAmount: int,
    transferRecipient: address,
    sendExcessesTo: address,
    forwardPayload: slice,
    customPayload: cell? = null,
): AskToTransfer {
    return AskToTransfer {
        queryId: 0,
        jettonAmount,
        transferRecipient,
        sendExcessesTo,
        customPayload,
        forwardTonAmount,
        forwardPayload,
    };
}

fun estimateMessageForwardFee<T>(body: T, dest: address, bounce: bool, value: coins): coins {
    val estMsg = createMessage({
        body,
        dest,
        bounce,
        value,
    });

    return estMsg.estimateFeeWithoutSending(SEND_MODE_REGULAR);
}

fun estimateBodyForwardFee<T>(body: T): coins {
    return estimateMessageForwardFee(body, randomAddress(), true, ton("1"));
}

fun sendRawTransferPayload(
    from: address,
    wallet: address,
    body: cell,
    value: int = DEFAULT_TRANSFER_MSG_VALUE,
): SendResultList {
    val outMessage = createMessage({
        bounce: true,
        value,
        dest: wallet,
        body,
    });
    return net.send(from, outMessage);
}

fun userWallet(minter: JettonMinter, userAddress: address): JettonWallet {
    val address = minter.getWalletAddress(userAddress);
    return JettonWallet { address };
}

fun walletStateInit(storage: WalletStorage): ContractState {
    return ContractState {
        code: build("JettonWallet"),
        data: storage.toCell(),
    };
}

fun deployWalletFromStorage(
    storage: WalletStorage,
    from: address,
    value: coins = ton("10"),
): JettonWallet {
    val init = storage.jettonBalance == 0
        ? calcDeployedJettonWallet(
            storage.ownerAddress,
            storage.minterAddress,
            build("JettonWallet"),
        )
        : AutoDeployAddress { stateInit: walletStateInit(storage) };
    val address = init.calculateAddress();

    val deployResult = net.send(
        from,
        createMessage({
            bounce: false,
            value,
            dest: init,
            body: TopUpTons {},
        }),
    );
    expect(deployResult).toHaveSuccessfulDeploy({ to: address });

    return JettonWallet { address };
}

fun setupDirectWallet(initialBalance: coins = 0): (JettonWallet, Treasury, Treasury, Treasury) {
    val owner = testing.treasury("direct-wallet-owner");
    val minter = testing.treasury("direct-wallet-minter");
    val outsider = testing.treasury("direct-wallet-outsider");
    val wallet = deployWalletFromStorage(
        WalletStorage {
            jettonBalance: 0,
            ownerAddress: owner.address,
            minterAddress: minter.address,
        },
        owner.address,
    );

    if (initialBalance) {
        val topUp = net.send(
            minter.address,
            createMessage({
                bounce: true,
                value: ton("0.3"),
                dest: wallet.address,
                body: InternalTransferStep {
                    queryId: 0,
                    jettonAmount: initialBalance,
                    transferInitiator: null,
                    sendExcessesTo: null,
                    forwardTonAmount: 0,
                    forwardPayload: makeTinyForwardPayload(),
                },
            }),
        );
        expect(topUp).toHaveSuccessfulTx<InternalTransferStep>({
            from: minter.address,
            to: wallet.address,
        });
    }

    return (wallet, owner, minter, outsider);
}

fun findSendResultForAccount(res: SendResultList, account: address): SendResult? {
    var i = 0;
    while (i < res.size()) {
        val item = res.at(i);
        if (item.tx.load().getAccountAddress() == account) {
            return item;
        }
        i += 1;
    }

    return null;
}

fun hasSetCodeAction(res: SendResultList, account: address): bool {
    val item = findSendResultForAccount(res, account);
    if (item == null) {
        return false;
    }

    val actions = item.allOutActions();
    var i = 0;
    while (i < actions.size()) {
        if (actions.at(i) is TlbOutActionSetCode) {
            return true;
        }
        i += 1;
    }

    return false;
}

fun setupTest(initialSupply: int = 0): (JettonMinter, Treasury, Treasury) {
    val deployer = testing.treasury("deployer");
    val notDeployer = testing.treasury("not-deployer");

    val minter = JettonMinter.fromStorage(MinterStorage {
        totalSupply: initialSupply,
        adminAddress: deployer.address,
        metadata: buildTestMetadata("link"),
        nextAdminAddress: null,
    });

    val transactions = minter.sendDeploy(deployer.address, { value: ton("10") });
    expect(transactions).toHaveSuccessfulDeploy({ to: minter.address });

    return (minter, deployer, notDeployer);
}

fun mintTo(minter: JettonMinter, admin: Treasury, recipient: address, amount: coins): JettonWallet {
    val supplyBefore = minter.getTotalSupply();
    val wallet = userWallet(minter, recipient);

    val mintResult = minter.sendMint(admin.address, recipient, amount, ton("0.05"), ton("0.3"));
    expect(mintResult).toHaveSuccessfulTx<MintNewJettons>({
        from: admin.address,
        to: minter.address,
    });

    expect(wallet.getJettonBalance()).toEqual(amount);
    expect(minter.getTotalSupply()).toEqual(supplyBefore + amount);
    return wallet;
}
