/// Modern TON vesting wallet with V3-style signing, owner send requests, and sender-managed whitelist.
contract WalletVesting {
    author: "TON Core"
    version: "1.0"
    description: "TON Vesting Wallet is a TON wallet that holds locked funds for a beneficiary and releases them by schedule and whitelist rules."
    storage: WalletVestingStorage
    incomingMessages: WalletVestingIncomingMessage
    incomingExternal: WalletVestingIncomingExternalMessage
    outgoingMessages: WalletVestingOutgoingMessage
    thrownErrors: Errors
}

/// Persistent vesting wallet state stored in the root data cell.
struct WalletVestingStorage {
    /// Replay-protection sequence number.
    seqno: uint32
    /// Wallet id, also called subwallet id.
    subwalletId: uint32
    /// Stored Ed25519 public key used to verify external signed requests.
    publicKey: uint256
    /// Whitelist dictionary keyed by `int8 workchain + uint256 address_hash`; values are one-bit true markers.
    whitelist: map<bits264, bool>
    /// Immutable vesting parameters stored as a single referenced cell.
    vesting: Cell<WalletVestingParameters>
}

/// Immutable vesting schedule and controlling addresses stored in `WalletVestingStorage.vesting`.
struct WalletVestingParameters {
    /// Unix timestamp at which proportional vesting starts.
    vestingStartTime: uint64
    /// Full vesting period in seconds.
    vestingTotalDuration: uint32
    /// Discrete unlock step in seconds.
    unlockPeriod: uint32
    /// Initial cliff period in seconds.
    cliffDuration: uint32
    /// Total locked Toncoin amount in nanotons.
    vestingTotalAmount: coins
    /// Sender address that can top up, receive locked funds back, and append whitelist entries.
    vestingSenderAddress: address
    /// Owner address allowed to initiate internal send requests.
    ownerAddress: address
}

/// Internal owner-authenticated request that sends one raw outbound message.
struct (0xa7733acd) WalletVestingSend {
    /// Caller-selected query id echoed in `WalletVestingSendResponse`.
    queryId: uint64
    /// Send mode passed to `send_raw_message`; locked funds only allow mode `3`.
    sendMode: uint8
    /// Raw outbound internal message cell loaded by reference and sent as-is.
    message: cell
}

/// Internal vesting-sender request that appends one or more whitelist addresses.
struct (0x7258a69b) WalletVestingAddWhitelist {
    /// Caller-selected query id echoed in `WalletVestingAddWhitelistResponse`.
    queryId: uint64
    /// Snake stream of `MsgAddress` values; each address is read inline, then the next cell ref if present.
    addresses: RemainingBitsAndRefs
}

/// External-in request authenticated by Ed25519 signature over the unsigned trailing payload.
struct WalletVestingSignedExternal {
    /// Ed25519 signature over subwalletId, validUntil, seqno, and outgoing tail.
    signature: bits512
    /// Wallet id expected to match storage.subwalletId.
    subwalletId: uint32
    /// Unix timestamp after which the request is expired.
    validUntil: uint32
    /// Request sequence number expected to match storage.seqno.
    seqno: uint32
    /// Raw tail that may contain one `sendMode:uint8 + ^Message` pair; other tails only affect seqno after valid auth.
    outgoing: RemainingBitsAndRefs
}

/// Response emitted after an accepted owner send request.
struct (0xf7733acd) WalletVestingSendResponse {
    /// Query id copied from the owner send request.
    queryId: uint64
}

/// Response emitted after an accepted whitelist append request.
struct (0xf258a69b) WalletVestingAddWhitelistResponse {
    /// Query id copied from the whitelist append request.
    queryId: uint64
}

/// Internal message bodies accepted by the vesting wallet.
type WalletVestingIncomingMessage =
    | WalletVestingSend
    | WalletVestingAddWhitelist

/// External-in message body accepted by the vesting wallet.
type WalletVestingIncomingExternalMessage = WalletVestingSignedExternal

/// Outgoing response bodies emitted by the vesting wallet.
type WalletVestingOutgoingMessage =
    | WalletVestingSendResponse
    | WalletVestingAddWhitelistResponse

/// Structured result returned by `get_vesting_data`.
struct GetVestingDataReply {
    /// Unix timestamp at which proportional vesting starts.
    vestingStartTime: uint64
    /// Full vesting period in seconds.
    vestingTotalDuration: uint32
    /// Discrete unlock step in seconds.
    unlockPeriod: uint32
    /// Initial cliff period in seconds.
    cliffDuration: uint32
    /// Total locked Toncoin amount in nanotons.
    vestingTotalAmount: coins
    /// Sender address that can top up, receive locked funds back, and append whitelist entries.
    vestingSenderAddress: address
    /// Owner address allowed to initiate internal send requests.
    ownerAddress: address
    /// Whitelist dictionary keyed by `int8 workchain + uint256 address_hash`; empty map means no entries.
    whitelist: map<bits264, bool>
}

/// Single `(workchain, address_hash)` pair returned by `get_whitelist`.
struct WalletVestingWhitelistEntry {
    /// Workchain id from the packed whitelist dictionary key.
    workchain: int32
    /// 256-bit address hash from the packed whitelist dictionary key.
    address: uint256
}

/// Vesting wallet exit-code catalog from the official FunC source.
enum Errors {
    /// External request seqno does not match storage.seqno.
    InvalidSeqno = 33
    /// External request wallet id does not match storage.subwalletId.
    InvalidSubwalletId = 34
    /// Ed25519 signature verification failed.
    InvalidSignature = 35
    /// External request validUntil is not greater than the current block time.
    ExpiredMessage = 36
    /// Locked-fund send uses a mode other than `PAY_FEES_SEPARATELY + IGNORE_ERRORS`.
    SendModeNotAllowed = 100
    /// Locked-fund whitelist send uses a non-bounceable outbound message.
    NonBounceableNotAllowed = 101
    /// Locked-fund whitelist send includes StateInit.
    StateInitNotAllowed = 102
    /// Locked-fund whitelist send has a disallowed body opcode or comment shape.
    CommentNotAllowed = 103
    /// Locked-fund whitelist text comment starts with a disallowed symbol.
    SymbolsNotAllowed = 104
}

/// Return the current replay-protection sequence number.
get fun seqno(): uint32 {
    throw 0;
}

/// Return the wallet id, equivalent to subwallet id.
get fun get_subwallet_id(): uint32 {
    throw 0;
}

/// Return the stored Ed25519 public key.
get fun get_public_key(): int {
    throw 0;
}

/// Return the immutable vesting parameters and raw whitelist dictionary.
get fun get_vesting_data(): GetVestingDataReply {
    throw 0;
}

/// Return whether an address is present in the whitelist dictionary.
get fun is_whitelisted(addr: address): bool {
    throw 0;
}

/// Return all whitelist entries as a TVM cons-list of `(workchain, address_hash)` pairs.
get fun get_whitelist(): lisp_list<WalletVestingWhitelistEntry> {
    throw 0;
}

/// Return the amount, in nanotons, still locked at `atTime`.
get fun get_locked_amount(atTime: int): coins {
    throw 0;
}
