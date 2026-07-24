import {describe, expect, test} from "bun:test"
import {Address, beginCell, BitString, type Cell, Dictionary, external} from "@ton/core"
import type {ContractABI} from "@ton/tolk-abi-to-typescript"

import {
  decodeCellWithAbi,
  decodeMessageBody,
  type ExtendedContractABI,
  getMessageOpcode,
} from "../src/lib/messageBody"
import type {ContractData} from "../src/model/transaction"

const counterAbi: ContractABI = {
  alias_instantiations: [],
  compiler_name: "tolk",
  compiler_version: "1.4.2",
  contract_name: "Counter",
  declarations: [
    {
      fields: [{name: "counter", ty_idx: 8}],
      kind: "struct",
      name: "Storage",
      ty_idx: 13,
    },
    {
      fields: [{name: "value", ty_idx: 8}],
      kind: "struct",
      name: "Increment",
      prefix: {prefix_len: 32, prefix_num: 1},
      ty_idx: 12,
    },
  ],
  emitted_events: [],
  get_methods: [],
  incoming_external: [],
  incoming_messages: [{body_ty_idx: 12}],
  outgoing_messages: [],
  storage: {storage_ty_idx: 13},
  struct_instantiations: [],
  thrown_errors: [],
  unique_types: [
    {kind: "void"},
    {kind: "int"},
    {kind: "slice"},
    {kind: "cell"},
    {kind: "builder"},
    {kind: "bool"},
    {kind: "coins"},
    {kind: "address"},
    {kind: "intN", n: 32},
    {kind: "uintN", n: 32},
    {kind: "intN", n: 64},
    {kind: "uintN", n: 64},
    {kind: "StructRef", struct_name: "Increment"},
    {kind: "StructRef", struct_name: "Storage"},
  ],
}

const extendedAbi: ExtendedContractABI = {
  compiler_abi: counterAbi,
  display_name: "Counter",
  code_hashes: ["counter-code-hash"],
}

const enumAbi: ContractABI = {
  ...counterAbi,
  declarations: [
    ...counterAbi.declarations,
    {
      encoded_as_ty_idx: 9,
      kind: "enum",
      members: [
        {name: "Disabled", value: "0"},
        {name: "Enabled", value: "1"},
      ],
      name: "FeatureMode",
      ty_idx: 14,
    },
    {
      fields: [{name: "mode", ty_idx: 14}],
      kind: "struct",
      name: "SetFeatureMode",
      prefix: {prefix_len: 32, prefix_num: 2},
      ty_idx: 15,
    },
  ],
  incoming_messages: [{body_ty_idx: 15}],
  unique_types: [
    ...counterAbi.unique_types,
    {kind: "EnumRef", enum_name: "FeatureMode"},
    {kind: "StructRef", struct_name: "SetFeatureMode"},
  ],
}

const enumExtendedAbi: ExtendedContractABI = {
  compiler_abi: enumAbi,
  display_name: "EnumExample",
  code_hashes: ["enum-example-code-hash"],
}

const bitsDictionaryAbi: ContractABI = {
  ...counterAbi,
  incoming_messages: [],
  storage: {storage_ty_idx: 15},
  unique_types: [
    ...counterAbi.unique_types,
    {kind: "bitsN", n: 8},
    {kind: "mapKV", key_ty_idx: 14, value_ty_idx: 0},
  ],
}

const bitsDictionaryExtendedAbi: ExtendedContractABI = {
  compiler_abi: bitsDictionaryAbi,
  display_name: "BitsDictionary",
  code_hashes: ["bits-dictionary-code-hash"],
}

type DeclaredMessageDirection = "incoming-internal" | "incoming-external" | "outgoing"

const createUintMessageAbi = ({
  contractName,
  messageName,
  direction,
  prefix,
}: {
  readonly contractName: string
  readonly messageName: string
  readonly direction: DeclaredMessageDirection
  readonly prefix?: number
}): ContractABI => ({
  ...counterAbi,
  contract_name: contractName,
  declarations: [
    {
      fields: [{name: "value", ty_idx: 9}],
      kind: "struct",
      name: messageName,
      ...(prefix === undefined ? {} : {prefix: {prefix_len: 32, prefix_num: prefix}}),
      ty_idx: 12,
    },
    ...counterAbi.declarations.filter(
      declaration => declaration.kind === "struct" && declaration.name === "Storage",
    ),
  ],
  incoming_external: direction === "incoming-external" ? [{body_ty_idx: 12}] : [],
  incoming_messages: direction === "incoming-internal" ? [{body_ty_idx: 12}] : [],
  outgoing_messages: direction === "outgoing" ? [{body_ty_idx: 12}] : [],
  unique_types: counterAbi.unique_types.map((type, index) =>
    index === 12 ? {kind: "StructRef", struct_name: messageName} : type,
  ),
})

const walletV4LikeAbi: ContractABI = {
  ...counterAbi,
  contract_name: "WalletV4Like",
  declarations: [
    {
      kind: "alias",
      name: "WalletV4ExternalMessage",
      target_ty_idx: 15,
      ty_idx: 12,
    },
    {
      fields: [
        {name: "signature", ty_idx: 14},
        {name: "subwalletId", ty_idx: 9},
        {name: "validUntil", ty_idx: 9},
        {name: "seqno", ty_idx: 9},
      ],
      kind: "struct",
      name: "WalletV4SignedExternal",
      ty_idx: 15,
    },
    ...counterAbi.declarations.filter(
      declaration => declaration.kind === "struct" && declaration.name === "Storage",
    ),
  ],
  incoming_external: [{body_ty_idx: 12}],
  incoming_messages: [],
  outgoing_messages: [],
  unique_types: [
    ...counterAbi.unique_types.slice(0, 12),
    {kind: "AliasRef", alias_name: "WalletV4ExternalMessage"},
    counterAbi.unique_types[13],
    {kind: "bitsN", n: 512},
    {kind: "StructRef", struct_name: "WalletV4SignedExternal"},
  ],
}

const senderAddress = new Address(0, Buffer.alloc(32, 0x11))
const receiverAddress = new Address(0, Buffer.alloc(32, 0x22))
const fallbackAddress = new Address(0, Buffer.alloc(32, 0x33))

const contractData = (address: Address, abi: ContractABI): ContractData => ({
  abi,
  address,
  displayName: abi.contract_name,
  letter: abi.contract_name.slice(0, 1),
})

const contractsByAddress = (
  ...contracts: readonly [address: Address, abi: ContractABI][]
): Map<string, ContractData> =>
  new Map(contracts.map(([address, abi]) => [address.toString(), contractData(address, abi)]))

const internalMessage = (body: Cell, bounced = false) => ({
  body,
  info: {
    bounce: true,
    bounced,
    createdAt: 0,
    createdLt: 0n,
    dest: receiverAddress,
    forwardFee: 0n,
    ihrDisabled: true,
    ihrFee: 0n,
    src: senderAddress,
    type: "internal" as const,
    value: {coins: 0n},
  },
})

const externalOutMessage = (body: Cell) => ({
  body,
  info: {
    createdAt: 0,
    createdLt: 0n,
    src: senderAddress,
    type: "external-out" as const,
  },
})

describe("decodeMessageBody", () => {
  test("decodes a single prefixless Wallet V4 external message before a text comment", () => {
    const body = beginCell()
      .storeUint(0n, 512)
      .storeUint(698_983_191, 32)
      .storeUint(1_900_000_000, 32)
      .storeUint(42, 32)
      .endCell()
    const message = external({to: receiverAddress, body})
    const parsedBody = decodeMessageBody(
      message,
      contractsByAddress([receiverAddress, walletV4LikeAbi]),
    )

    expect({
      decodedOpcode: getMessageOpcode(message, parsedBody),
      parsedBody,
      rawFirst32Bits: getMessageOpcode(message),
    }).toMatchSnapshot()
  })

  test("prefers receiver incoming over sender outgoing for an internal message", () => {
    const receiverAbi = createUintMessageAbi({
      contractName: "InternalReceiver",
      messageName: "ReceiverIncoming",
      direction: "incoming-internal",
    })
    const senderAbi = createUintMessageAbi({
      contractName: "InternalSender",
      messageName: "SenderOutgoing",
      direction: "outgoing",
    })
    const body = beginCell().storeUint(0, 32).endCell()

    expect(
      decodeMessageBody(
        internalMessage(body),
        contractsByAddress([senderAddress, senderAbi], [receiverAddress, receiverAbi]),
      ),
    ).toMatchSnapshot()
  })

  test("falls back to sender outgoing when receiver incoming does not match", () => {
    const receiverAbi = createUintMessageAbi({
      contractName: "InternalReceiver",
      messageName: "ReceiverIncoming",
      direction: "incoming-internal",
      prefix: 1,
    })
    const senderAbi = createUintMessageAbi({
      contractName: "InternalSender",
      messageName: "SenderOutgoing",
      direction: "outgoing",
    })
    const body = beginCell().storeUint(0, 32).endCell()

    expect(
      decodeMessageBody(
        internalMessage(body),
        contractsByAddress([senderAddress, senderAbi], [receiverAddress, receiverAbi]),
      ),
    ).toMatchSnapshot()
  })

  test("prefers the message source over the caller-provided source fallback", () => {
    const receiverAbi = createUintMessageAbi({
      contractName: "InternalReceiver",
      messageName: "ReceiverIncoming",
      direction: "incoming-internal",
      prefix: 1,
    })
    const senderAbi = createUintMessageAbi({
      contractName: "InternalSender",
      messageName: "ActualSenderOutgoing",
      direction: "outgoing",
    })
    const fallbackAbi = createUintMessageAbi({
      contractName: "FallbackSender",
      messageName: "FallbackSenderOutgoing",
      direction: "outgoing",
    })
    const body = beginCell().storeUint(0, 32).endCell()

    expect(
      decodeMessageBody(
        internalMessage(body),
        contractsByAddress(
          [senderAddress, senderAbi],
          [receiverAddress, receiverAbi],
          [fallbackAddress, fallbackAbi],
        ),
        fallbackAddress.toString(),
      ),
    ).toMatchSnapshot()
  })

  test("decodes external-out using the sender outgoing ABI", () => {
    const senderAbi = createUintMessageAbi({
      contractName: "ExternalSender",
      messageName: "SenderExternalOutgoing",
      direction: "outgoing",
    })
    const body = beginCell().storeUint(0, 32).endCell()

    expect(
      decodeMessageBody(externalOutMessage(body), contractsByAddress([senderAddress, senderAbi])),
    ).toMatchSnapshot()
  })

  test("uses the caller-provided source for a relaxed message without src", () => {
    const senderAbi = createUintMessageAbi({
      contractName: "RelaxedSender",
      messageName: "RelaxedSenderOutgoing",
      direction: "outgoing",
    })
    const body = beginCell().storeUint(0, 32).endCell()
    const outgoingMessage = externalOutMessage(body)
    const message = {
      ...outgoingMessage,
      info: {...outgoingMessage.info, src: undefined},
    }

    expect(
      decodeMessageBody(
        message,
        contractsByAddress([senderAddress, senderAbi]),
        senderAddress.toString(),
      ),
    ).toMatchSnapshot()
  })

  test("selects the matching opcode when the receiver ABI has multiple messages", () => {
    const receiverAbi: ContractABI = {
      ...enumAbi,
      incoming_external: [{body_ty_idx: 12}, {body_ty_idx: 15}],
      incoming_messages: [],
    }
    const body = beginCell().storeUint(2, 32).storeUint(1, 32).endCell()

    expect(
      decodeMessageBody(
        external({to: receiverAddress, body}),
        contractsByAddress([receiverAddress, receiverAbi]),
      ),
    ).toMatchSnapshot()
  })

  test("does not guess between multiple prefixless messages without an opcode", () => {
    const receiverAbi: ContractABI = {
      ...createUintMessageAbi({
        contractName: "AmbiguousReceiver",
        messageName: "FirstPrefixlessMessage",
        direction: "incoming-external",
      }),
      incoming_external: [{body_ty_idx: 12}, {body_ty_idx: 13}],
    }
    const body = beginCell().storeUint(0, 32).endCell()

    expect(
      decodeMessageBody(
        external({to: receiverAddress, body}),
        contractsByAddress([receiverAddress, receiverAbi]),
      ),
    ).toMatchSnapshot()
  })

  test("decodes a single prefixless bounced message without an opcode", () => {
    const receiverAbi = createUintMessageAbi({
      contractName: "BouncedReceiver",
      messageName: "OriginalOutgoing",
      direction: "outgoing",
    })
    const body = beginCell().storeUint(0xff_ff_ff_ff, 32).storeUint(0, 32).endCell()

    expect(
      decodeMessageBody(
        internalMessage(body, true),
        contractsByAddress([receiverAddress, receiverAbi]),
      ),
    ).toMatchSnapshot()
  })

  test("decodes a short prefixless bounced message with fewer than 32 bits", () => {
    const baseAbi = createUintMessageAbi({
      contractName: "ShortBouncedReceiver",
      messageName: "ShortOriginalOutgoing",
      direction: "outgoing",
    })
    const receiverAbi: ContractABI = {
      ...baseAbi,
      declarations: baseAbi.declarations.map(declaration =>
        declaration.kind === "struct" && declaration.name === "ShortOriginalOutgoing"
          ? {...declaration, fields: [{name: "value", ty_idx: 14}]}
          : declaration,
      ),
      unique_types: [...baseAbi.unique_types, {kind: "uintN", n: 8}],
    }
    const body = beginCell().storeUint(7, 8).endCell()

    expect(
      decodeMessageBody(
        internalMessage(body, true),
        contractsByAddress([receiverAddress, receiverAbi]),
      ),
    ).toMatchSnapshot()
  })

  test("tries fallback contract ABIs after both endpoint ABIs", () => {
    const receiverAbi = createUintMessageAbi({
      contractName: "InternalReceiver",
      messageName: "ReceiverIncoming",
      direction: "incoming-internal",
      prefix: 1,
    })
    const senderAbi = createUintMessageAbi({
      contractName: "InternalSender",
      messageName: "SenderOutgoing",
      direction: "outgoing",
      prefix: 1,
    })
    const fallbackAbi = createUintMessageAbi({
      contractName: "FallbackContract",
      messageName: "FallbackIncoming",
      direction: "incoming-internal",
    })
    const body = beginCell().storeUint(0, 32).endCell()

    expect(
      decodeMessageBody(
        internalMessage(body),
        contractsByAddress(
          [senderAddress, senderAbi],
          [receiverAddress, receiverAbi],
          [fallbackAddress, fallbackAbi],
        ),
      ),
    ).toMatchSnapshot()
  })

  test("decodes a text comment only after receiver and sender ABIs fail", () => {
    const receiverAbi = createUintMessageAbi({
      contractName: "InternalReceiver",
      messageName: "ReceiverIncoming",
      direction: "incoming-internal",
      prefix: 1,
    })
    const senderAbi = createUintMessageAbi({
      contractName: "InternalSender",
      messageName: "SenderOutgoing",
      direction: "outgoing",
      prefix: 1,
    })
    const body = beginCell().storeUint(0, 32).storeStringTail("hello").endCell()

    expect(
      decodeMessageBody(
        internalMessage(body),
        contractsByAddress([senderAddress, senderAbi], [receiverAddress, receiverAbi]),
      ),
    ).toMatchSnapshot()
  })

  test("returns undefined when no ABI or built-in parser matches", () => {
    const receiverAbi = createUintMessageAbi({
      contractName: "InternalReceiver",
      messageName: "ReceiverIncoming",
      direction: "incoming-internal",
      prefix: 1,
    })
    const body = beginCell().storeUint(0xff, 8).endCell()

    expect(
      decodeMessageBody(internalMessage(body), contractsByAddress([receiverAddress, receiverAbi])),
    ).toMatchSnapshot()
  })
})

describe("decodeCellWithAbi", () => {
  test("decodes text comments before ABI candidates", () => {
    const cell = beginCell().storeUint(0, 32).storeStringTail("hello").endCell()

    expect(decodeCellWithAbi(cell, extendedAbi)).toEqual({
      category: "comment",
      name: "Text Comment",
      value: {
        kind: "object",
        typeName: "Text Comment",
        entries: [{key: "text", value: {kind: "scalar", value: "hello"}}],
      },
      provenance: {source: "text-comment", parser: "built-in"},
    })
  })

  test("decodes an internal message body with registry provenance", () => {
    const cell = beginCell().storeUint(1, 32).storeInt(7, 32).endCell()

    expect(decodeCellWithAbi(cell, extendedAbi)).toEqual({
      category: "message",
      direction: "incoming-internal",
      name: "Increment",
      consumption: {
        initialBits: 64,
        initialRefs: 0,
        remainingBits: 0,
        remainingRefs: 0,
        complete: true,
      },
      value: {
        kind: "object",
        typeName: "Increment",
        entries: [{key: "value", value: {kind: "scalar", value: "7", typeName: "int32"}}],
      },
      provenance: {
        source: "compiler-abi",
        displayName: "Counter",
        codeHashes: ["counter-code-hash"],
      },
    })
  })

  test("reports the ABI candidate group that matched", () => {
    const cell = beginCell().storeUint(1, 32).storeInt(7, 32).endCell()
    const externalAbi: ExtendedContractABI = {
      ...extendedAbi,
      compiler_abi: {
        ...counterAbi,
        incoming_messages: [],
        incoming_external: [{body_ty_idx: 12}],
      },
    }
    const outgoingAbi: ExtendedContractABI = {
      ...extendedAbi,
      compiler_abi: {
        ...counterAbi,
        incoming_messages: [],
        outgoing_messages: [{body_ty_idx: 12}],
      },
    }

    expect(decodeCellWithAbi(cell, externalAbi)?.direction).toBe("incoming-external")
    expect(decodeCellWithAbi(cell, outgoingAbi)?.direction).toBe("outgoing")
  })

  test("does not invent a direction when the same payload is declared twice", () => {
    const cell = beginCell().storeUint(1, 32).storeInt(7, 32).endCell()
    const ambiguousAbi: ExtendedContractABI = {
      ...extendedAbi,
      compiler_abi: {
        ...counterAbi,
        outgoing_messages: [{body_ty_idx: 12}],
      },
    }

    expect(decodeCellWithAbi(cell, ambiguousAbi)).toMatchObject({
      category: "message",
      directionCandidates: ["incoming-internal", "outgoing"],
    })
    expect(decodeCellWithAbi(cell, ambiguousAbi)?.direction).toBeUndefined()
  })

  test("falls back to storage when no message schema matches", () => {
    const cell = beginCell().storeInt(7, 32).endCell()

    expect(decodeCellWithAbi(cell, extendedAbi)).toEqual({
      category: "storage",
      name: "Storage",
      consumption: {
        initialBits: 32,
        initialRefs: 0,
        remainingBits: 0,
        remainingRefs: 0,
        complete: true,
      },
      value: {
        kind: "object",
        typeName: "Storage",
        entries: [{key: "counter", value: {kind: "scalar", value: "7", typeName: "int32"}}],
      },
      provenance: {
        source: "compiler-abi",
        displayName: "Counter",
        codeHashes: ["counter-code-hash"],
      },
    })
  })

  test("returns undefined when the cell matches no ABI schema", () => {
    expect(decodeCellWithAbi(beginCell().storeUint(0xff, 8).endCell(), extendedAbi)).toBeUndefined()
  })

  test("reports accepted trailing message bits", () => {
    const cell = beginCell().storeUint(1, 32).storeInt(7, 32).storeUint(0xff, 8).endCell()

    expect(decodeCellWithAbi(cell, extendedAbi)?.consumption).toEqual({
      initialBits: 72,
      initialRefs: 0,
      remainingBits: 8,
      remainingRefs: 0,
      complete: false,
    })
  })

  test("decodes storage maps with bits keys through the compatibility fallback", () => {
    const values = Dictionary.empty(Dictionary.Keys.BitString(8), {
      serialize() {
        // Values are void, so dictionary leaves contain no value bits.
      },
      parse() {
        return undefined
      },
    })
    values.set(new BitString(Buffer.from([0xab]), 0, 8), undefined)
    const cell = beginCell().storeDict(values).endCell()

    expect(decodeCellWithAbi(cell, bitsDictionaryExtendedAbi)).toMatchSnapshot()
  })

  test("preserves enum member names and unknown encoded values", () => {
    const decodeMode = (mode: number) =>
      decodeCellWithAbi(beginCell().storeUint(2, 32).storeUint(mode, 32).endCell(), enumExtendedAbi)

    expect({known: decodeMode(1), unknown: decodeMode(7)}).toMatchSnapshot()
  })
})
