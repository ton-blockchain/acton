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
    return WalletV5 { address, stateInit, toShard };
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

struct ActionsList {
    outActions: array<SendAction>
    extraActions: array<ExtraAction>
}

/// Build a typed OutActionList chain that matches the contract's `Cell<OutActionWithSendMessageOnly>?` ABI.
/// Returns null when there are no actions; otherwise a cell whose contents are the head OutAction
/// with `prev` linking back through the chain to a terminating empty cell.
fun buildOutActionsCell(specs: array<SendAction>): Cell<OutActionWithSendMessageOnly>? {
    if (specs.size() == 0) {
        return null;
    }
    var prev: cell = createEmptyCell();
    var i = specs.size() - 1;
    while (i > 0) {
        val node = OutActionWithSendMessageOnly {
            prev: prev as Cell<OutActionWithSendMessageOnlyOrEmpty>,
            sendMode: specs.get(i).mode as uint8,
            outMsg: specs.get(i).messageCell,
        };
        prev = node.toCell();
        i -= 1;
    }
    val head = OutActionWithSendMessageOnly {
        prev: prev as Cell<OutActionWithSendMessageOnlyOrEmpty>,
        sendMode: specs.get(0).mode as uint8,
        outMsg: specs.get(0).messageCell,
    };
    return head.toCell();
}

/// Build the snake-format slice that the contract reads from the `extraActions: SnakedExtraActions` field.
/// Empty array returns an empty slice; the contract gates reading via `hasExtraActions`.
fun buildExtraActionsSlice(actions: array<ExtraAction>): SnakedExtraActions {
    val n = actions.size();
    if (n == 0) {
        return createEmptyCell().beginParse() as SnakedExtraActions;
    }
    var b: builder = beginCell().storeSlice(actions.get(0).toCell().beginParse());
    if (n > 1) {
        var snaked: cell = actions.get(n - 1).toCell();
        var j = n - 2;
        while (j >= 1) {
            snaked = beginCell()
                .storeSlice(actions.get(j).toCell().beginParse())
                .storeRef(snaked)
                .endCell();
            j -= 1;
        }
        b = b.storeRef(snaked);
    }
    return b.endCell().beginParse() as SnakedExtraActions;
}

fun WalletV5.sendExtensionActionBody(
    self,
    from: address,
    actions: ActionsList,
    queryId: uint64 = 0,
    config: SendParams = {},
): SendResultList {
    return self.sendExtensionActionRequest(
        from,
        queryId,
        buildOutActionsCell(actions.outActions),
        actions.extraActions.size() != 0,
        buildExtraActionsSlice(actions.extraActions),
        config,
    );
}

/// Send a pre-built actions body cell as the body of an ExtensionActionRequest.
/// Use for tests that inject malformed/edge-case action bodies.
fun WalletV5.sendExtensionActionRaw(
    self,
    from: address,
    actionsBody: cell,
    queryId: uint64 = 0,
    config: SendParams = {},
): SendResultList {
    val body = beginCell()
        .storeUint(0x6578746E, 32)
        .storeUint(queryId, 64)
        .storeSlice(actionsBody.beginParse())
        .endCell();
    return self.sendAny(from, body, config);
}

struct SignedBody {
    payload: InternalSignedRequest
    signature: bytes64
}

fun WalletV5.sendSignedBody(
    self,
    from: address,
    body: SignedBody,
    config: SendParams = {},
): SendResultList {
    return self.sendAny(from, body.toCell(), config);
}

fun buildSignedPayload(
    walletId: int,
    seqno: int,
    actions: ActionsList,
    validUntil: int? = null,
): InternalSignedRequest {
    if (validUntil == null) {
        validUntil = blockchain.now() + 1000;
    }
    return InternalSignedRequest {
        walletId,
        validUntil,
        seqno,
        outActions: buildOutActionsCell(actions.outActions) as unknown as cell?,
        hasExtraActions: actions.extraActions.size() != 0,
        extraActions: buildExtraActionsSlice(actions.extraActions),
    };
}

fun createSignedBody(
    keyPair: KeyPair,
    walletId: int,
    seqno: int,
    actions: ActionsList,
    validUntil: int? = null,
): SignedBody {
    val payload = buildSignedPayload(walletId, seqno, actions, validUntil);
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

struct ExternalSignedBody {
    payload: ExternalSignedRequest
    signature: bytes64
}

fun buildExternalSignedPayload(
    walletId: int,
    seqno: int,
    actions: ActionsList,
    validUntil: int? = null,
): ExternalSignedRequest {
    if (validUntil == null) {
        validUntil = blockchain.now() + 1000;
    }
    return ExternalSignedRequest {
        walletId,
        validUntil,
        seqno,
        outActions: buildOutActionsCell(actions.outActions) as unknown as cell?,
        hasExtraActions: actions.extraActions.size() != 0,
        extraActions: buildExtraActionsSlice(actions.extraActions),
    };
}

fun createExternalSignedBody(
    keyPair: KeyPair,
    walletId: int,
    seqno: int,
    actions: ActionsList,
    validUntil: int? = null,
): ExternalSignedBody {
    val payload = buildExternalSignedPayload(walletId, seqno, actions, validUntil);
    val signature = crypto.sign(keyPair.privateKey, payload.toCell()) as bytes64;
    return ExternalSignedBody { payload, signature };
}

fun WalletV5.sendExternalSigned(self, body: ExternalSignedBody): ExternalSendResult {
    return self.sendAnyExternal(body.toCell());
}

fun WalletV5.sendExternalRaw(self, body: cell): ExternalSendResult {
    return self.sendAnyExternal(body);
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
    val signedBody = createSignedBody(
        keyPair,
        DEFAULT_SUBWALLET_ID,
        seqno,
        ActionsList {
            outActions: [],
            extraActions,
        },
    );
    return walletV5.sendSignedBody(sender, signedBody, { value: grams("0.1") });
}
