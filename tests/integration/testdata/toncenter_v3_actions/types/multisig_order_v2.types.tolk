/// TON Multisig V2 order contract interface with approval state, init/approve messages, and execution responses.
contract MultisigOrderV2 {
    author: "TON Core"
    version: "2.0"
    description: "TON Multisig V2 Order is a temporary approval contract that collects signer confirmations before a multisig transaction executes."
    storage: MultisigOrderV2Storage
    storageAtDeployment: MultisigOrderV2DeploymentStorage
    incomingMessages: MultisigOrderV2IncomingMessage
    outgoingMessages: MultisigOrderV2OutgoingMessage
    thrownErrors: OrderErrors
}

/// Deployment data used before an order contract receives its init message.
struct MultisigOrderV2DeploymentStorage {
    /// Parent main multisig address authorized to initialize the order.
    multisigAddress: address
    /// Order id used together with the parent address to derive this contract address.
    orderSeqno: uint256
}

/// Initialized order state stored after the first valid init message.
struct MultisigOrderV2Storage {
    /// Parent main multisig address authorized to initialize and execute the order.
    multisigAddress: address
    /// Order id used together with parent address to derive this contract address.
    orderSeqno: uint256
    /// Number of signer approvals required before execution is sent to the parent multisig.
    threshold: uint8
    /// True after the order contract has sent the execution message to the parent multisig.
    sentForExecution: bool
    /// Raw mandatory `Hashmap 8 MsgAddressInt` signer dictionary root cell.
    signers: cell
    /// 256-bit approval bitmap keyed by signer index.
    approvalsMask: uint256
    /// Number of approvals currently set in approvalsMask.
    approvalsNum: uint8
    /// Unix timestamp after which init/execution is expired, stored on-chain as uint48.
    expirationDate: uint48
    /// Referenced raw mandatory `Hashmap 8 ^Action` order root cell.
    order: cell
}

/// Init message sent by the parent multisig to deploy or re-approve this order.
struct (0x9c73fba2) MultisigOrderV2Init {
    /// Query id from the parent new-order request.
    queryId: uint64
    /// Approval threshold copied from parent multisig storage.
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

/// Signer approval message carrying the signer's dictionary index.
struct (0xa762230f) MultisigOrderV2Approve {
    /// Caller-selected query id echoed in accept/reject responses.
    queryId: uint64
    /// Signer index expected to map to the sender address.
    signerIndex: uint8
}

/// Zero-op text-comment approval path; the remaining text must equal ASCII `approve`.
struct (0x00000000) MultisigOrderV2ApproveText {
    /// Raw text-comment tail after the zero opcode.
    text: RemainingBitsAndRefs
}

/// Execution message emitted to the parent multisig after enough approvals.
struct (0x75097f5d) MultisigOrderV2Execute {
    /// Query id from the approval that triggered execution.
    queryId: uint64
    /// Order id used by the parent to authenticate this order address.
    orderSeqno: uint256
    /// Unix timestamp after which execution is rejected, stored as uint48.
    expirationDate: uint48
    /// Number of approvals collected by this order.
    approvalsNum: uint8
    /// Hash of the signer dictionary used by this order.
    signersHash: uint256
    /// Referenced raw mandatory `Hashmap 8 ^Action` order root cell.
    order: cell
}

/// Approval accepted notification emitted to the signer/response address.
struct (0x82609bf6) MultisigOrderV2ApproveAccepted {
    /// Query id supplied in the approval message.
    queryId: uint64
}

/// Approval rejected notification emitted to the signer/response address.
struct (0xafaf283e) MultisigOrderV2ApproveRejected {
    /// Query id supplied in the approval message.
    queryId: uint64
    /// Exit code caught while processing the approval attempt.
    exitCode: uint32
}

/// Internal message bodies accepted by Multisig Order V2.
type MultisigOrderV2IncomingMessage =
    | MultisigOrderV2Init
    | MultisigOrderV2Approve
    | MultisigOrderV2ApproveText

/// Internal message bodies emitted by Multisig Order V2.
type MultisigOrderV2OutgoingMessage =
    | MultisigOrderV2Execute
    | MultisigOrderV2ApproveAccepted
    | MultisigOrderV2ApproveRejected

/// Structured result returned by `get_order_data`.
struct GetMultisigOrderV2DataReply {
    /// Parent main multisig address.
    multisigAddress: address
    /// Order id assigned by the parent multisig.
    orderSeqno: int
    /// Number of approvals required before execution; null before init.
    threshold: int?
    /// True after the execution message has been sent to the parent multisig; null before init.
    sentForExecution: bool?
    /// Raw optional `Hashmap 8 MsgAddressInt` signer dictionary root returned as a TVM dict.
    signers: dict
    /// Approval bitmap keyed by signer index; null before init.
    approvalsMask: int?
    /// Number of collected approvals; null before init.
    approvalsNum: int?
    /// Expiration timestamp returned as a TVM integer; null before init.
    expirationDate: int?
    /// Raw optional `Hashmap 8 ^Action` order root returned as a TVM dict.
    order: dict
}

/// Multisig Order V2 exit-code catalog from official source and local schema.
enum OrderErrors {
    /// Init sender is not the parent main multisig address.
    UnauthorizedInit = 104
    /// Repeated init data does not match the already initialized order.
    AlreadyInited = 105
    /// Approval sender is not the signer at the supplied index.
    UnauthorizedSign = 106
    /// The signer index has already approved this order.
    AlreadyApproved = 107
    /// Init or execution expiration timestamp is earlier than current Unix timestamp.
    Expired = 111
    /// Approval attempted after execution was already sent.
    AlreadyExecuted = 112
    /// Message opcode is neither init, approve, nor zero-op text comment `approve`.
    UnknownOp = 65535
}

/// Return parent multisig address, order id, approval state, signer dictionary, expiration, and raw order cell.
get fun get_order_data(): GetMultisigOrderV2DataReply {
    throw 0;
}
