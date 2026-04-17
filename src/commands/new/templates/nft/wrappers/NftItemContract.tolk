import "@acton/build/build"
import "@acton/emulation/network"
import "@acton/testing/assert"
import "@contracts/messages"
import "@contracts/types"

struct NftItemContract {
    address: address
    init: ContractState? = null
}

/// Creates a contract wrapper instance from the storage data
fun NftItemContract.fromStorage(storage: NftItemStorage): NftItemContract {
    val init = ContractState {
        code: build("NftItem"),
        data: storage.toCell(),
    };
    val address = AutoDeployAddress { stateInit: init }.calculateAddress();
    return NftItemContract { address, init };
}

/// Creates a contract wrapper instance from the address
fun NftItemContract.fromAddress(address: address): NftItemContract {
    return NftItemContract { address };
}

/// Deploys the contract to the blockchain
fun NftItemContract.deploy(self, from: address, config: SendParams = {}): SendResultList {
    if (self.init == null) {
        Assert.fail("Cannot deploy a contract created with 'fromAddress' because it lacks state init for deployment");
    }
    val deployMessage = createMessage({
        bounce: config.bounce,
        value: config.value,
        dest: { stateInit: self.init },
    });
    return net.send(from, deployMessage);
}

fun NftItemContract.sendAskToChangeOwnership(
    self,
    from: address,
    queryId: uint64,
    newOwnerAddress: address,
    sendExcessesTo: address?,
    customPayload: dict,
    forwardTonAmount: coins,
    forwardPayload: slice,
    config: SendParams = {},
): SendResultList {
    val (bodyBits, bodyRefs) = AskToChangeOwnership {
        queryId,
        newOwnerAddress,
        sendExcessesTo,
        customPayload,
        forwardTonAmount,
        forwardPayload: PayloadInline { value: createEmptySlice() },
    }.toCell()
        .beginParse()
        .remainingBitsAndRefsCount();

    val payload: Payload = (1023 - bodyBits) > forwardPayload.remainingBitsCount() &&
    (4 - bodyRefs) > forwardPayload.remainingRefsCount()
        ? PayloadInline { value: forwardPayload }
        : PayloadInRef { value: forwardPayload.toCell() };

    val askToChangeOwnershipMessage = createMessage({
        bounce: config.bounce,
        value: config.value,
        dest: self.address,
        body: AskToChangeOwnership {
            queryId,
            newOwnerAddress,
            sendExcessesTo,
            customPayload,
            forwardTonAmount,
            forwardPayload: payload,
        },
    });
    return net.send(from, askToChangeOwnershipMessage);
}

fun NftItemContract.sendRequestStaticData(
    self,
    from: address,
    queryId: uint64,
    config: SendParams = {},
): SendResultList {
    val requestStaticDataMessage = createMessage({
        bounce: config.bounce,
        value: config.value,
        dest: self.address,
        body: RequestStaticData { queryId },
    });
    return net.send(from, requestStaticDataMessage);
}

/// Send message to the contract with a custom body cell
fun NftItemContract.sendAny(
    self,
    from: address,
    body: cell,
    config: SendParams = {},
): SendResultList {
    val outgoingMessage = createMessage({
        bounce: config.bounce,
        value: config.value,
        dest: self.address,
        body,
    });
    return net.send(from, outgoingMessage);
}

fun NftItemContract.getNftData(self): NftDataReply {
    return net.runGetMethod(self.address, "get_nft_data");
}
