import {describe, expect, test} from "bun:test"
import {beginCell, Cell} from "@ton/core"
import type {ContractABI} from "@ton/tolk-abi-to-typescript"

import bundledAbiCatalog from "../../../crates/acton-abi-catalog/data/data-abis.json"
import {inferAbiByOpcode, parseCell} from "../src/cell-inspector"
import type {ExtendedContractABI} from "../src/api/compilerAbi"

const compilerAbi: ContractABI = {
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

const registryAbi: ExtendedContractABI = {
  compiler_abi: compilerAbi,
  display_name: "Counter",
  code_hashes: ["counter-code-hash"],
}

const defaultOptions = {
  rootIndex: 0,
  strict: true,
  maxDepth: 4,
  customTlb: "",
  abi: registryAbi,
  abiCodeHash: "counter-code-hash",
} as const

describe("Cell Inspector parser pipeline", () => {
  test("uses the resolved registry ABI before generic TL-B parsers", () => {
    const cell = beginCell().storeUint(1, 32).storeInt(7, 32).endCell()
    const result = parseCell(cell.toBoc().toString("base64"), defaultOptions)

    expect(result).toMatchObject({
      status: "success",
      parser: "abi-registry",
      provenance: {
        label: "Counter · Increment",
        source: "abi-registry",
        details: {
          category: "message",
          codeHash: "counter-code-hash",
          direction: "incoming-internal",
          value: "Increment",
        },
      },
      abiValue: {
        kind: "object",
        typeName: "Increment",
        entries: [{key: "value", value: {kind: "scalar", typeName: "int32", value: "7"}}],
      },
      warnings: [],
    })
  })

  test("uses only custom TL-B when it is explicitly preferred", () => {
    const cell = beginCell().storeUint(1, 32).storeUint(7, 32).endCell()
    const input = cell.toBoc().toString("base64")
    const customTlb = "_ opcode:# value:# = CustomValue;"

    expect(parseCell(input, {...defaultOptions, customTlb}).parser).toBe("abi-registry")
    expect(
      parseCell(input, {...defaultOptions, customTlb, customTlbAuthoritative: true}),
    ).toMatchObject({
      status: "partial",
      parser: "custom-tlb",
      data: {kind: "CustomValue", opcode: 1, value: 7},
      provenance: {source: "user-schema"},
    })
  })

  test("reports a preferred custom TL-B failure without falling back to ABI", () => {
    const cell = beginCell().storeUint(1, 32).storeUint(7, 32).endCell()
    const result = parseCell(cell.toBoc().toString("base64"), {
      ...defaultOptions,
      customTlb: "_ value:^Cell = CustomValue;",
      customTlbAuthoritative: true,
    })

    expect(result).toMatchObject({
      status: "error",
      error: {
        code: "custom-tlb-failed",
        message: "Custom TL-B could not decode this root",
      },
      warnings: [{code: "custom-tlb-error"}],
    })
  })

  test("keeps built-in comment provenance even when an ABI is selected", () => {
    const cell = beginCell().storeUint(0, 32).storeStringTail("hello TON").endCell()
    const result = parseCell(cell.toBoc().toString("base64"), defaultOptions)

    expect(result).toMatchObject({
      status: "success",
      parser: "standard-comment",
      provenance: {label: "Text comment", source: "ton-standard"},
      data: {kind: "text-comment", text: "hello TON"},
    })
  })

  test("accepts an ABI decode with trailing bits only in relaxed mode", () => {
    const cell = beginCell().storeUint(1, 32).storeInt(7, 32).storeUint(0xff, 8).endCell()
    const boc = cell.toBoc().toString("base64")
    const result = parseCell(boc, {...defaultOptions, strict: false})

    expect(result).toMatchObject({
      status: "partial",
      parser: "abi-registry",
      provenance: {confidence: {score: 0.9, level: "high"}},
      warnings: [
        {
          code: "partial-match",
          message: "This ABI decoded the value but left 8 bits and 0 references unread",
        },
      ],
    })

    const strictResult = parseCell(boc, defaultOptions)
    expect(strictResult.parser).not.toBe("abi-registry")
    expect(strictResult.warnings).toContainEqual({
      code: "partial-match",
      message: "Strict parsing ignored this ABI because 8 bits and 0 references remained unread",
    })
  })

  test("silently falls back when an automatically discovered ABI does not decode", () => {
    const bocHex =
      "b5ee9c720102070100010500020a0ec3c86d5001020000026162002aef29b142b4239f0d70edb653da95568b394f6da9c2ef92ad64e546dba508e20000000000000000000000000003c0030602013404050842028f452d7a4dfd74066b682365177259ed05734435be76b5fd4bd5d8af2b7c3d68008700800415d66e65d7160a8e2a1b344e2f09454ef19b569454fd5e36f14958df1e9b247002c44ea652d4092859c67da44e4ca3add6565b0e2897d640a2c51bfb370d8877fa00a9178d45190000000000000000402625a008011ac445debca569067cf73f05b9545361d0dd2c5bad6549bafb73bad27e85c7db00235888bbd794ad20cf9ee7e0b72a8a6c3a1ba58b75aca9375f6e775a4fd0b8fb4405"
    const result = parseCell(bocHex, {
      ...defaultOptions,
      warnOnAbiMismatch: false,
    })

    expect(result).toMatchObject({
      status: "success",
      parser: "block-tlb",
      provenance: {label: "TON block.tlb · OutList"},
      warnings: [],
    })
  })

  test("infers the common ABI schema for a jetton internal transfer opcode", () => {
    const bocHex =
      "b5ee9c724101010100570000a9178d45190000000000000000402625a008011ac445debca569067cf73f05b9545361d0dd2c5bad6549bafb73bad27e85c7db00235888bbd794ad20cf9ee7e0b72a8a6c3a1ba58b75aca9375f6e775a4fd0b8fb44054e0b1389"
    const [decodedRoot] = Cell.fromBoc(Buffer.from(bocHex, "hex"))
    const candidates = bundledAbiCatalog.contracts.map(entry => ({
      abi: {
        compiler_abi: entry.compilerAbi as ContractABI,
        display_name: entry.displayName,
        code_hashes: entry.hashes,
        links: entry.links ?? [],
      },
    }))
    const inference = inferAbiByOpcode(decodedRoot, candidates)
    if (!inference.abi || inference.confidenceScore === undefined) {
      throw new Error("Expected ABI inference to resolve the jetton payload")
    }

    expect(inference).toMatchObject({
      abi: {display_name: "ABI catalog"},
      confidenceScore: 0.7,
      warning: {code: "ambiguous-match", message: expect.stringContaining("0x178d4519")},
    })

    const result = parseCell(bocHex, {
      rootIndex: 0,
      strict: true,
      maxDepth: 4,
      customTlb: "",
      abi: inference.abi,
      abiConfidence: {
        score: inference.confidenceScore,
        reason: inference.confidenceReason ?? "ABI inferred by opcode",
      },
    })
    expect(result).toMatchObject({
      parser: "abi-registry",
      provenance: {
        label: "ABI catalog · JettonInternalTransfer",
        confidence: {score: 0.7, level: "medium"},
      },
      abiValue: {
        kind: "object",
        typeName: "JettonInternalTransfer",
      },
    })
    if (result.abiValue?.kind !== "object") {
      throw new Error("Expected the inferred ABI to decode an object")
    }
    expect(result.abiValue.entries[0]).toMatchObject({
      key: "queryId",
      value: {kind: "scalar", value: "0"},
    })
    expect(result.abiValue.entries[1]).toMatchObject({
      key: "amount",
      value: {kind: "scalar", value: "40000000"},
    })
  })
})
