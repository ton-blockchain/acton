/// TON NFT v1 collection interface with standard and editable variants.
contract NftV1Collection {
    author: "TON Core"
    version: "1"
    description: "TON NFT v1 Collection is the parent contract for an NFT set, responsible for minting items and publishing content and royalties."
    storage: NftV1CollectionStorage
    incomingMessages: NftV1CollectionIncomingMessage
    outgoingMessages: NftV1CollectionOutgoingMessage
    forceAbiExport: NftV1CollectionItemDeploymentMessage | NftV1CollectionBatchDeployItem
}

/// Persistent collection storage shared by standard and editable collection variants.
struct NftV1CollectionStorage {
    /// Admin address authorized to deploy items and mutate collection state.
    adminAddress: address
    /// Next item index stored as uint64.
    nextItemIndex: uint64
    /// Referenced collection content cell containing metadata and common-content refs.
    content: Cell<NftV1CollectionContent>
    /// Referenced NFT item code cell used to derive item state init.
    nftItemCode: cell
    /// Referenced royalty-params cell.
    royaltyParams: Cell<NftV1RoyaltyParams>
}

/// Collection content stored under the storage content reference.
struct NftV1CollectionContent {
    /// Referenced collection metadata cell returned by `get_collection_data`.
    collectionMetadata: cell
    /// Referenced common content prefix cell used by `get_nft_content`.
    commonContent: cell
}

/// TEP-66 royalty parameters stored in a referenced cell.
struct NftV1RoyaltyParams {
    /// Royalty numerator.
    numerator: uint16
    /// Royalty denominator.
    denominator: uint16
    /// Royalty payout address.
    royaltyAddress: address
}

/// Collection-to-item initialization body for standard item deployments.
struct NftV1CollectionItemInit {
    /// Initial owner address stored by the item.
    ownerAddress: address
    /// Referenced individual item content cell.
    content: cell
}

/// Value stored in the batch deploy dictionary.
struct NftV1CollectionBatchDeployItem {
    /// Nanotons attached to the item deployment message.
    attachTonAmount: coins
    /// Referenced item initialization body.
    initParams: Cell<NftV1CollectionItemInit>
}

/// Royalty-params request accepted by all TEP-66 collection variants.
struct (0x693d3950) NftV1GetRoyaltyParams {
    /// Caller-selected query id copied into `report_royalty_params`.
    queryId: uint64
}

/// Admin request to deploy a single NFT item.
struct (0x00000001) NftV1DeployItem {
    /// Admin-selected query id parsed by the source.
    queryId: uint64
    /// Item index to deploy; must not be greater than `nextItemIndex`.
    itemIndex: uint64
    /// Nanotons attached to the item deployment message.
    attachTonAmount: coins
    /// Referenced item initialization body.
    initParams: Cell<NftV1CollectionItemInit>
}

/// Admin request to deploy a dictionary of NFT items.
struct (0x00000002) NftV1BatchDeployItems {
    /// Admin-selected query id parsed by the source.
    queryId: uint64
    /// Dictionary keyed by uint64 item index with deployment amount and init params.
    deployList: map<uint64, NftV1CollectionBatchDeployItem>
}

/// Admin request to change the collection admin address.
struct (0x00000003) NftV1ChangeAdmin {
    /// Admin-selected query id parsed by the source.
    queryId: uint64
    /// Replacement admin address.
    newAdminAddress: address
}

/// Editable-collection request to replace collection content and royalty params.
struct (0x00000004) NftV1ChangeContent {
    /// Admin-selected query id parsed by the source.
    queryId: uint64
    /// Referenced replacement collection content cell.
    content: Cell<NftV1CollectionContent>
    /// Referenced replacement royalty params cell.
    royaltyParams: Cell<NftV1RoyaltyParams>
}

/// Item initialization body emitted by collection item deployment.
type NftV1CollectionItemDeploymentMessage = NftV1CollectionItemInit

/// Royalty-params response emitted by the collection.
struct (0xa8cb00ad) NftV1ReportRoyaltyParams {
    /// Query id copied from `get_royalty_params`.
    queryId: uint64
    /// Royalty numerator.
    numerator: uint16
    /// Royalty denominator.
    denominator: uint16
    /// Royalty payout address.
    royaltyAddress: address
}

/// Internal message bodies accepted by NFT v1 collection variants.
type NftV1CollectionIncomingMessage =
    | NftV1GetRoyaltyParams
    | NftV1DeployItem
    | NftV1BatchDeployItems
    | NftV1ChangeAdmin
    | NftV1ChangeContent

/// Internal message bodies emitted by NFT v1 collection variants.
type NftV1CollectionOutgoingMessage = NftV1ReportRoyaltyParams

/// Structured result returned by `get_collection_data`.
struct GetNftV1CollectionDataReply {
    /// Next item index returned as a TVM stack integer.
    nextItemIndex: int
    /// Referenced collection metadata cell.
    collectionMetadata: cell
    /// Current admin address.
    adminAddress: address
}

/// NFT v1 collection exit-code catalog from the reference FunC sources.
enum CollectionErrors {
    /// Batch deploy would exceed the action-list limit.
    BatchLimitExceeded = 399
    /// Sender is not the stored admin address.
    UnauthorizedAdmin = 401
    /// Single deploy item index is greater than `nextItemIndex`.
    InvalidItemIndex = 402
    /// Batch deploy item index errors are encoded as 403 plus the 1-based batch counter.
    InvalidBatchItemIndexBase = 403
    /// Incoming internal message opcode is not supported by the collection.
    UnsupportedOperation = 65535
}

/// Return next item index, collection metadata cell, and admin address.
get fun get_collection_data(): GetNftV1CollectionDataReply {
    throw 0;
}

/// Return deterministic NFT item address for an item index.
get fun get_nft_address_by_index(itemIndex: int): address {
    throw 0;
}

/// Return royalty numerator, denominator, and payout address.
get fun royalty_params(): NftV1RoyaltyParams {
    throw 0;
}

/// Return full NFT content by combining common collection content with individual item content.
get fun get_nft_content(itemIndex: int, individualContent: cell): cell {
    throw 0;
}
