import "errors"
import "fees-management"
import "messages"
import "types"

contract NftItem {
    author: "The Tolk Team"
    version: "1.0.0"
    description: "Standard NFT Item. TEP-62 and TEP-64 compliant"
    incomingMessages: AllowedMessageToNftItem
    storage: NftItemStorage
    storageAtDeployment: NftItemStorageNotInitialized
}

type AllowedMessageToNftItem =
    | AskToChangeOwnership
    | RequestStaticData

fun onInternalMessage(in: InMessage) {
    var loadingStorage = NftItemStorage.startLoading();
    if (loadingStorage.isNotInitialized()) {
        assert (
            in.senderAddress == loadingStorage.collectionAddress
        ) throw Errors.NotFromCollection;

        // using a message from collection, convert "uninitialized" to "initialized" state
        val initParams = NftItemInitAtDeployment.fromSlice(in.body);
        val storage: NftItemStorage = {
            itemIndex: loadingStorage.itemIndex,
            collectionAddress: loadingStorage.collectionAddress,
            ownerAddress: initParams.ownerAddress,
            content: initParams.content,
        };
        storage.save();
        return;
    }

    var storage = loadingStorage.endLoading();

    val msg = lazy AllowedMessageToNftItem.fromSlice(in.body);

    match (msg) {
        AskToChangeOwnership => {
            assert (in.senderAddress == storage.ownerAddress) throw Errors.NotFromOwner;

            assert (msg.newOwnerAddress.getWorkchain() == BASECHAIN) throw Errors.InvalidWorkchain;

            val fwdFee = in.originalForwardFee;
            var restAmount = contract.getOriginalBalance() - MIN_TONS_FOR_STORAGE;
            if (msg.forwardTonAmount) {
                restAmount -= (msg.forwardTonAmount + fwdFee);
            }
            if (msg.sendExcessesTo != null) {
                assert (
                    msg.sendExcessesTo.getWorkchain() == BASECHAIN
                ) throw Errors.InvalidWorkchain;
                restAmount -= fwdFee;
            }

            // base nft spends fixed amount of gas, will not check for response
            assert (restAmount >= 0) throw Errors.TooSmallRestAmount;

            if (msg.forwardTonAmount) {
                val ownershipMsg = createMessage({
                    bounce: BounceMode.NoBounce,
                    dest: msg.newOwnerAddress,
                    value: msg.forwardTonAmount,
                    body: NotificationForNewOwner {
                        queryId: msg.queryId,
                        oldOwnerAddress: storage.ownerAddress,
                        payload: msg.forwardPayload,
                    },
                });
                ownershipMsg.send(SEND_MODE_PAY_FEES_SEPARATELY);
            }
            if (msg.sendExcessesTo != null) {
                val excessesMsg = createMessage({
                    bounce: BounceMode.NoBounce,
                    dest: msg.sendExcessesTo,
                    value: restAmount,
                    body: ReturnExcessesBack { queryId: msg.queryId },
                });
                excessesMsg.send(SEND_MODE_PAY_FEES_SEPARATELY);
            }

            storage.ownerAddress = msg.newOwnerAddress;
            storage.save();
        }

        RequestStaticData => {
            val respondMsg = createMessage({
                bounce: BounceMode.NoBounce,
                dest: in.senderAddress,
                value: 0,
                // why ever, in FunC implementation, item index was encoded as 256-bit (not 64-bit),
                // do the same here to pass FunC tests;
                // but since a response becomes too long (64 + 256 + address), the compiler will create a ref;
                // force the compiler not to create a ref, because it's guaranteed that with
                // value (coins) = 0 it will nevertheless fit into a message cell directly
                body: UnsafeBodyNoRef {
                    forceInline: ResponseStaticData {
                        queryId: msg.queryId,
                        itemIndex: storage.itemIndex as uint256,
                        collectionAddress: storage.collectionAddress,
                    },
                },
            });
            respondMsg.send(SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE);
        }

        else => {
            // ignore empty messages, "wrong opcode" for others
            assert (in.body.isEmpty()) throw 0xFFFF;
        }
    }
}

get fun get_nft_data(): NftDataReply {
    var loadingStorage = NftItemStorage.startLoading();
    if (loadingStorage.isNotInitialized()) {
        return {
            isInitialized: false,
            itemIndex: loadingStorage.itemIndex,
            collectionAddress: loadingStorage.collectionAddress,
        };
    }

    val storage = loadingStorage.endLoading();
    return {
        isInitialized: true,
        itemIndex: storage.itemIndex,
        collectionAddress: storage.collectionAddress,
        ownerAddress: storage.ownerAddress,
        content: storage.content,
    };
}
