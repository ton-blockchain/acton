/// Jetton v1 minter with admin minting, burn notifications, and mutable admin/content controls.
contract JettonV1Master {
    author: "TON Core"
    version: "1"
    description: "Jetton v1 Master is the issuer contract for a TEP-74 token, defining supply, metadata, admin, and wallet creation."
    storage: JettonV1MasterStorage
    incomingMessages: JettonV1MasterIncomingMessage
    outgoingMessages: JettonV1MasterOutgoingMessage
    thrownErrors: JettonMasterErrors
}

/// Persistent minter state stored in the root data cell.
struct JettonV1MasterStorage {
    /// Total issued jettons encoded as TL-B `Coins` in storage.
    totalSupply: coins
    /// Current admin address authorized to mint and mutate admin/content fields.
    adminAddress: address
    /// Referenced jetton metadata content cell returned by `get_jetton_data`.
    jettonContent: cell
    /// Referenced wallet code cell used to derive deterministic user wallet addresses.
    jettonWalletCode: cell
}

/// Admin mint request accepted by Jetton v1 minters.
struct (0x00000015) JettonV1Mint {
    /// Admin-selected query id carried only in the minter request.
    queryId: uint64
    /// Jetton owner address whose deterministic wallet receives the mint transfer.
    toAddress: address
    /// Nanotons attached to the wallet deployment/internal-transfer message.
    tonAmount: coins
    /// Referenced raw wallet message body, normally `JettonInternalTransfer`.
    masterMsg: cell
}

/// Burn notification sent by a deterministic user wallet to decrease total supply.
struct (0x7bdd97de) JettonMasterBurnNotification {
    /// Query id copied from the original wallet burn request.
    queryId: uint64
    /// Burned jetton amount in base units.
    amount: coins
    /// Owner address whose deterministic wallet burned the jettons.
    sender: address
    /// Optional response address that receives an `Excesses` refund when non-null.
    responseDestination: address
}

/// Admin request that replaces the stored admin address.
struct (0x00000003) JettonV1ChangeAdmin {
    /// Admin-selected query id carried in the change request.
    queryId: uint64
    /// New admin address stored by the minter.
    newAdminAddress: address
}

/// Admin request that replaces the stored metadata content cell.
struct (0x00000004) JettonV1ChangeContent {
    /// Admin-selected query id carried in the change request.
    queryId: uint64
    /// Referenced replacement jetton metadata content cell.
    jettonContent: cell
}

/// TEP-74 excesses response emitted after burn notification processing when a response address is present.
struct (0xd53276db) JettonMasterExcesses {
    /// Query id copied from the request that produced the refund.
    queryId: uint64
}

/// Internal message bodies accepted by the Jetton v1 minter.
type JettonV1MasterIncomingMessage =
    | JettonV1Mint
    | JettonMasterBurnNotification
    | JettonV1ChangeAdmin
    | JettonV1ChangeContent

/// Statically known internal message bodies emitted by the Jetton v1 minter.
type JettonV1MasterOutgoingMessage = JettonMasterExcesses

/// Structured result returned by `get_jetton_data`.
struct GetJettonV1DataReply {
    /// Total issued jettons returned as a TVM stack integer.
    totalSupply: int
    /// Legacy source returns true because this minter can mint through the admin.
    mintable: bool
    /// Current admin address.
    adminAddress: address
    /// Referenced jetton metadata content cell.
    jettonContent: cell
    /// Referenced wallet code cell embedded in deterministic wallet state init.
    jettonWalletCode: cell
}

/// Jetton v1 minter exit-code catalog from the official FunC source.
enum JettonMasterErrors {
    /// Sender is not the stored admin for mint/admin/content requests.
    UnauthorizedAdmin = 73
    /// Burn notification sender is not the deterministic wallet for the reported owner.
    InvalidBurnNotificationSender = 74
    /// Incoming internal message opcode is not supported by the minter.
    UnsupportedOperation = 65535
}

/// Return total supply, mintability, admin address, content cell, and wallet code cell.
get fun get_jetton_data(): GetJettonV1DataReply {
    throw 0;
}

/// Return the deterministic wallet address for `ownerAddress`.
get fun get_wallet_address(ownerAddress: address): address {
    throw 0;
}
