import "../../../lib/emulation/network"
import "../../../lib/build/build"

import "../../contracts/messages"
import "../../contracts/storage"

struct JettonWallet {
    address: address
}

fun JettonWallet.getJettonBalance(self): coins {
    if (!net.isDeployed(self.address)) {
        return 0
    }

    val result: JettonWalletDataReply = net.runGetMethod(self.address, "get_wallet_data");
    return result.jettonBalance
}

fun JettonWallet.sendTransfer(self, from: address, value: int, jettonAmount: int, recepient: address, responseAddress: address, customPayload: cell?, forwardTonAmount: int, forwardPayload: cell?) {
    val msg = createMessage({
        bounce: true,
        value: value,
        dest: self.address,
        body: AskToTransfer {
            queryId: 0,
            jettonAmount: jettonAmount,
            transferRecipient: recepient,
            sendExcessesTo: responseAddress,
            customPayload: customPayload,
            forwardTonAmount: forwardTonAmount,
            forwardPayload: beginCell().storeMaybeRef(forwardPayload.toCell()).endCell().beginParse(),
        },
    });
    return net.send(from, msg, SEND_MODE_PAY_FEES_SEPARATELY)
}

fun JettonWallet.sendBurn(self, from: address, value: int, jettonAmount: int, responseAddress: address, customPayload: cell?) {
    val msg = createMessage({
        bounce: false,
        value: value,
        dest: self.address,
        body: AskToBurn {
            queryId: 0,
            jettonAmount: jettonAmount,
            sendExcessesTo: responseAddress,
            customPayload: customPayload,
        },
    });
    return net.send(from, msg, SEND_MODE_PAY_FEES_SEPARATELY)
}

struct JettonMinter {
    address: address
    init: ContractState
}

fun JettonMinter.sendDiscovery(self, from: address, ownerAddress: address, includeOwnerAddress: bool, value: coins = ton("0.1")) {
    val msg = createMessage({
        bounce: false,
        value,
        dest: self.address,
        body: RequestWalletAddress {
            queryId: 0,
            ownerAddress: ownerAddress,
            includeOwnerAddress: includeOwnerAddress,
        },
    });
    return net.send(from, msg, SEND_MODE_PAY_FEES_SEPARATELY)
}

fun JettonMinter.sendChangeContent(self, from: address, newContent: cell) {
    val msg = createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: self.address,
        body: ChangeMinterContent {
            queryId: 0,
            newContent: newContent,
        },
    });
    return net.send(from, msg, SEND_MODE_PAY_FEES_SEPARATELY)
}

fun JettonMinter.fromStorage(storage: MinterStorage) {
    val init = ContractState {
        code: build("jetton-minter"),
        data: storage.toCell(),
    };
    val addr = AutoDeployAddress { stateInit: init }.calculateAddress();
    return JettonMinter {
        address: addr, init
    }
}

fun JettonMinter.sendDeploy(self, params: SendParams) {
    val msg = createMessage({
        bounce: false,
        value: params.value,
        dest: {
            workchain: 0,
            stateInit: self.init,
        },
    });
    return net.send(net.treasury("treasury").address, msg, SEND_MODE_PAY_FEES_SEPARATELY)
}

fun JettonMinter.sendMint(self, from: address, recipient: address, jetton_amount: int, forward_ton_amount: int, total_ton_amount: int) {
    val msg = createMessage({
        bounce: false,
        value: total_ton_amount + ton("0.015"),
        dest: self.address,
        body: MintNewJettons {
            queryId: 0,
            mintRecipient: recipient,
            tonAmount: total_ton_amount,
            internalTransferMsg: InternalTransferStep {
                queryId: 0,
                jettonAmount: jetton_amount,
                transferInitiator: null,
                sendExcessesTo: self.address,
                forwardTonAmount: forward_ton_amount,
                forwardPayload: createEmptySlice(),
            }.toCell(),
        },
    });
    return net.send(from, msg, SEND_MODE_PAY_FEES_SEPARATELY)
}

fun JettonMinter.sendChangeAdmin(self, from: address, new_owner: address) {
    val msg = createMessage({
        bounce: false,
        value: ton("0.05"),
        dest: self.address,
        body: ChangeMinterAdmin {
            queryId: 0,
            newAdminAddress: new_owner,
        },
    });
    return net.send(from, msg, SEND_MODE_PAY_FEES_SEPARATELY)
}

fun JettonMinter.getJettonData(self): JettonDataReply {
    return net.runGetMethod(self.address, "get_jetton_data");
}

fun JettonMinter.getTotalSupply(self): int {
    return self.getJettonData().totalSupply
}

fun JettonMinter.getAdminAddress(self): address {
    return self.getJettonData().adminAddress
}

fun JettonMinter.getContent(self): cell {
    return self.getJettonData().jettonContent
}

fun JettonMinter.getWalletAddress(self, owner: address): address {
    return net.runGetMethod(self.address, "get_wallet_address", owner)
}
