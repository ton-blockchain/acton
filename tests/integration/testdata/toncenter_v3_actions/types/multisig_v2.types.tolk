/// Main TON Multisig V2 wallet interface with order creation, execution, storage, get methods, and source-derived exit codes.
contract MultisigV2 {
    author: "TON Core"
    version: "2.0"
    description: "TON Multisig V2 is a shared TON wallet where multiple signers approve and execute transactions for one account."
    storage: MultisigV2Storage
    incomingMessages: MultisigV2IncomingMessage
    outgoingMessages: MultisigV2OutgoingMessage
    forceAbiExport: MultisigV2OrderAction
    thrownErrors: MultisigErrors
}

/// Persistent Multisig V2 state stored in the root data cell.
struct MultisigV2Storage {
    /// Next sequential order id unless arbitrary order ids are enabled.
    nextOrderSeqno: uint256
    /// Number of signer approvals required for execution.
    threshold: uint8
    /// Raw mandatory `Hashmap 8 MsgAddressInt` root cell keyed by contiguous signer indexes.
    signers: cell
    /// Cached signer count validated against the signer dictionary sequence.
    signersNum: uint8
    /// `HashmapE 8 MsgAddressInt` proposer dictionary keyed by contiguous proposer indexes.
    proposers: map<uint8, address>
    /// When true, `get_multisig_data` returns `-1` for seqno and new orders may choose arbitrary ids.
    allowArbitraryOrderSeqno: bool
}

/// Order action that sends an arbitrary raw internal message.
struct (0xf1381e5b) MultisigV2SendMessageAction {
    /// Raw send mode passed to `send_raw_message`.
    mode: uint8
    /// Referenced raw message cell passed unchanged to `send_raw_message`.
    message: cell
}

/// Order action that replaces threshold, signer set, and proposer set.
struct (0x1d0cfbd3) MultisigV2UpdateMultisigParamAction {
    /// New approval threshold; source requires `0 < threshold <= signers_num`.
    threshold: uint8
    /// Raw mandatory `Hashmap 8 MsgAddressInt` root cell with contiguous signer indexes.
    signers: cell
    /// `HashmapE 8 MsgAddressInt` proposer dictionary.
    proposers: map<uint8, address>
}

/// Action value stored under an 8-bit key in a mandatory order dictionary.
type MultisigV2OrderAction =
    | MultisigV2SendMessageAction
    | MultisigV2UpdateMultisigParamAction

/// Request to create or approve a deterministic order contract.
struct (0xf718510f) MultisigV2NewOrder {
    /// Caller-selected query id forwarded to the order contract init message.
    queryId: uint64
    /// Requested order id; all-ones means use the next stored seqno when arbitrary ids are disabled.
    orderSeqno: uint256
    /// Selects signer dictionary when true and proposer dictionary when false for authorization.
    isAutoApprove: bool
    /// Index in the selected signer/proposer dictionary.
    index: uint8
    /// Unix timestamp after which the order is expired, stored on-chain as uint48.
    expirationDate: uint48
    /// Referenced raw mandatory `Hashmap 8 ^Action` order root cell.
    order: cell
}

/// Execution request sent by a validated order contract back to the multisig.
struct (0x75097f5d) MultisigV2Execute {
    /// Query id copied from order approval flow.
    queryId: uint64
    /// Order id used to derive and authenticate the sender order address.
    orderSeqno: uint256
    /// Unix timestamp after which execution is rejected, stored as uint48.
    expirationDate: uint48
    /// Number of approvals collected by the order contract.
    approvalsNum: uint8
    /// Hash of the signer dictionary used by the order contract.
    signersHash: uint256
    /// Referenced raw mandatory `Hashmap 8 ^Action` order root cell.
    order: cell
}

/// Self-message that executes an order body without rechecking signer dictionary freshness.
struct (0xa32c59bf) MultisigV2ExecuteInternal {
    /// Caller-selected query id carried in the self-message body.
    queryId: uint64
    /// Referenced raw mandatory `Hashmap 8 ^Action` order root cell.
    order: cell
}

/// Init message emitted by the main multisig to a deterministic order contract.
struct (0x9c73fba2) MultisigV2OrderInit {
    /// Query id from the new-order request.
    queryId: uint64
    /// Approval threshold copied from main multisig storage.
    threshold: uint8
    /// Raw mandatory `Hashmap 8 MsgAddressInt` signer dictionary root cell.
    signers: cell
    /// Unix timestamp after which the order is expired, stored as uint48.
    expirationDate: uint48
    /// Referenced raw mandatory `Hashmap 8 ^Action` order root cell.
    order: cell
    /// Optional signer index; the Maybe presence bit is source field `approve_on_init`.
    signerIndexOnInit: uint8?
}

/// Zero-op text-comment body accepted and ignored by the main multisig.
struct (0x00000000) MultisigV2TextComment {
    /// Raw text-comment tail after the zero opcode.
    comment: RemainingBitsAndRefs
}

/// Internal message bodies accepted by the main Multisig V2 contract.
type MultisigV2IncomingMessage =
    | MultisigV2NewOrder
    | MultisigV2Execute
    | MultisigV2ExecuteInternal
    | MultisigV2TextComment

/// Internal message bodies emitted by the main Multisig V2 contract with known static ABI.
type MultisigV2OutgoingMessage = MultisigV2OrderInit

/// Structured result returned by `get_multisig_data`.
struct GetMultisigV2DataReply {
    /// Next order id, or `-1` when arbitrary order ids are enabled.
    seqno: int
    /// Number of signer approvals required for execution.
    threshold: uint8
    /// Getter-decoded mandatory `Hashmap 8 MsgAddressInt` signer dictionary.
    signers: map<uint8, address>
    /// `HashmapE 8 MsgAddressInt` proposer dictionary.
    proposers: map<uint8, address>
}

/// Main Multisig V2 exit-code catalog from official source and local schema.
enum MultisigErrors {
    /// New-order sender is not the selected signer/proposer at the supplied index.
    UnauthorizedNewOrder = 1007
    /// New-order seqno is neither the next seqno nor the all-ones auto-seqno marker.
    InvalidNewOrder = 1008
    /// New-order message value is below the source-calculated order processing cost.
    NotEnoughTon = 100
    /// Execution sender is neither the expected order contract nor self for internal execution.
    UnauthorizedExecute = 101
    /// Order signer dictionary hash or approval count is not current enough for execution.
    SignersOutdated = 102
    /// Signer/proposer dictionary keys are not contiguous from zero.
    InvalidDictionarySequence = 103
    /// Stored signer count does not match the signer dictionary sequence length.
    InconsistentData = 108
    /// Threshold is zero or greater than the signer count.
    InvalidThreshold = 109
    /// Signer dictionary is empty.
    InvalidSigners = 110
    /// Order expiration timestamp is earlier than the current Unix timestamp.
    Expired = 111
}

/// Return next order id, threshold, signer dictionary, and proposer dictionary.
get fun get_multisig_data(): GetMultisigV2DataReply {
    throw 0;
}

/// Return estimated nanotons required to create, store, approve, and execute an order until expiration.
get fun get_order_estimate(order: cell, expirationDate: int): int {
    throw 0;
}

/// Return the deterministic order contract address for `orderSeqno`.
get fun get_order_address(orderSeqno: int): address {
    throw 0;
}
