/// Module defining message types.
///
/// This module provides the core structures for working with TON messages in the emulator.
/// It defines both internal and external message structures, as well as their components.
///
/// Examples:
/// ```tolk
/// // Parsing an out action into a typed message
/// val action = outActions.getSendMessageAt(0);
/// if (action != null) {
///     val msg = action.loadMessage<Transfer>();
///     val body = msg.loadBody();
///     println1("Transfer amount: {:ton}", body.amount);
/// }
///
/// // Parsing a generic message to check opcode
/// val genericMsg = action.loadGenericMessage();
/// val opcode = genericMsg.loadOpcode();
/// if (opcode == 0x12345678) {
///     // handle specific opcode...
/// }
/// ```

import "../tlb/either"
import "../tlb/maybe"

/// Represents a generic relaxed message. Unlike [MessageRelaxed], this message does not
/// load the body of the message. Instead body is represented as a slice and can be parsed
/// later manually (for example to load the opcode).
///
/// Example:
/// ```tolk
/// val out_actions = vm.outActions();
/// val action = out_actions.at(0);
/// if (action is OutActionSendMessage) {
///     val message = action.loadGenericMessage();
///     val opcode = message.loadOpcode();
///     println(opcode);
/// }
/// ```
struct MessageRelaxedGeneric {
    info: IntMsgInfoRelaxed
    init: Maybe<Either<StateInit, Cell<StateInit>>>
    body: RemainingBitsAndRefs
}

/// Loads the opcode of the message.
///
/// Returns `null` for malformed or too-short body data:
/// - body marks `EitherRight` but does not contain a body reference;
/// - decoded body has fewer than 32 bits for the first opcode;
/// - [skipBounce] is true, first opcode is `0xFFFFFFFF`, and fewer than 32 bits remain
///   for the second opcode.
///
/// If [skipBounce] is true, the `0xFFFFFFFF` prefix is skipped.
///
/// Example:
/// ```tolk
/// val out_actions = vm.outActions();
/// val action = out_actions.at(0);
/// if (action is OutActionSendMessage) {
///     val message = action.loadGenericMessage();
///     val opcode = message.loadOpcode(true);
///     println(opcode);
/// }
/// ```
fun MessageRelaxedGeneric.loadOpcode(self, skipBounce: bool = false): int? {
    return implMsg.loadOpcode(self.body, skipBounce);
}

/// Represents a generic external message. Unlike [ExtMessageRelaxed], this message does not
/// load the body of the message. Instead body is represented as a slice and can be parsed
/// later manually (for example to load the opcode).
struct ExtMessageRelaxedGeneric {
    info: ExtMsgInfoRelaxed
    init: Maybe<Either<StateInit, Cell<StateInit>>>
    body: RemainingBitsAndRefs
}

/// Loads the opcode of the message.
///
/// Same behavior as [MessageRelaxedGeneric.loadOpcode]:
/// - returns the first 32-bit opcode by default;
/// - if [skipBounce] is true, the `0xFFFFFFFF` prefix is skipped;
/// - returns `null` for malformed or too-short body data.
fun ExtMessageRelaxedGeneric.loadOpcode(self, skipBounce: bool = false): int? {
    return implMsg.loadOpcode(self.body, skipBounce);
}

/// Represents a generic message. Unlike [MessageRelaxedGeneric], this message loads
/// the body of the message with passed type.
///
/// If actual message body has a different type, loading will fail with exit code 63.
///
/// Example:
/// ```tolk
/// val out_actions = vm.outActions();
/// val action = out_actions.at(0);
/// if (action is OutActionSendMessage) {
///     val message = action.loadMessage<IncreaseCounter>();
///     val body = message.loadBody();
///     println(body.increaseBy);
/// }
/// ```
@overflow1023_policy("suppress")
struct MessageRelaxed<X> {
    info: IntMsgInfoRelaxed
    init: Maybe<Either<StateInit, Cell<StateInit>>>
    body: Either<X, Cell<X>>
}

/// Loads the body of the message.
///
/// Decoding branch behavior:
/// - `EitherLeft`: body is already in this cell and is returned directly.
/// - `EitherRight`: body is loaded from referenced cell.
///
/// Note that if message type mismatch, loading will fail with exit code 63.
///
/// Example:
/// ```tolk
/// val out_actions = vm.outActions();
/// val action = out_actions.at(0);
/// if (action is OutActionSendMessage) {
///     val message = action.loadMessage<IncreaseCounter>();
///     val body = message.loadBody();
///     println(body.increaseBy);
/// }
/// ```
fun MessageRelaxed<X>.loadBody(self): X {
    if (self.body is EitherLeft<X>) {
        return self.body.value;
    }
    return self.body.value.load({ assertEndAfterReading: false });
}

/// Represents a generic external message. Unlike [ExtMessageRelaxedGeneric], this message
/// loads the body of the message with passed type.
///
/// If actual message body has a different type, loading will fail with exit code 63.
@overflow1023_policy("suppress")
struct ExtMessageRelaxed<X> {
    info: ExtMsgInfoRelaxed
    init: Maybe<Either<StateInit, Cell<StateInit>>>
    body: Either<X, Cell<X>>
}

/// Loads the body of the message.
///
/// Decoding branch behavior:
/// - `EitherLeft`: body is already in this cell and is returned directly.
/// - `EitherRight`: body is loaded from referenced cell.
///
/// Note that if message type mismatch, loading will fail with exit code 63.
fun ExtMessageRelaxed<X>.loadBody(self): X {
    if (self.body is EitherLeft<X>) {
        return self.body.value;
    }
    return self.body.value.load({ assertEndAfterReading: false });
}

/// Represents common message info for internal relaxed message.
struct (0b0) IntMsgInfoRelaxed {
    ihrDisabled: bool
    bounce: bool
    bounced: bool
    src: any_address
    dest: address
    value: CurrencyCollection
    extraFlags: varuint16
    fwdFee: coins
    createdLt: uint64
    createdAt: uint32
}

/// Represents common message info for external relaxed message.
struct (0b11) ExtMsgInfoRelaxed {
    src: address
    dest: any_address
    createdLt: uint64
    createdAt: uint32
}

/// Represents a currency collection as TON amount + extra currencies.
struct CurrencyCollection {
    /// Amount in nanoton.
    grams: coins
    /// Extra currencies.
    other: map<int32, varuint32>
}

/// ```tlb
/// int_msg_info$0 ihr_disabled:Bool bounce:Bool bounced:Bool
///                src:MsgAddressInt dest:MsgAddressInt
///                value:CurrencyCollection extra_flags:(VarUInteger 16) fwd_fee:Grams
///                created_lt:uint64 created_at:uint32 = CommonMsgInfo;
/// ```
struct (0b0) InternalMessage {
    ihrDisabled: bool
    bounce: bool
    bounced: bool
    src: address
    dest: address
    value: CurrencyCollection
    extraFlags: varuint16
    forwardFee: coins
    createdLt: uint64
    createdAt: uint32
}

/// ```tlb
/// ext_in_msg_info$10 src:MsgAddressExt dest:MsgAddressInt
///                    import_fee:Grams = CommonMsgInfo;
/// ```
struct (0b10) ExternalInMessageInfo {
    src: any_address
    dest: address
    importFee: coins
}

/// ```tlb
/// ext_out_msg_info$11 src:MsgAddressInt dest:MsgAddressExt
///                     created_lt:uint64 created_at:uint32 = CommonMsgInfo;
/// ```
struct (0b11) ExternalOutMessageInfo {
    src: address
    dest: any_address
    createdLt: uint64
    createdAt: uint32
}

/// Tagged union for the TL-B `CommonMsgInfo` constructors.
///
/// Runtime variant is selected by the leading tag:
/// - `0b0`: [InternalMessage]
/// - `0b10`: [ExternalInMessageInfo]
/// - `0b11`: [ExternalOutMessageInfo]
type CommonMessageInfo =
    | InternalMessage
    | ExternalInMessageInfo
    | ExternalOutMessageInfo

/// Generic message structure with configurable body and info types.
///
/// `TInfo` defaults to [CommonMessageInfo], so when `Message` is loaded with default generic
/// arguments, `info` is decoded as one of the `CommonMsgInfo` tagged variants above.
@overflow1023_policy("suppress")
struct Message<TBody = RemainingBitsAndRefs, TInfo = CommonMessageInfo> {
    info: TInfo
    init: Maybe<Either<StateInit, Cell<StateInit>>>
    body: Either<TBody, Cell<TBody>>
}

/// Loads `Message.body`.
///
/// Decoding branch behavior:
/// - `EitherLeft`: body is already in this cell and is returned directly.
/// - `EitherRight`: body is loaded from referenced cell.
fun Message<TBody, TInfo>.loadBody(self): TBody {
    return match (self.body) {
        EitherLeft => self.body.value,
        EitherRight => self.body.value.load({ assertEndAfterReading: false }),
    };
}

struct implMsg

fun implMsg.loadOpcode(body: slice, skipBounce: bool) {
    var slice = body;
    if (slice.loadBool()) {
        // actual body in the ref
        if (slice.remainingRefsCount() == 0) {
            return null;
        }
        val body = slice.loadRef();
        slice = body.beginParse();
    }

    if (slice.remainingBitsCount() < 32) {
        return null;
    }

    val opcode = slice.loadUint(32);
    if (skipBounce && opcode == 0xFFFFFFFF) {
        if (slice.remainingBitsCount() < 32) {
            return null;
        }
        return slice.loadUint(32);
    }
    return opcode;
}
