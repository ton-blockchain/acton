import "@stdlib/strings"
import "errors"
import "fees-management"
import "messages"
import "types"

contract NftCollection {
    author: "The Tolk Team"
    version: "1.0.0"
    description: "Standard NFT Collection. TEP-62 and TEP-64 compliant"
    incomingMessages: AllowedMessageToNftCollection
    storage: NftCollectionStorage
}

fun deployNftItem(
    itemIndex: int,
    nftItemCode: cell,
    attachTonAmount: coins,
    initParams: Cell<NftItemInitAtDeployment>,
) {
    val deployMsg = createMessage({
        bounce: BounceMode.Only256BitsOfBody,
        dest: calcDeployedNftItem(itemIndex, contract.getAddress(), nftItemCode),
        value: attachTonAmount,
        body: initParams,
    });
    deployMsg.send(SEND_MODE_PAY_FEES_SEPARATELY);
}

type AllowedMessageToNftCollection =
    | RequestRoyaltyParams
    | DeployNft
    | BatchDeployNfts
    | ChangeCollectionAdmin

fun onInternalMessage(in: InMessage) {
    val msg = lazy AllowedMessageToNftCollection.fromSlice(in.body);

    match (msg) {
        DeployNft => {
            var storage = lazy NftCollectionStorage.load();
            assert (in.senderAddress == storage.adminAddress) throw Errors.NotFromAdmin;
            assert (msg.itemIndex <= storage.nextItemIndex) throw Errors.InvalidItemIndex;

            val isLast = msg.itemIndex == storage.nextItemIndex;
            deployNftItem(msg.itemIndex, storage.nftItemCode, msg.attachTonAmount, msg.initParams);
            if (isLast) {
                storage.nextItemIndex += 1;
                storage.save();
            }
        }

        RequestRoyaltyParams => {
            val storage = lazy NftCollectionStorage.load();
            val respondMsg = createMessage({
                bounce: BounceMode.NoBounce,
                dest: in.senderAddress,
                value: 0,
                body: ResponseRoyaltyParams {
                    queryId: msg.queryId,
                    royaltyParams: storage.royaltyParams.load(),
                },
            });
            respondMsg.send(SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE);
        }

        BatchDeployNfts => {
            var storage = lazy NftCollectionStorage.load();
            assert (in.senderAddress == storage.adminAddress) throw Errors.NotFromAdmin;

            var counter = 0;
            var r = msg.deployList.findFirst();
            while (r.isFound) {
                counter += 1;
                assert (counter < BATCH_SIZE_LIMIT) throw Errors.BatchLimitExceeded;

                // Error cast is used to replicate exit codes from FunC version
                val itemIndex = r.getKey();
                assert (
                    itemIndex <= storage.nextItemIndex
                ) throw (Errors.NotFromAdmin as int) + counter;

                val dictItem = r.loadValue();
                deployNftItem(
                    itemIndex,
                    storage.nftItemCode,
                    dictItem.attachTonAmount,
                    dictItem.initParams,
                );
                if (itemIndex == storage.nextItemIndex) {
                    storage.nextItemIndex += 1;
                }

                r = msg.deployList.iterateNext(r);
            }
            storage.save();
        }

        ChangeCollectionAdmin => {
            var storage = lazy NftCollectionStorage.load();
            assert (in.senderAddress == storage.adminAddress) throw Errors.NotFromAdmin;
            storage.adminAddress = msg.newAdminAddress;
            storage.save();
        }

        else => {
            // ignore empty messages, "wrong opcode" for others
            assert (in.body.isEmpty()) throw 0xFFFF;
        }
    }
}

fun onBouncedMessage(_in: InMessageBounced) {
    // For receiving funds from possibly bounced deployment of NFT
}

get fun get_collection_data(): CollectionDataReply {
    val storage = lazy NftCollectionStorage.load();
    val content = lazy storage.content.load();

    return {
        nextItemIndex: storage.nextItemIndex,
        collectionMetadata: content.collectionMetadata,
        adminAddress: storage.adminAddress,
    };
}

get fun get_nft_address_by_index(itemIndex: int): address {
    val storage = lazy NftCollectionStorage.load();
    val nftDeployed = calcDeployedNftItem(itemIndex, contract.getAddress(), storage.nftItemCode);
    return nftDeployed.calculateAddress();
}

get fun royalty_params(): RoyaltyParams {
    val storage = lazy NftCollectionStorage.load();
    return storage.royaltyParams.load();
}

get fun get_nft_content(
    _itemIndex: int,
    individualNftContent: string,
): Cell<OffchainMetadataReply> {
    val storage = lazy NftCollectionStorage.load();
    val content = lazy storage.content.load();

    // construct a response from "common content" and "individual content";
    // for example:
    // common content = "https://site.org/my-collection/"
    // individual nft = "my-picture-123.png"  (probably, a long string, snake-encoded)
    // result         = concatenation
    var b = StringBuilder.create();
    b.append(content.commonContent);
    b.append(individualNftContent);
    return OffchainMetadataReply { string: b.build() }.toCell();
}
