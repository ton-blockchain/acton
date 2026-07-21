/// Jetton v1 wallet implementing the TEP-74 transfer, receive, burn, notify, and excesses flows.
contract JettonV1Wallet {
    author: "TON Core"
    version: "1"
    description: "Jetton v1 Wallet is a user's TEP-74 token account, holding one owner's jetton balance and handling token transfers and burns."
    storage: JettonV1WalletStorage
    incomingMessages: JettonV1WalletIncomingMessage
    outgoingMessages: JettonV1WalletOutgoingMessage
    thrownErrors: Errors
}

/// Persistent wallet state stored in the root data cell.
struct JettonV1WalletStorage {
    /// Jetton balance encoded as TL-B `Coins` in storage.
    balance: coins
    /// Owner address authorized to initiate outgoing transfers and burns.
    ownerAddress: address
    /// Jetton minter/master address authenticated for mint-originated internal transfers.
    jettonMasterAddress: address
    /// Referenced wallet code cell used to authenticate peer wallet senders.
    jettonWalletCode: cell
}

/// Owner-originated TEP-74 transfer request accepted by the wallet.
struct (0x0f8a7ea5) JettonTransfer {
    /// Caller-selected query id copied into the outgoing internal transfer.
    queryId: uint64
    /// Jetton amount to debit from this wallet, in base units.
    amount: coins
    /// Destination owner address whose deterministic wallet receives the transfer.
    destination: address
    /// Optional response address for an `Excesses` refund.
    responseDestination: address
    /// Optional referenced custom payload; v1 wallets parse it for layout compatibility.
    customPayload: cell?
    /// Nanotons forwarded to the destination owner notification.
    forwardTonAmount: coins
    /// Raw TEP-74 `Either Cell ^Cell` forward payload tail.
    forwardPayload: RemainingBitsAndRefs
}

/// Wallet-to-wallet or minter-to-wallet TEP-74 internal transfer body.
struct (0x178d4519) JettonInternalTransfer {
    /// Query id copied from the source transfer or mint payload.
    queryId: uint64
    /// Jetton amount credited to the recipient wallet, in base units.
    amount: coins
    /// Source owner address reported by the sending wallet or mint payload.
    from: address
    /// Optional response address for an `Excesses` refund after processing.
    responseAddress: address
    /// Nanotons forwarded to the owner notification.
    forwardTonAmount: coins
    /// Raw TEP-74 `Either Cell ^Cell` forward payload tail.
    forwardPayload: RemainingBitsAndRefs
}

/// Owner-originated TEP-74 burn request accepted by the wallet.
struct (0x595f07bc) JettonBurn {
    /// Caller-selected query id copied into the burn notification.
    queryId: uint64
    /// Jetton amount to burn from this wallet, in base units.
    amount: coins
    /// Optional response address for the minter's `Excesses` refund.
    responseDestination: address
    /// Optional referenced custom payload; v1 wallets keep it as a TEP-74 field.
    customPayload: cell?
}

/// Burn notification emitted to the minter after a successful wallet burn.
struct (0x7bdd97de) JettonBurnNotification {
    /// Query id copied from the original burn request.
    queryId: uint64
    /// Burned jetton amount in base units.
    amount: coins
    /// Wallet owner address reported to the minter.
    sender: address
    /// Optional response address for the minter's `Excesses` refund.
    responseDestination: address
}

/// Owner notification emitted when an incoming transfer carries forward TON.
struct (0x7362d09c) JettonTransferNotification {
    /// Query id copied from the internal transfer.
    queryId: uint64
    /// Received jetton amount in base units.
    amount: coins
    /// Source owner address reported by the internal transfer.
    sender: address
    /// Raw TEP-74 `Either Cell ^Cell` forward payload tail.
    forwardPayload: RemainingBitsAndRefs
}

/// TEP-74 excesses response emitted to a transfer response address when value remains after processing.
struct (0xd53276db) JettonExcesses {
    /// Query id copied from the request that produced the refund.
    queryId: uint64
}

/// Internal message bodies accepted by the Jetton v1 wallet.
type JettonV1WalletIncomingMessage =
    | JettonTransfer
    | JettonInternalTransfer
    | JettonBurn

/// Internal message bodies emitted by the Jetton v1 wallet.
type JettonV1WalletOutgoingMessage =
    | JettonInternalTransfer
    | JettonBurnNotification
    | JettonTransferNotification
    | JettonExcesses

/// Structured result returned by `get_wallet_data`.
struct GetJettonV1WalletDataReply {
    /// Jetton balance returned as a TVM stack integer.
    balance: int
    /// Wallet owner address.
    ownerAddress: address
    /// Jetton minter/master address.
    jettonMasterAddress: address
    /// Referenced wallet code cell.
    jettonWalletCode: cell
}

/// Jetton v1 wallet exit-code catalog from the official FunC source.
enum Errors {
    /// Sender is not the stored owner for transfer or burn requests.
    UnauthorizedOwner = 705
    /// Transfer or burn would make the stored jetton balance negative.
    InsufficientJettonBalance = 706
    /// Internal transfer sender is not authenticated, or burn funding is below the required gas/forward-fee budget.
    InvalidSenderOrBurnFunding = 707
    /// v1.03+ transfer body does not contain the required TEP-74 `Either` tag for `forward_payload`.
    MissingForwardPayloadTag = 708
    /// Transfer funding is below the required forward-fee/storage/gas budget, or a bounced body is not recognized.
    TransferFundingOrBounce = 709
    /// Incoming internal message opcode is not supported by the wallet.
    UnsupportedOperation = 65535
}

/// Return wallet balance, owner address, minter address, and wallet code cell.
get fun get_wallet_data(): GetJettonV1WalletDataReply {
    throw 0;
}
