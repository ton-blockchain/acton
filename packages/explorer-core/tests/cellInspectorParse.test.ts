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
  test("recognizes a referenced metadata URI from a real jetton master", () => {
    const bocHex =
      "b5ee9c7201010201002400010001003e68747470733a2f2f7465746865722e746f2f757364742d746f6e2e6a736f6e"
    const result = parseCell(bocHex, {
      rootIndex: 0,
      strict: true,
      maxDepth: 4,
      customTlb: "",
    })

    if (result.status === "error") {
      throw new Error(result.error.message)
    }

    expect({
      parser: result.parser,
      provenance: result.provenance,
      parsedValue: result.parsedValue,
    }).toMatchInlineSnapshot(`
      {
        "parsedValue": {
          "entries": [
            {
              "key": "URI",
              "value": {
                "kind": "scalar",
                "typeName": "string",
                "value": "https://tether.to/usdt-ton.json",
              },
            },
          ],
          "kind": "object",
          "typeName": "Metadata URI",
        },
        "parser": "domain-parser",
        "provenance": {
          "confidence": {
            "level": "high",
            "reasons": [
              "The complete snake value is a valid UTF-8 metadata URI",
            ],
            "score": 0.9,
          },
          "details": {
            "layout": "Referenced snake string",
          },
          "engine": "domain-parser",
          "label": "Metadata URI",
          "source": "ton-domain",
        },
      }
    `)
  })

  test("recognizes real on-chain Jetton metadata", () => {
    const bocHex =
      "b5ee9c720102110100024600010300c00102012002040143bff082eb663b57a00192f4a6ac467288df2dfeddb9da1bee28f6521c8bebd21f1ec00300460068747470733a2f2f646f6765636f696e2e636f6d2f646f67652d6c6f676f2e706e67020120050a02012006080141bf4546a6ffe1b79cfdd86bad3db874313dcde2fb05e6a74aa7f3552d9617c79d1307001200444f4745434f494e0141bf6ed4f942a7848ce2cb066b77a1128c6a1ff8c43f438a2dce24612ba9ffab8b0309000a00444f47450201200b0f0141bf5208def46f5a1d4f9dce66ab309f4a851305f166f91ef79d923ef58e34f9a2090c01fe004174206974732068656172742c20446f6765636f696e20697320746865206163636964656e74616c2063727970746f206d6f76656d656e742074686174206d616b65732070656f706c6520736d696c652120497420697320616c736f20616e206f70656e736f7572636520706565722d746f2d706565722063727970746f0d01fc63757272656e63792074686174207574696c6973657320626c6f636b636861696e20746563686e6f6c6f67792c206120686967686c792073656375726520646563656e7472616c697365642073797374656d206f662073746f72696e6720696e666f726d6174696f6e2061732061207075626c6963206c656467657220740e0070686174206973206d61696e7461696e65642062792061206e6574776f726b206f6620636f6d7075746572732063616c6c6564206e6f6465730141bf5d01fa5e3c06901c45046c6b2ddcea5af764fea0eed72a10d404f2312ceb247d1000040039"
    const result = parseCell(bocHex, {
      rootIndex: 0,
      strict: true,
      maxDepth: 4,
      customTlb: "",
    })

    if (result.status === "error") {
      throw new Error(result.error.message)
    }

    expect({
      parser: result.parser,
      provenance: result.provenance,
      parsedValue: result.parsedValue,
    }).toMatchInlineSnapshot(`
      {
        "parsedValue": {
          "entries": [
            {
              "key": "Storage",
              "value": {
                "kind": "scalar",
                "value": "On-chain",
              },
            },
            {
              "key": "Name",
              "value": {
                "kind": "scalar",
                "typeName": "string",
                "value": "DOGECOIN",
              },
            },
            {
              "key": "Symbol",
              "value": {
                "kind": "scalar",
                "typeName": "string",
                "value": "DOGE",
              },
            },
            {
              "key": "Decimals",
              "value": {
                "kind": "scalar",
                "typeName": "string",
                "value": "9",
              },
            },
            {
              "key": "Description",
              "value": {
                "kind": "scalar",
                "typeName": "string",
                "value": "At its heart, Dogecoin is the accidental crypto movement that makes people smile! It is also an opensource peer-to-peer cryptocurrency that utilises blockchain technology, a highly secure decentralised system of storing information as a public ledger that is maintained by a network of computers called nodes",
              },
            },
            {
              "key": "Image",
              "value": {
                "kind": "scalar",
                "typeName": "string",
                "value": "https://dogecoin.com/doge-logo.png",
              },
            },
          ],
          "kind": "object",
          "typeName": "Jetton metadata",
        },
        "parser": "domain-parser",
        "provenance": {
          "confidence": {
            "level": "exact",
            "reasons": [
              "The TEP-64 on-chain prefix matched",
              "The metadata dictionary consumed the complete cell",
              "5 standard metadata fields matched",
            ],
            "score": 0.99,
          },
          "details": {
            "fields": "5",
            "storage": "On-chain",
          },
          "engine": "domain-parser",
          "label": "Jetton metadata · On-chain",
          "source": "ton-domain",
        },
      }
    `)
  })

  test("recognizes a referenced TON DNS smart-contract record", () => {
    const bocHex =
      "b5ee9c7201010201002a0001000100499fd3801f260860e6843a3f67200983bbd28ca5e8be6c4c81295fcb7111e933568646b88010"
    const result = parseCell(bocHex, {
      rootIndex: 0,
      strict: true,
      maxDepth: 4,
      customTlb: "",
    })

    if (result.status === "error") {
      throw new Error(result.error.message)
    }

    expect({
      status: result.status,
      parser: result.parser,
      provenance: result.provenance,
      parsedValue: result.parsedValue,
      cell: result.cell,
      warnings: result.warnings,
    }).toMatchInlineSnapshot(`
      {
        "cell": {
          "bits": 0,
          "depth": 1,
          "hash": "80528a2061ed78809d7efa1850ee1d140b8b73b1653e0ddf3c6f31ad46775055",
          "refs": 1,
          "rootCount": 1,
          "rootIndex": 0,
        },
        "parsedValue": {
          "entries": [
            {
              "key": "Record type",
              "value": {
                "kind": "scalar",
                "value": "Smart contract address",
              },
            },
            {
              "key": "Address",
              "value": {
                "kind": "address",
                "value": "0:f93043073421d1fb39004c1dde94652f45f36264094afe5b888f499ab43235c4",
              },
            },
            {
              "key": "Flags",
              "value": {
                "kind": "scalar",
                "typeName": "uint8",
                "value": "0",
              },
            },
            {
              "key": "Capabilities",
              "value": {
                "kind": "scalar",
                "value": "None",
              },
            },
          ],
          "kind": "object",
          "typeName": "TON DNS record",
        },
        "parser": "domain-parser",
        "provenance": {
          "confidence": {
            "level": "exact",
            "reasons": [
              "The DNS constructor tag matched",
              "The DNS record consumed the complete value",
              "The empty slice root contained exactly one value reference",
            ],
            "score": 0.99,
          },
          "details": {
            "layout": "Referenced value",
            "type": "Smart contract address",
          },
          "engine": "domain-parser",
          "label": "TON DNS · Smart contract address",
          "source": "ton-domain",
        },
        "status": "success",
        "warnings": [],
      }
    `)
  })

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
      parsedValue: {
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
      parsedValue: {
        kind: "object",
        typeName: "JettonInternalTransfer",
      },
    })
    if (result.parsedValue?.kind !== "object") {
      throw new Error("Expected the inferred ABI to decode an object")
    }
    expect(result.parsedValue.entries[0]).toMatchObject({
      key: "queryId",
      value: {kind: "scalar", value: "0"},
    })
    expect(result.parsedValue.entries[1]).toMatchObject({
      key: "amount",
      value: {kind: "scalar", value: "40000000"},
    })
  })

  test("keeps exotic library references inspectable", () => {
    const bocHex =
      "b5ee9c720101010100230008420212bebb0dc8e202b7e26f721e2547e16bb9ebaec934f657d19f22e76d62bec878"
    const [decodedRoot] = Cell.fromBoc(Buffer.from(bocHex, "hex"))

    expect(decodedRoot?.isExotic).toBe(true)
    expect(inferAbiByOpcode(decodedRoot, [])).toEqual({})

    const result = parseCell(bocHex, {
      rootIndex: 0,
      strict: true,
      maxDepth: 4,
      customTlb: "",
    })

    expect(result).toMatchObject({
      status: "unknown",
      parser: "raw-cell-tree",
      provenance: {label: "Raw exotic cell structure"},
      raw: {
        roots: [
          {
            exotic: true,
            type: "library-reference",
          },
        ],
      },
      cell: {
        bits: 264,
        refs: 0,
        rootCount: 1,
      },
    })

    const customResult = parseCell(bocHex, {
      rootIndex: 0,
      strict: true,
      maxDepth: 4,
      customTlb: "_ value:# = ExoticValue;",
      customTlbAuthoritative: true,
    })
    expect(customResult).toMatchObject({
      status: "unknown",
      parser: "raw-cell-tree",
      warnings: [
        {
          code: "custom-tlb-error",
          message: "Custom TL-B does not support exotic roots; showing raw cell structure",
        },
      ],
    })
  })
})
