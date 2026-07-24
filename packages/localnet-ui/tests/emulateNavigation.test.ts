import {describe, expect, test} from "bun:test"
import type {ContractABI} from "@acton/transaction-ui"
import {Address, beginCell, Cell, loadMessage, type Message} from "@ton/core"

import {
  createEmulateNavigationState,
  EMULATE_HANDOFF_QUERY_PARAM,
  readEmulateNavigationPayload,
  readStoredEmulateNavigationPayload,
  saveEmulateNavigationPayload,
} from "../src/explorer/pages/emulateNavigation"

const SOURCE = Address.parse("EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c")
const DESTINATION = Address.parse("EQAREREREREREREREREREREREREREREREREREREREREREeYT")
const SOURCE_ABI = {
  contract_name: "Sender",
  compiler_name: "tolk",
  compiler_version: "test",
  declarations: [
    {
      kind: "struct",
      name: "InternalTransferStep",
      ty_idx: 12,
      prefix: {prefix_num: 1, prefix_len: 32},
      fields: [{name: "value", ty_idx: 8}],
    },
  ],
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
    {kind: "StructRef", struct_name: "InternalTransferStep"},
  ],
  struct_instantiations: [],
  alias_instantiations: [],
  storage: {},
  incoming_messages: [],
  incoming_external: [],
  outgoing_messages: [{body_ty_idx: 12}],
  emitted_events: [],
  get_methods: [],
  thrown_errors: [],
} satisfies ContractABI
const DESTINATION_ABI = {
  ...SOURCE_ABI,
  contract_name: "Recipient",
  declarations: [
    {
      kind: "struct",
      name: "AskToTransfer",
      ty_idx: 12,
      fields: [
        {name: "opcode", ty_idx: 9},
        {name: "value", ty_idx: 8},
      ],
    },
  ],
  unique_types: [
    ...SOURCE_ABI.unique_types.slice(0, -1),
    {kind: "StructRef", struct_name: "AskToTransfer"},
  ],
  incoming_messages: [{body_ty_idx: 12}],
  outgoing_messages: [],
} satisfies ContractABI

describe("transaction to emulation navigation", () => {
  test("falls back to a complete raw message when the transaction has no ABI", () => {
    const message = createInternalMessage(
      beginCell().storeUint(0x12_34, 16).endCell(),
      1_250_000_000n,
    )

    const state = createEmulateNavigationState(message, {}, 42)
    const payload = readEmulateNavigationPayload(state)
    const restored = payload
      ? loadMessage(Cell.fromHex(payload.rawMessage).beginParse())
      : undefined
    const originalLocalStorage = Object.getOwnPropertyDescriptor(globalThis, "localStorage")
    const storedValues = new Map<string, string>()
    Object.defineProperty(globalThis, "localStorage", {
      configurable: true,
      value: {
        getItem: (key: string) => storedValues.get(key) ?? null,
        setItem: (key: string, value: string) => storedValues.set(key, value),
        removeItem: (key: string) => storedValues.delete(key),
      },
    })
    const payloadId = payload ? saveEmulateNavigationPayload(payload) : undefined
    const payloadParams = new URLSearchParams()
    if (payloadId) {
      payloadParams.set(EMULATE_HANDOFF_QUERY_PARAM, payloadId)
    }
    const storedPayload = readStoredEmulateNavigationPayload(payloadParams)
    if (originalLocalStorage) {
      Object.defineProperty(globalThis, "localStorage", originalLocalStorage)
    } else {
      Reflect.deleteProperty(globalThis, "localStorage")
    }

    expect({
      payload,
      storedPayloadMatches: JSON.stringify(storedPayload) === JSON.stringify(payload),
      restored:
        restored?.info.type === "internal"
          ? {
              source: restored.info.src.toString(),
              destination: restored.info.dest.toString(),
              value: restored.info.value.coins.toString(),
              bounce: restored.info.bounce,
              body: restored.body.bits.toString(),
            }
          : undefined,
      invalidState: readEmulateNavigationPayload({emulatePayload: {inputMode: "raw"}}),
    }).toMatchSnapshot()
  })

  test("uses raw mode instead of guessing an ABI without a message name", () => {
    const message = createInternalMessage(beginCell().storeUint(1, 32).storeInt(7, 32).endCell())

    const state = createEmulateNavigationState(message, {source: SOURCE_ABI}, 42)

    expect(navigationPayloadSnapshot(state)).toMatchSnapshot()
  })

  test("uses the sender outgoing ABI when the destination ABI does not decode the message", () => {
    const message = createInternalMessage(beginCell().storeUint(1, 32).storeInt(7, 32).endCell())

    const state = createEmulateNavigationState(
      message,
      {source: SOURCE_ABI},
      42,
      "InternalTransferStep",
    )

    expect(navigationPayloadSnapshot(state)).toMatchSnapshot()
  })

  test("uses the named source message instead of a false-positive destination decode", () => {
    const message = createInternalMessage(beginCell().storeUint(1, 32).storeInt(7, 32).endCell())

    const state = createEmulateNavigationState(
      message,
      {destination: DESTINATION_ABI, source: SOURCE_ABI},
      42,
      "InternalTransferStep",
    )

    expect(navigationPayloadSnapshot(state)).toMatchSnapshot()
  })
})

function createInternalMessage(body: Cell, coins = 250_000_000n): Message {
  return {
    info: {
      type: "internal",
      ihrDisabled: true,
      bounce: false,
      bounced: false,
      src: SOURCE,
      dest: DESTINATION,
      value: {coins},
      ihrFee: 0n,
      forwardFee: 0n,
      createdLt: 17n,
      createdAt: 23,
    },
    body,
  }
}

function navigationPayloadSnapshot(state: unknown): unknown {
  const payload = readEmulateNavigationPayload(state)
  if (payload?.inputMode !== "builder") {
    return payload
  }

  const {argsJson, ...builder} = payload.builder
  return {
    ...payload,
    builder: {
      ...builder,
      args: JSON.parse(argsJson),
    },
  }
}
