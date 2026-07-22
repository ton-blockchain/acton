/// Getgems fixed-price NFT sale v3 interface for TON-only listings.
contract GetgemsNftFixpriceSaleV3 {
    author: "Getgems"
    version: "3.0"
    description: "Getgems fixed-price sale V3 is a marketplace escrow for buying one listed NFT at a fixed TON price."
    storage: GetgemsNftFixpriceSaleV3Storage
    incomingMessages: GetgemsNftFixpriceSaleV3IncomingMessage
    incomingExternal: GetgemsNftFixpriceSaleV3IncomingExternalMessage
    outgoingMessages: GetgemsNftFixpriceSaleV3OutgoingMessage
    forceAbiExport: GetgemsNftFixpriceSaleV3EmptyBuy
    thrownErrors: Errors
}

/// Fee distribution cell referenced from the root sale storage.
struct GetgemsNftFixpriceSaleV3Fees {
    /// Marketplace fee payout address used by every successful purchase.
    marketplaceFeeAddress: address
    /// Marketplace fee amount in nanotons.
    marketplaceFee: coins
    /// Royalty payout address; source skips royalty transfer when this is `addr_none`.
    royaltyAddress: address?
    /// Royalty amount in nanotons.
    royaltyAmount: coins
}

/// Persistent sale state; `nftOwnerAddress = addr_none` marks the pre-initialized deployment state.
struct GetgemsNftFixpriceSaleV3Storage {
    /// Completion flag stored as one bit; completed sales reject ordinary internal messages.
    isComplete: bool
    /// Unix timestamp supplied at deployment, stored as uint32.
    createdAt: uint32
    /// Marketplace address allowed to top up uninitialized sales, cancel sales, and use rescue resend.
    marketplaceAddress: address
    /// NFT item address authorized to initialize the sale through `ownership_assigned`.
    nftAddress: address
    /// Current NFT owner before purchase; null until the NFT callback initializes the sale.
    nftOwnerAddress: address?
    /// Full sale price in nanotons, excluding the required one-ton gas reserve.
    fullPrice: coins
    /// Referenced fee and royalty amounts paid during successful purchase.
    fees: Cell<GetgemsNftFixpriceSaleV3Fees>
    /// One-bit deployment flag allowing a single external message to clear itself.
    canDeployByExternal: bool
}

/// Empty internal message body; when initialized, source treats it as a buy request with query id zero.
struct GetgemsNftFixpriceSaleV3EmptyBuy {}

/// Opcode-zero internal body also reaches the buy branch; source ignores the parsed query id.
struct (0x00000000) GetgemsNftFixpriceSaleV3ZeroOpcodeBuy {
    /// Parsed before dispatch but not copied into the outgoing NFT transfer.
    queryId: uint64
}

/// NFT ownership callback accepted only from the configured NFT address before sale initialization.
struct (0x05138d91) GetgemsNftFixpriceSaleV3OwnershipAssigned {
    /// Query id supplied by the NFT transfer callback; source parses it but does not store it.
    queryId: uint64
    /// Previous owner address stored as the seller address.
    previousOwnerAddress: address
    /// Raw standard NFT callback tail; source leaves any remaining bits and refs unread.
    forwardPayload: RemainingBitsAndRefs
}

/// Internal command that only accepts attached coins and leaves sale state unchanged.
struct (0x00000001) GetgemsNftFixpriceSaleV3AcceptCoins {
    /// Caller-selected query id parsed before the no-op branch returns.
    queryId: uint64
}

/// Internal command that purchases the NFT when enough nanotons are attached.
struct (0x00000002) GetgemsNftFixpriceSaleV3Buy {
    /// Query id copied into the outgoing NFT transfer body.
    queryId: uint64
}

/// Internal command that cancels the sale and returns the NFT to the seller.
struct (0x00000003) GetgemsNftFixpriceSaleV3CancelSale {
    /// Query id copied into the outgoing NFT transfer body.
    queryId: uint64
}

/// Referenced rescue envelope used by the marketplace-only resend command.
struct GetgemsNftFixpriceSaleV3ResendEnvelope {
    /// Raw `send_raw_message` mode loaded from the envelope.
    mode: uint8
    /// Complete raw message cell passed to `send_raw_message`.
    rawMessage: cell
}

/// Marketplace-only rescue command accepted after completion to send one raw message.
struct (0x0000022b) GetgemsNftFixpriceSaleV3ResendRawMessage {
    /// Parsed but otherwise unused query id.
    queryId: uint64
    /// Referenced envelope containing send mode and raw message ref.
    resendEnvelope: Cell<GetgemsNftFixpriceSaleV3ResendEnvelope>
}

/// External-in body accepted only when storage `canDeployByExternal` is set.
struct GetgemsNftFixpriceSaleV3ExternalDeploy {
    /// Source ignores the external body, so this captures any supplied tail.
    ignoredBody: RemainingBitsAndRefs
}

/// NFT transfer message emitted to the NFT item on buy or cancel.
struct (0x5fcc3d14) GetgemsNftFixpriceSaleV3NftTransfer {
    /// Query id from the buy/cancel request, or zero for empty-body buys.
    queryId: uint64
    /// New NFT owner: buyer on purchase, previous owner on cancellation.
    newOwnerAddress: address
    /// Response destination mirrors the new owner in this source.
    responseAddress: address
    /// Always absent in this source.
    customPayload: cell?
    /// Zero on cancellation, or 0.03 TON on purchase.
    forwardTonAmount: coins
    /// Raw TEP-62 `Either Cell ^Cell` forward payload tail; source emits the empty left-cell tag.
    forwardPayload: RemainingBitsAndRefs
}

/// Internal message bodies accepted by Getgems fixed-price sale v3.
type GetgemsNftFixpriceSaleV3IncomingMessage =
    | GetgemsNftFixpriceSaleV3ZeroOpcodeBuy
    | GetgemsNftFixpriceSaleV3OwnershipAssigned
    | GetgemsNftFixpriceSaleV3AcceptCoins
    | GetgemsNftFixpriceSaleV3Buy
    | GetgemsNftFixpriceSaleV3CancelSale
    | GetgemsNftFixpriceSaleV3ResendRawMessage

/// External-in message bodies accepted by Getgems fixed-price sale v3.
type GetgemsNftFixpriceSaleV3IncomingExternalMessage = GetgemsNftFixpriceSaleV3ExternalDeploy

/// Outgoing message bodies emitted by Getgems fixed-price sale v3.
type GetgemsNftFixpriceSaleV3OutgoingMessage = GetgemsNftFixpriceSaleV3NftTransfer

/// Structured result returned by `get_sale_data`.
struct GetgemsNftFixpriceSaleV3SaleDataReply {
    /// Fixed magic `0x46495850`, ASCII `FIXP`.
    magic: uint64
    /// True after the sale is bought or cancelled.
    isComplete: bool
    /// Deployment timestamp loaded from uint32 storage.
    createdAt: uint32
    /// Marketplace address stored in sale state.
    marketplaceAddress: address
    /// NFT item address stored in sale state.
    nftAddress: address
    /// Seller address, or `addr_none` before the NFT ownership callback initializes the sale.
    nftOwnerAddress: any_address
    /// Full sale price in nanotons.
    fullPrice: coins
    /// Marketplace fee payout address.
    marketplaceFeeAddress: address
    /// Marketplace fee amount in nanotons.
    marketplaceFee: coins
    /// Royalty payout stack address, or `addr_none` when no royalty transfer should be sent.
    royaltyAddress: any_address
    /// Royalty amount in nanotons.
    royaltyAmount: coins
}

/// Source-level exit-code catalog for Getgems fixed-price sale v3.
enum Errors {
    /// Sale was already bought or cancelled.
    AlreadySoldOrCancelled = 404
    /// Purchase did not attach `fullPrice + 1 TON`.
    NotEnoughTonsForSale = 450
    /// Cancellation did not attach the required one-ton gas reserve.
    NotEnoughTonsForCancellation = 457
    /// Cancellation sender was neither seller nor marketplace.
    UnauthorizedCancellation = 458
    /// Uninitialized sale was messaged by an address other than marketplace or NFT.
    UnauthorizedInitiation = 500
    /// Initialization body opcode was not `ownership_assigned`.
    InvalidOpExpectedOwnershipAssigned = 501
    /// External deployment message arrived after the deployment flag was cleared.
    ExternalDeployNotAllowed = 65534
    /// Internal message opcode is not supported.
    UnknownOperation = 65535
}

/// Return fixed-price sale state, fee, and royalty data.
get fun get_sale_data(): GetgemsNftFixpriceSaleV3SaleDataReply {
    throw 0;
}
