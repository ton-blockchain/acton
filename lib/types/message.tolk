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
///     println("Transfer amount: {:gram}", body.amount);
///
///     // Parsing a generic message to check opcode
///     val genericMsg = action.loadGenericMessage();
///     val opcode = genericMsg.loadOpcode();
///     if (opcode == 0x12345678) {
///         // handle specific opcode...
///     }
/// }
/// ```

import "../impl"
import "../tlb/either"
import "../tlb/maybe"

/// Represents a generic relaxed message. Unlike [TlbMessageRelaxed], this message does not
/// load the body of the message. Instead body is represented as a slice and can be parsed
/// later manually (for example to load the opcode).
///
/// Example:
/// ```tolk
/// val outActions = testing.outActions();
/// val action = outActions.at(0);
/// if (action is TlbOutActionSendMessage) {
///     val message = action.loadGenericMessage();
///     val opcode = message.loadOpcode();
///     println(opcode);
/// }
/// ```
struct TlbMessageRelaxedGeneric {
    info: TlbIntMsgInfoRelaxed
    init: TlbMaybe<TlbEither<StateInit, Cell<StateInit>>>
    body: RemainingBitsAndRefs
}

/// Loads the opcode of the message.
///
/// Returns `null` for malformed or too-short body data:
/// - body does not contain the `TlbEither` branch bit;
/// - body marks `TlbEitherRight` but does not contain a body reference;
/// - decoded body has fewer than 32 bits for the first opcode;
/// - [skipBounce] is true, first opcode is a bounce prefix (`0xFFFFFFFF` or `0xFFFFFFFE`),
///   and fewer than 32 bits remain for the second opcode.
///
/// If [skipBounce] is true, bounce prefixes `0xFFFFFFFF` and `0xFFFFFFFE` are skipped.
///
/// Example:
/// ```tolk
/// val outActions = testing.outActions();
/// val action = outActions.at(0);
/// if (action is TlbOutActionSendMessage) {
///     val message = action.loadGenericMessage();
///     val opcode = message.loadOpcode(true);
///     println(opcode);
/// }
/// ```
fun TlbMessageRelaxedGeneric.loadOpcode(self, skipBounce: bool = false): int? {
    return impl.loadOpcode(self.body, skipBounce);
}

/// Represents a generic external message. Unlike [TlbExtMessageRelaxed], this message does not
/// load the body of the message. Instead body is represented as a slice and can be parsed
/// later manually (for example to load the opcode).
struct TlbExtMessageRelaxedGeneric {
    info: TlbExtMsgInfoRelaxed
    init: TlbMaybe<TlbEither<StateInit, Cell<StateInit>>>
    body: RemainingBitsAndRefs
}

/// Loads the opcode of the message.
///
/// Same behavior as [TlbMessageRelaxedGeneric.loadOpcode]:
/// - returns the first 32-bit opcode by default;
/// - if [skipBounce] is true, bounce prefixes `0xFFFFFFFF` and `0xFFFFFFFE` are skipped;
/// - returns `null` for malformed or too-short body data.
fun TlbExtMessageRelaxedGeneric.loadOpcode(self, skipBounce: bool = false): int? {
    return impl.loadOpcode(self.body, skipBounce);
}

/// Represents a generic message. Unlike [TlbMessageRelaxedGeneric], this message loads
/// the body of the message with passed type.
///
/// If actual message body has a different type, loading will fail with exit code 63.
///
/// Example:
/// ```tolk
/// val outActions = testing.outActions();
/// val action = outActions.at(0);
/// if (action is TlbOutActionSendMessage) {
///     val message = action.loadMessage<IncreaseCounter>();
///     val body = message.loadBody();
///     println(body.increaseBy);
/// }
/// ```
@overflow1023_policy("suppress")
struct TlbMessageRelaxed<X> {
    info: TlbIntMsgInfoRelaxed
    init: TlbMaybe<TlbEither<StateInit, Cell<StateInit>>>
    body: TlbEither<X, Cell<X>>
}

/// Loads the body of the message.
///
/// Decoding branch behavior:
/// - `TlbEitherLeft`: body is already in this cell and is returned directly.
/// - `TlbEitherRight`: body is loaded from referenced cell.
///
/// Note that if message type mismatch, loading will fail with exit code 63.
///
/// Example:
/// ```tolk
/// val outActions = testing.outActions();
/// val action = outActions.at(0);
/// if (action is TlbOutActionSendMessage) {
///     val message = action.loadMessage<IncreaseCounter>();
///     val body = message.loadBody();
///     println(body.increaseBy);
/// }
/// ```
fun TlbMessageRelaxed<X>.loadBody(self): X {
    if (self.body is TlbEitherLeft<X>) {
        return self.body.value;
    }
    return self.body.value.load({ assertEndAfterReading: false });
}

/// Represents a generic external message. Unlike [TlbExtMessageRelaxedGeneric], this message
/// loads the body of the message with passed type.
///
/// If actual message body has a different type, loading will fail with exit code 63.
@overflow1023_policy("suppress")
struct TlbExtMessageRelaxed<X> {
    info: TlbExtMsgInfoRelaxed
    init: TlbMaybe<TlbEither<StateInit, Cell<StateInit>>>
    body: TlbEither<X, Cell<X>>
}

/// Loads the body of the message.
///
/// Decoding branch behavior:
/// - `TlbEitherLeft`: body is already in this cell and is returned directly.
/// - `TlbEitherRight`: body is loaded from referenced cell.
///
/// Note that if message type mismatch, loading will fail with exit code 63.
fun TlbExtMessageRelaxed<X>.loadBody(self): X {
    if (self.body is TlbEitherLeft<X>) {
        return self.body.value;
    }
    return self.body.value.load({ assertEndAfterReading: false });
}

/// Represents common message info for internal relaxed message.
struct (0b0) TlbIntMsgInfoRelaxed {
    ihrDisabled: bool
    bounce: bool
    bounced: bool
    src: any_address
    dest: address
    value: TlbCurrencyCollection
    extraFlags: varuint16
    fwdFee: coins
    createdLt: uint64
    createdAt: uint32
}

/// Represents common message info for external relaxed message.
struct (0b11) TlbExtMsgInfoRelaxed {
    src: address
    dest: any_address
    createdLt: uint64
    createdAt: uint32
}

/// Represents a currency collection as GRAM amount + extra currencies.
struct TlbCurrencyCollection {
    /// Amount in nanograms.
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
struct (0b0) TlbInternalMessage {
    ihrDisabled: bool
    bounce: bool
    bounced: bool
    src: address
    dest: address
    value: TlbCurrencyCollection
    extraFlags: varuint16
    forwardFee: coins
    createdLt: uint64
    createdAt: uint32
}

/// ```tlb
/// ext_in_msg_info$10 src:MsgAddressExt dest:MsgAddressInt
///                    import_fee:Grams = CommonMsgInfo;
/// ```
struct (0b10) TlbExternalInMessageInfo {
    src: any_address
    dest: address
    importFee: coins
}

/// ```tlb
/// ext_out_msg_info$11 src:MsgAddressInt dest:MsgAddressExt
///                     created_lt:uint64 created_at:uint32 = CommonMsgInfo;
/// ```
struct (0b11) TlbExternalOutMessageInfo {
    src: address
    dest: any_address
    createdLt: uint64
    createdAt: uint32
}

/// Tagged union for the TL-B `CommonMsgInfo` constructors.
///
/// Runtime variant is selected by the leading tag:
/// - `0b0`: [TlbInternalMessage]
/// - `0b10`: [TlbExternalInMessageInfo]
/// - `0b11`: [TlbExternalOutMessageInfo]
type TlbCommonMessageInfo =
    | TlbInternalMessage
    | TlbExternalInMessageInfo
    | TlbExternalOutMessageInfo

/// Generic message structure with configurable body and info types.
///
/// `TInfo` defaults to [TlbCommonMessageInfo], so when [TlbMessage] is loaded with default generic
/// arguments, `info` is decoded as one of the `CommonMsgInfo` tagged variants above.
@overflow1023_policy("suppress")
struct TlbMessage<TBody = RemainingBitsAndRefs, TInfo = TlbCommonMessageInfo> {
    info: TInfo
    init: TlbMaybe<TlbEither<StateInit, Cell<StateInit>>>
    body: TlbEither<TBody, Cell<TBody>>
}

/// Loads `Message.body`.
///
/// Decoding branch behavior:
/// - `TlbEitherLeft`: body is already in this cell and is returned directly.
/// - `TlbEitherRight`: body is loaded from referenced cell.
fun TlbMessage<TBody, TInfo>.loadBody(self): TBody {
    return match (self.body) {
        TlbEitherLeft => self.body.value,
        TlbEitherRight => self.body.value.load({ assertEndAfterReading: false }),
    };
}

fun impl.loadOpcode(body: slice, skipBounce: bool): int? {
    var slice = body;
    if (slice.remainingBitsCount() < 1) {
        return null;
    }
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
    if (skipBounce && (opcode == 0xFFFFFFFF || opcode == 0xFFFFFFFE)) {
        if (slice.remainingBitsCount() < 32) {
            return null;
        }
        return slice.loadUint(32);
    }
    return opcode;
}
