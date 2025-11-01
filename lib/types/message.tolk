import "../tlb/either"
import "../tlb/maybe"

/// Represents a generic relaxed message. Unlike [MessageRelaxed], this message does not
/// load the body of the message. Instead body is represented as a slice and can be parsed
/// later manually (for example to load the opcode).
/// 
/// Example:
/// ```
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
/// If [skip_bounce] is true, the 0xFFFFFFFF prefix is skipped.
/// 
/// Example:
/// ```
/// val out_actions = vm.outActions();
/// val action = out_actions.at(0);
/// if (action is OutActionSendMessage) {
///     val message = action.loadGenericMessage();
///     val opcode = message.loadOpcode(true);
///     println(opcode);
/// }
/// ```
fun MessageRelaxedGeneric.loadOpcode(self, skip_bounce: bool = false): int? {
    return impl_msg.load_opcode(self.body, skip_bounce);
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
/// If [skip_bounce] is true, the 0xFFFFFFFF prefix is skipped.
fun ExtMessageRelaxedGeneric.loadOpcode(self, skip_bounce: bool = false): int? {
    return impl_msg.load_opcode(self.body, skip_bounce);
}

/// Represents a generic message. Unlike [MessageRelaxedGeneric], this message loads
/// the body of the message with passed type.
/// 
/// If actual message body has a different type, loading will fail with exit code 63.
/// 
/// Example:
/// ```
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
/// Note that if message type mismatch, loading will fail with exit code 63.
/// 
/// Example:
/// ```
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
/// Note that if message type mismatch, loading will fail with exit code 63.
fun ExtMessageRelaxed<X>.loadBody(self): X {
    if (self.body is EitherLeft<X>) {
        return self.body.value;
    }
    return self.body.value.load({ assertEndAfterReading: false });
}

/// Represents common message info for internal relaxed message.
struct (0b0) IntMsgInfoRelaxed {
    ihr_disabled: bool
    bounce: bool
    bounced: bool
    src: address
    dest: address
    value: CurrencyCollection
    extra_flags: varuint16
    fwd_fee: coins
    created_lt: uint64
    created_at: uint32
}

/// Represents common message info for external relaxed message.
struct (0b11) ExtMsgInfoRelaxed {
    src: address
    dest: address
    created_lt: uint64
    created_at: uint32
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
    createdLT: uint64
    createdAt: uint32
}

/// ```tlb
/// ext_in_msg_info$10 src:MsgAddressExt dest:MsgAddressInt
///                    import_fee:Grams = CommonMsgInfo;
/// ```
struct (0b10) ExternalInMessageInfo {
    src: address // TODO: Change to `address_any` after TVM v12 update.
    dest: address
    importFee: coins
}

/// ```tlb
/// ext_out_msg_info$11 src:MsgAddressInt dest:MsgAddressExt
///                     created_lt:uint64 created_at:uint32 = CommonMsgInfo;
/// ```
struct (0b11) ExternalOutMessageInfo {
    src: address
    dest: address // TODO: Change to `address_any` after TVM v12 update.
    createdLt: uint64
    createdAt: uint32
}

type CommonMessageInfo =
    | InternalMessage
    | ExternalInMessageInfo
    | ExternalOutMessageInfo

struct Message<TBody = RemainingBitsAndRefs, TInfo = CommonMessageInfo> {
    info: TInfo
    init: Maybe<Either<StateInit, Cell<StateInit>>>
    body: Either<TBody, Cell<TBody>>
}

fun Message<TBody, TInfo>.loadBody(self): TBody {
    return match (self.body) {
        EitherLeft => self.body.value,
        EitherRight => self.body.value.load({ assertEndAfterReading: false }),
    };
}

struct impl_msg

fun impl_msg.load_opcode(body: slice, skip_bounce: bool) {
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
    if (skip_bounce && opcode == 0xFFFFFFFF) {
        if (slice.remainingBitsCount() < 32) {
            return null;
        }
        return slice.loadUint(32);
    }
    return opcode;
}
