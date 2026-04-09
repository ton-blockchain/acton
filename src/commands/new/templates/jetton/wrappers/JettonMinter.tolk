import "@acton/build/build"

import "@acton/emulation/network"
import "@contracts/messages"
import "@contracts/storage"

struct JettonMinter {
    address: address
    init: ContractState
}

fun JettonMinter.sendDiscovery(
    self,
    from: address,
    ownerAddress: address,
    includeOwnerAddress: bool,
    value: coins = ton("0.1"),
): SendResultList {
    val discoveryMsg = createMessage({
        bounce: false,
        value,
        dest: self.address,
        body: RequestWalletAddress {
            queryId: 0,
            ownerAddress,
            includeOwnerAddress,
        },
    });
    return net.send(from, discoveryMsg);
}

fun JettonMinter.sendChangeContent(self, from: address, newContent: cell): SendResultList {
    val changeMsg = createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: self.address,
        body: ChangeMinterContent {
            queryId: 0,
            newContent,
        },
    });
    return net.send(from, changeMsg);
}

fun JettonMinter.fromStorage(storage: MinterStorage): JettonMinter {
    val init = ContractState {
        code: build("jetton_minter"),
        data: storage.toCell(),
    };
    val addr = AutoDeployAddress { stateInit: init }.calculateAddress();
    return JettonMinter {
        address: addr,
        init,
    };
}

fun JettonMinter.sendDeploy(self, from: address, params: SendParams): SendResultList {
    val deployMsg = createMessage({
        bounce: false,
        value: params.value,
        dest: { workchain: 0, stateInit: self.init },
    });
    return net.send(from, deployMsg);
}

fun JettonMinter.sendMint(
    self,
    from: address,
    recipient: address,
    jettonAmount: int,
    forwardTonAmount: int,
    totalTonAmount: int,
): SendResultList {
    val mintMsg = createMessage({
        bounce: false,
        value: totalTonAmount + ton("0.015"),
        dest: self.address,
        body: MintNewJettons {
            queryId: 0,
            mintRecipient: recipient,
            tonAmount: totalTonAmount,
            internalTransferMsg: InternalTransferStep {
                queryId: 0,
                jettonAmount,
                transferInitiator: null,
                sendExcessesTo: self.address,
                forwardTonAmount,
                forwardPayload: createEmptySlice(),
            }.toCell(),
        },
    });
    return net.send(from, mintMsg);
}

fun JettonMinter.sendChangeAdmin(self, from: address, newAdminAddress: address): SendResultList {
    val changeMsg = createMessage({
        bounce: false,
        value: ton("0.05"),
        dest: self.address,
        body: ChangeMinterAdmin {
            queryId: 0,
            newAdminAddress,
        },
    });
    return net.send(from, changeMsg);
}

fun JettonMinter.getJettonData(self): JettonDataReply {
    return net.runGetMethod(self.address, "get_jetton_data");
}

fun JettonMinter.getTotalSupply(self): int {
    return self.getJettonData().totalSupply;
}

fun JettonMinter.getAdminAddress(self): address {
    return self.getJettonData().adminAddress;
}

fun JettonMinter.getContent(self): cell {
    return self.getJettonData().jettonContent;
}

fun JettonMinter.getWalletAddress(self, owner: address): address {
    return net.runGetMethod(self.address, "get_wallet_address", owner);
}
