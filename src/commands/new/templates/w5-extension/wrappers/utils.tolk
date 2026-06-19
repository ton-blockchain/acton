import "@stdlib/exotic-cells"
import "@acton/build"
import "@acton/crypto"
import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"
import "@contracts/walletv5/messages"
import "@contracts/walletv5/storage"
import "WalletV5.gen"

fun constructAddressFromWorkchainAndHash(wc: int8, hash: uint256): address {
    return beginCell().storeUint(4, 3).storeUint(wc, 8).storeUint(hash, 256).toSlice() as address;
}

fun WalletV5.deployCodeLib(): cell {
    val realCode = build("WalletV5");
    testing.registerLibrary(realCode);
    return realCode.toLibraryReference();
}

fun WalletV5.fromLibraryStorage(
    storage: Storage,
    toShard: AddressShardingOptions? = null,
    workchain: int8 = BASECHAIN,
): WalletV5 {
    val stateInit = ContractState {
        code: WalletV5.deployCodeLib(),
        data: storage.toCell(),
    };
    val address = AutoDeployAddress { workchain, stateInit, toShard }.calculateAddress();
    return WalletV5 { address, stateInit };
}

fun WalletV5.getSeqno(self): int {
    return self.seqno();
}

fun WalletV5.getExtensionsArray(self): array<address> {
    val ext: map<uint256, bool> = self.getExtensions();
    val selfWorkchain = self.address.getWorkchain();
    var res: array<address> = [];
    var cur = ext.findFirst();
    while (cur.isFound) {
        val addr: address = constructAddressFromWorkchainAndHash(selfWorkchain, cur.getKey());
        res.push(addr);
        cur = ext.iterateNext(cur);
    }
    return res;
}

fun WalletV5.sendExtensionActionBody(
    self,
    from: address,
    actionsBody: cell,
    queryId: uint64 = 0,
    config: SendParams = {},
): SendResultList {
    val body = beginCell()
        .storeUint(0x6578746e, 32)
        .storeUint(queryId, 64)
        .storeSlice(actionsBody.beginParse())
        .endCell();
    return self.sendAny(from, body, config);
}

fun WalletV5.sendSignedBody(
    self,
    from: address,
    body: SignedBody,
    config: SendParams = {},
): SendResultList {
    return self.sendAny(from, body.toCell(), config);
}

const DEFAULT_SUBWALLET_ID = 0

fun defaultStorage(): Storage {
    return Storage {
        isSignatureAllowed: true,
        seqno: 0,
        subwalletId: DEFAULT_SUBWALLET_ID,
        publicKey: 0,
        extensions: [],
    };
}

struct SendAction {
    mode: int
    messageCell: cell
}

struct ActionsList {
    outActions: array<SendAction>
    extraActions: array<ExtraAction>
}

fun ActionsList.packToBuilder(self, mutate b: builder) {
    if (self.outActions.size() == 0) {
        b.storeUint(0, 1);
    } else {
        b.storeUint(1, 1);
        var c5 = createEmptyCell();
        var i = self.outActions.size() - 1;
        while (i >= 0) {
            val action = self.outActions.get(i);
            c5 = beginCell()
                .storeRef(c5)
                .storeUint(0x0ec3c86d, 32)
                .storeUint(action.mode, 8)
                .storeRef(action.messageCell)
                .endCell();
            i -= 1;
        }
        b.storeRef(c5);
    }

    if (self.extraActions.size() == 0) {
        b.storeUint(0, 1);
    } else {
        b.storeUint(1, 1);
        b.storeSlice(self.extraActions.get(0).toCell().beginParse());
        if (self.extraActions.size() > 1) {
            var snaked: cell = self.extraActions.get(self.extraActions.size() - 1).toCell();
            var j = self.extraActions.size() - 2;
            while (j >= 1) {
                snaked = beginCell()
                    .storeSlice(self.extraActions.get(j).toCell().beginParse())
                    .storeRef(snaked)
                    .endCell();
                j -= 1;
            }
            b.storeRef(snaked);
        }
    }
}

fun sendMsgAction(dest: address, value: coins): SendAction {
    val innerMsg = createMessage({
        bounce: false,
        value,
        dest,
    });
    return SendAction {
        mode: SEND_MODE_PAY_FEES_SEPARATELY | SEND_MODE_IGNORE_ERRORS,
        messageCell: innerMsg.messageCell,
    };
}

struct (0x73696e74) SignedPayload {
    subwalletId: int32
    now: uint32
    seqno: uint32
    actions: ActionsList
}

struct SignedBody {
    payload: SignedPayload
    signature: bytes64
}

fun createSignedBody(
    keyPair: KeyPair,
    subwalletId: int,
    seqno: int,
    actions: ActionsList,
): SignedBody {
    val payload = SignedPayload {
        subwalletId,
        now: blockchain.now() + 1000,
        seqno,
        actions,
    };

    val signature = crypto.sign(keyPair.privateKey, payload.toCell()) as bytes64;

    return SignedBody { payload, signature };
}

fun deployWallet(storage: Storage): (WalletV5, Treasury) {
    val deployer = testing.treasury("deployer");
    val walletV5 = WalletV5.fromLibraryStorage(storage);

    val deployResult = walletV5.deploy(deployer.address, { value: grams("5"), bounce: false });

    expect(deployResult).toHaveSuccessfulDeploy({ to: walletV5.address });

    return (walletV5, deployer);
}

fun deployWalletWithKey(keyPair: KeyPair): (WalletV5, Treasury) {
    val storage = Storage {
        isSignatureAllowed: true,
        seqno: 0,
        subwalletId: DEFAULT_SUBWALLET_ID,
        publicKey: keyPair.publicKey,
        extensions: [],
    };
    return deployWallet(storage);
}

struct (0x7369676e) ExternalSignedPayload {
    subwalletId: int32
    validUntil: uint32
    seqno: uint32
    actions: ActionsList
}

struct ExternalSignedBody {
    payload: ExternalSignedPayload
    signature: bytes64
}

fun createExternalSignedBody(
    keyPair: KeyPair,
    subwalletId: int,
    seqno: int,
    actions: ActionsList,
    validUntil: int? = null,
): ExternalSignedBody {
    if (validUntil == null) {
        validUntil = blockchain.now() + 1000;
    }
    val payload = ExternalSignedPayload { subwalletId, validUntil, seqno, actions };
    val signature = crypto.sign(keyPair.privateKey, payload.toCell()) as bytes64;
    return ExternalSignedBody { payload, signature };
}

fun addExtensionSigned(
    walletV5: WalletV5,
    sender: address,
    extensionAddr: address,
    keyPair: KeyPair,
    seqno: int,
): SendResultList {
    var extraActions: array<ExtraAction> = [];
    extraActions.push(AddExtensionExtraAction { addr: extensionAddr });
    val actionsBody = ActionsList {
        outActions: [],
        extraActions,
    };
    val signedBody = createSignedBody(keyPair, DEFAULT_SUBWALLET_ID, seqno, actionsBody);
    return walletV5.sendSignedBody(sender, signedBody, { value: grams("0.1") });
}
