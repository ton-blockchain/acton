/// TON NFT v1 item interface for standard and simple-schema variants.
contract NftV1Item {
    author: "TON Core"
    version: "1"
    description: "TON NFT v1 Item is the contract for one TEP-62 collectible, holding its owner and content within a parent collection."
    storage: NftV1ItemStorage
    storageAtDeployment: NftV1ItemDeploymentStorage
    incomingMessages: NftV1ItemIncomingMessage
    outgoingMessages: NftV1ItemOutgoingMessage
}

/// Deployment storage present before the collection initializes the item.
struct NftV1ItemDeploymentStorage {
    /// NFT item index stored as uint64 in the data cell.
    itemIndex: uint64
    /// Collection address authorized to initialize this item.
    collectionAddress: address
}

/// Initialized storage for the standard NFT item variant.
struct NftV1ItemStorage {
    /// NFT item index stored as uint64 in the data cell.
    itemIndex: uint64
    /// Collection address used for static-data reports.
    collectionAddress: address
    /// Current owner address authorized to transfer this item.
    ownerAddress: address
    /// Referenced individual item content cell.
    content: cell
}

/// Collection-to-item initialization body used while storage is uninitialized.
struct NftV1ItemInit {
    /// Initial owner address stored by the item.
    ownerAddress: address
    /// Referenced individual item content cell.
    content: cell
}

/// Owner-originated TEP-62 ownership transfer request.
struct (0x5fcc3d14) NftV1Transfer {
    /// Caller-selected query id copied into outgoing responses.
    queryId: uint64
    /// New owner address; source restricts it to basechain.
    newOwnerAddress: address
    /// Optional response destination for `excesses`; `addr_none` means no response.
    responseDestination: address?
    /// Optional referenced custom payload parsed for TEP-62 layout compatibility.
    customPayload: cell?
    /// Nanotons forwarded to the new owner notification.
    forwardTonAmount: coins
    /// Raw TEP-62 `Either Cell ^Cell` forward payload tail.
    forwardPayload: RemainingBitsAndRefs
}

/// Request for static item data emitted back to the caller.
struct (0x2fcb26a2) NftV1GetStaticData {
    /// Caller-selected query id copied into `report_static_data`.
    queryId: uint64
}

/// Ownership assignment notification sent to the new owner.
struct (0x05138d91) NftV1OwnershipAssigned {
    /// Query id copied from the transfer request.
    queryId: uint64
    /// Previous owner address.
    previousOwnerAddress: address
    /// Raw forwarded payload copied from the transfer request.
    forwardPayload: RemainingBitsAndRefs
}

/// Static-data response sent to the requester.
struct (0x8b771735) NftV1ReportStaticData {
    /// Query id copied from `get_static_data`.
    queryId: uint64
    /// NFT item index encoded as uint256 by the reference source.
    itemIndex: uint256
    /// Collection address stored by this item.
    collectionAddress: address
}

/// Excess-value response sent after transfer or editorship transfer.
struct (0xd53276db) NftV1Excesses {
    /// Query id copied from the request that produced the refund.
    queryId: uint64
}

/// Internal message bodies accepted by standard NFT v1 item variants.
type NftV1ItemIncomingMessage =
    | NftV1Transfer
    | NftV1GetStaticData

/// Internal message bodies emitted by standard NFT v1 item variants.
type NftV1ItemOutgoingMessage =
    | NftV1OwnershipAssigned
    | NftV1ReportStaticData
    | NftV1Excesses

/// Structured result returned by `get_nft_data`.
struct GetNftV1DataReply {
    /// True when runtime storage includes owner and content fields.
    isInitialized: bool
    /// NFT item index returned as a TVM stack integer.
    itemIndex: int
    /// Collection address stored by the item.
    collectionAddress: address
    /// Current owner address; null while storage is uninitialized.
    ownerAddress: address?
    /// Referenced individual content cell; null while storage is uninitialized.
    content: cell?
}

/// NFT v1 item exit-code catalog from the reference FunC sources.
enum ItemErrors {
    /// Address workchain is not basechain.
    InvalidWorkchain = 333
    /// Transfer sender is not the current owner.
    UnauthorizedOwner = 401
    /// Remaining balance after storage, forward amount, and fees would be negative.
    InsufficientRestAmount = 402
    /// Initialization sender is not the stored collection address.
    UnauthorizedCollectionInit = 405
    /// Forward payload does not contain the required TEP-62 `Either` tag in v1.03.
    InvalidForwardPayload = 708
    /// Incoming internal message opcode is not supported by the initialized item.
    UnsupportedOperation = 65535
}

/// Return item initialization flag, index, collection, owner, and content.
get fun get_nft_data(): GetNftV1DataReply {
    throw 0;
}
