import "@stdlib/exotic-cells"
import "@acton/build"
import "@acton/crypto"
import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"
import "@acton/types/message"
import "@contracts/walletv5/messages"
import "@contracts/walletv5/storage"

const ERROR_WALLET_V5_INIT_REQUIRED = 666

fun constructAddressFromWorkchainAndHash(wc: int8, hash: uint256): address {
    return beginCell().storeUint(4, 3).storeUint(wc, 8).storeUint(hash, 256).toSlice() as address;
}

struct WalletV5Contract {
    address: address
    init: ContractState? = null
}

// Deploys code as a library and returns library cell
fun WalletV5Contract.deployCodeLib(): cell {
    val realCode = build("WalletV5");
    testing.registerLibrary(realCode);
    return realCode.toLibraryReference();
}

fun WalletV5Contract.fromStorage(storage: Storage) {
    val init = ContractState {
        code: WalletV5Contract.deployCodeLib(),
        data: storage.toCell(),
    };
    val address = AutoDeployAddress { stateInit: init }.calculateAddress();
    return WalletV5Contract { address, init };
}

fun WalletV5Contract.deploy(self, from: address, config: SendParams = {}): SendResultList {
    if (self.init == null) {
        throw ERROR_WALLET_V5_INIT_REQUIRED;
    }
    val msgDeploy = createMessage({
        bounce: config.bounce,
        value: config.value,
        dest: { stateInit: self.init },
    });
    return net.send(from, msgDeploy);
}

fun WalletV5Contract.getSeqno(self): int {
    return net.runGetMethod(self.address, "seqno");
}

fun WalletV5Contract.getPublicKey(self): int {
    return net.runGetMethod(self.address, "get_public_key");
}

fun WalletV5Contract.getSubwalletId(self): int {
    return net.runGetMethod(self.address, "get_subwallet_id");
}

fun WalletV5Contract.isSignatureAllowed(self): bool {
    return net.runGetMethod(self.address, "is_signature_allowed");
}

fun WalletV5Contract.getExtensions(self): map<uint256, bool> {
    return net.runGetMethod(self.address, "get_extensions");
}

fun WalletV5Contract.getExtensionsArray(self): array<address> {
    val ext: map<uint256, bool> = net.runGetMethod(self.address, "get_extensions");
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

fun WalletV5Contract.sendExtensionActionRequest(
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
    val msgExtActRequest = createMessage({
        bounce: config.bounce,
        value: config.value,
        dest: self.address,
        body,
    });
    return net.send(from, msgExtActRequest);
}

fun WalletV5Contract.sendSignedBody(
    self,
    from: address,
    body: SignedBody,
    config: SendParams = {},
): SendResultList {
    val msgWithSignedBody = createMessage({
        bounce: config.bounce,
        value: config.value,
        dest: self.address,
        body,
    });
    return net.send(from, msgWithSignedBody);
}

fun WalletV5Contract.sendAny(
    self,
    from: address,
    body: cell,
    config: SendParams = {},
): SendResultList {
    val msgAny = createMessage({
        bounce: config.bounce,
        value: config.value,
        dest: self.address,
        body,
    });
    return net.send(from, msgAny);
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

// Pack out actions (C5 format) and extra actions (snaked format) into a single cell.
// Resulting cell format:
//   outActions: Maybe<ref(OutActionsList)>
//   hasExtraActions: bool
//   extraActions: SnakedExtraActions (first inline, rest in refs)
fun ActionsList.packToBuilder(self, mutate b: builder) {
    // Pack out actions as Maybe ref to C5 action list
    if (self.outActions.size() == 0) {
        b.storeUint(0, 1);
    } else {
        b.storeUint(1, 1);
        // Build reversed linked list (last action first in chain)
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

    // Pack extra actions in snaked format
    if (self.extraActions.size() == 0) {
        b.storeUint(0, 1);
    } else {
        b.storeUint(1, 1);
        // First extra action inline
        b.storeSlice(self.extraActions.get(0).toCell().beginParse());
        // Rest in snaked refs
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

// Helper: create a send message out action
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

// Create a signed internal message body (opcode 0x73696E74)

struct (0x73696E74) SignedPayload {
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
    // val payload = beginCell()
    //     .storeUint(0x73696E74, 32)
    //     .storeUint(subwalletId, 32)
    //     .storeUint(blockchain.now() + 1000, 32)
    //     .storeUint(seqno, 32)
    //     .storeSlice(actionsBody.beginParse())
    //     .endCell();

    val payload = SignedPayload {
        subwalletId,
        now: blockchain.now() + 1000,
        seqno,
        actions,
    };

    val signature = crypto.sign(keyPair.privateKey, payload.toCell()) as bytes64;

    return SignedBody { payload, signature };
}

// Deploy wallet from storage
fun deployWallet(storage: Storage): (WalletV5Contract, Treasury) {
    val deployer = testing.treasury("deployer");
    val walletV5 = WalletV5Contract.fromStorage(storage);

    val deployResult = walletV5.deploy(deployer.address, { value: ton("5"), bounce: false });

    expect(deployResult).toHaveSuccessfulDeploy({ to: walletV5.address });

    return (walletV5, deployer);
}

// Deploy wallet with keypair, no extensions
fun deployWalletWithKey(keyPair: KeyPair): (WalletV5Contract, Treasury) {
    val storage = Storage {
        isSignatureAllowed: true,
        seqno: 0,
        subwalletId: DEFAULT_SUBWALLET_ID,
        publicKey: keyPair.publicKey,
        extensions: [],
    };
    return deployWallet(storage);
}

// External signed message helpers (opcode 0x7369676E)

struct (0x7369676E) ExternalSignedPayload {
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

fun WalletV5Contract.sendExternalSigned(self, body: ExternalSignedBody): SendResultList? {
    val msg = net.createExternalMessage(self.address, body);
    return net.sendExternal(msg);
}

fun WalletV5Contract.sendExternalRaw(self, body: cell): SendResultList? {
    val msg = net.createExternalMessage(self.address, body);
    return net.sendExternal(msg);
}

// Add extension via signed internal message (like TS beforeEach + createBody)
fun addExtensionSigned(
    walletV5: WalletV5Contract,
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
    return walletV5.sendSignedBody(sender, signedBody, { value: ton("0.1") });
}
