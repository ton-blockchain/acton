// ABI fixture for the viewer. Regenerate from this directory:
// acton compile abiRendering.tolk --abi abiRendering.json
contract AbiRendering {
    storage: Storage<uint32>
    storageAtDeployment: Storage<address>
    incomingMessages: GenericMessage<uint32> | GenericMessage<address>
    incomingExternal: Message<address>
    outgoingMessages: Wrapped<address>
    emittedEvents: Message<bool> | WidePrefix
}

struct Storage<T> {
    value: T
    history: Cell<Storage<T>>?
    status: Status = Status.Ready
    enabled: bool = false
    counter: uint64 = 18446744073709551615
}

struct (0b001) Message<T> {
    /// The message payload
    value: T
}

struct Empty<T> {}

struct GenericMessage<T> {
    value: T
}

struct Pair<L, R> {
    left: L
    right: R
}

type Either<L, R> = L | R

struct (0x123456789abc) WidePrefix {
    value: uint8
}

type Wrapped<T> = Message<T>

enum Status: uint8 {
    /// Ready to receive messages
    Ready = 0
    Busy = 7
}

struct Meta {
    value: uint8
}

struct `odd-name` {
    `field-name`: uint16
}

struct `type` {
    `fun`: uint8
}

type `odd-alias`<T> = (T, `odd-name`)

type WirePayload = RemainingBitsAndRefs

struct ClientMessage {
    @abi.clientType(Cell<Message<uint32>>)
    payload: WirePayload
}

type CustomInt = int32

fun CustomInt.packToBuilder(self, mutate b: builder) {
    b.storeInt(self, 32);
}

fun CustomInt.unpackFromSlice(mutate s: slice): CustomInt {
    return s.loadInt(32);
}

struct Defaults {
    count: int = 0
    large: uint64 = 18446744073709551615
    enabled: bool = false
    text: string = "hello\n\"world\""
    bits: slice = "ABCD".hexToSlice()
    owner: address = address("0:0000000000000000000000000000000000000000000000000000000000000000")
    pair: (int, bool) = (1, true)
    tupleValue: [int, bool] = [2, false]
    meta: Meta = { value: 7 }
    message: Message<uint32> = { value: 1 }
    wrapped: Wrapped<uint32> = { value: 2 }
    nested: (Message<uint32>, Meta) = ({ value: 3 }, { value: 4 })
    boxed: [Message<uint32>] = [{ value: 5 }]
    items: array<Message<uint32>> = [{ value: 6 }]
    state: Status = Status.Busy
    optional: int? = null
    custom: CustomInt = 42 as CustomInt
}

get fun get_storage(limit: int = 10): Storage<uint32> {
    return Storage<uint32> {
        value: limit as uint32,
        history: null,
    };
}

get fun get_wrapped(): Wrapped<address> {
    return Message<address> {
        value: address("0:0000000000000000000000000000000000000000000000000000000000000000"),
    };
}

get fun get_empty(): Empty<bool> {
    return {};
}

get fun get_defaults(value: Defaults = {}): Defaults {
    return value;
}

get fun get_client_message(value: ClientMessage): ClientMessage {
    return value;
}

get fun get_odd(value: `odd-alias`<uint32>): `odd-alias`<uint32> {
    return value;
}

get fun `return`(`val`: `type`): `type` {
    return `val`;
}

get fun get_pair(value: Pair<uint32, Wrapped<address>>): Pair<uint32, Wrapped<address>> {
    return value;
}

get fun get_either(value: Either<uint32, address>): Either<uint32, address> {
    return value;
}

get fun get_union(value: Message<uint32> | WidePrefix): Message<uint32> | WidePrefix {
    return value;
}

get fun get_optional(value: Storage<uint32>?): Storage<uint32>? {
    return value;
}

get fun get_types(
    maybe: (uint8 | address)?,
    cells: Cell<Message<uint32>>,
    items: array<`odd-name`>,
    linked: lisp_list<uint16>,
    mapping: map<uint32, Cell<Storage<address>>>,
    tupleValue: [int, (address, bool)],
    remainder: RemainingBitsAndRefs,
    any: any_address,
    bits: bits13,
    vi: varint16,
    vu: varuint32,
): (
    (uint8 | address)?,
    Cell<Message<uint32>>,
    array<`odd-name`>,
    lisp_list<uint16>,
    map<uint32, Cell<Storage<address>>>,
    [int, (address, bool)],
    RemainingBitsAndRefs,
    any_address,
    bits13,
    varint16,
    varuint32,
) {
    return (
        maybe,
        cells,
        items,
        linked,
        mapping,
        tupleValue,
        remainder,
        any,
        bits,
        vi,
        vu,
    );
}

fun onInternalMessage(_: InMessage) {}
