import {describe, expect, test} from "bun:test"
import {beginCell} from "@ton/core"
import type {ContractABI} from "@ton/tolk-abi-to-typescript"

import {decodeCellWithAbi, type ExtendedContractABI} from "../src/lib/messageBody"

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
})
