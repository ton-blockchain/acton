import {describe, expect, test} from "bun:test"
import {beginCell, Cell, storeStateInit} from "@ton/core"
import {Buffer} from "buffer"

import {
  decodeCellInput,
  describeCellForest,
  ENCRYPTED_COMMENT_OPCODE,
  normalizeCellInput,
  parseBlockTlb,
  recognizeStandardComment,
  toSerializable,
  tryParseCustomTlb,
} from "../src/explorer/cell-inspector"

describe("cell inspector input", () => {
  const cell = beginCell().storeUint(0x12_34, 16).endCell()
  const base64 = cell.toBoc().toString("base64")

  test("normalizes whitespace in base64 and decodes the selected root", () => {
    const spaced = base64.match(/.{1,5}/g)?.join(" \n") ?? base64
    const result = decodeCellInput(spaced)

    expect(result.ok).toBe(true)
    if (!result.ok) return
    expect(result.decoded.selectedRoot.hash().equals(cell.hash())).toBe(true)
    expect(result.decoded.selectedRootIndex).toBe(0)
    expect(result.decoded.roots).toHaveLength(1)
  })

  test("extracts a percent-encoded BoC from a link", () => {
    const result = normalizeCellInput(
      `https://example.test/cell?payload=${encodeURIComponent(base64)}`,
    )

    expect(result).toMatchObject({
      ok: true,
      input: {kind: "base64", source: "link"},
    })
  })

  test("accepts canonical BoC hex", () => {
    const result = normalizeCellInput(`0x${cell.toBoc().toString("hex")}`)
    expect(result).toMatchObject({ok: true, input: {kind: "hex", source: "direct"}})
  })

  test("reports an invalid root index without throwing", () => {
    const result = decodeCellInput(base64, {rootIndex: 1})
    expect(result).toMatchObject({ok: false, error: {code: "root-out-of-range"}})
  })

  test("preserves and selects roots from a multi-root BoC", () => {
    const singleRootBoc = cell.toBoc({idx: false, crc32: false})
    // This small BoC uses one-byte counters. Add a second entry pointing at the same root cell.
    const multiRootBoc = Buffer.concat([
      singleRootBoc.subarray(0, 7),
      Buffer.from([2]),
      singleRootBoc.subarray(8, 11),
      Buffer.from([0]),
      singleRootBoc.subarray(11),
    ])
    const result = decodeCellInput(multiRootBoc.toString("base64"), {rootIndex: 1})

    expect(result).toMatchObject({
      ok: true,
      decoded: {selectedRootIndex: 1, byteLength: multiRootBoc.length},
    })
    if (!result.ok) return
    expect(result.decoded.roots).toHaveLength(2)
    expect(result.decoded.selectedRoot.hash().equals(cell.hash())).toBe(true)
  })
})

describe("raw cell forest", () => {
  test("represents shared DAG nodes as references", () => {
    const shared = beginCell().storeUint(7, 8).endCell()
    const root = beginCell().storeRef(shared).storeRef(shared).endCell()
    const forest = describeCellForest([root])

    expect(forest.uniqueNodes).toBe(2)
    expect(forest.duplicateRefs).toBe(1)
    expect(forest.roots[0]?.refs.items[1]).toMatchObject({
      kind: "cell-ref",
      targetPath: "$[0].refs[0]",
    })
  })

  test("stops recursive output at the configured depth", () => {
    const leaf = beginCell().storeUint(1, 1).endCell()
    const middle = beginCell().storeRef(leaf).endCell()
    const root = beginCell().storeRef(middle).endCell()
    const forest = describeCellForest([root], {maxDepth: 1})

    expect(forest.truncatedNodes).toBe(1)
    expect(forest.warnings).toContainEqual(
      expect.objectContaining({code: "tree-depth-limit", path: "$[0].refs[0].refs[0]"}),
    )
  })
})

describe("standard comments", () => {
  test("decodes a UTF-8 text comment", () => {
    const cell = beginCell().storeUint(0, 32).storeStringTail("hello TON").endCell()
    expect(recognizeStandardComment(cell)).toMatchObject({
      kind: "text-comment",
      opcode: "0x00000000",
      text: "hello TON",
      warnings: [],
    })
  })

  test("recognizes encrypted comments without pretending to decrypt them", () => {
    const cell = beginCell()
      .storeUint(ENCRYPTED_COMMENT_OPCODE, 32)
      .storeBuffer(Buffer.from([0xde, 0xad, 0xbe, 0xef]))
      .endCell()
    expect(recognizeStandardComment(cell)).toMatchObject({
      kind: "encrypted-comment",
      encrypted: true,
      payloadHex: "deadbeef",
      warnings: [expect.objectContaining({code: "decryption-key-required"})],
    })
  })
})

describe("semantic helpers", () => {
  test("parses a raw OutList BoC", () => {
    const bocHex =
      "b5ee9c720102070100010500020a0ec3c86d5001020000026162002aef29b142b4239f0d70edb653da95568b394f6da9c2ef92ad64e546dba508e20000000000000000000000000003c0030602013404050842028f452d7a4dfd74066b682365177259ed05734435be76b5fd4bd5d8af2b7c3d68008700800415d66e65d7160a8e2a1b344e2f09454ef19b569454fd5e36f14958df1e9b247002c44ea652d4092859c67da44e4ca3add6565b0e2897d640a2c51bfb370d8877fa00a9178d45190000000000000000402625a008011ac445debca569067cf73f05b9545361d0dd2c5bad6549bafb73bad27e85c7db00235888bbd794ad20cf9ee7e0b72a8a6c3a1ba58b75aca9375f6e775a4fd0b8fb4405"
    const cell = Cell.fromBoc(Buffer.from(bocHex, "hex"))[0]
    if (!cell) throw new Error("Expected an OutList root")

    expect(parseBlockTlb(cell, {strict: true})).toMatchObject({
      parser: "block.tlb",
      name: "OutList",
      value: {
        kind: "OutList_out_list",
        action: {kind: "OutAction_action_send_msg"},
      },
      consumption: {complete: true, remainingBits: 0, remainingRefs: 0},
    })
  })

  test("parses a standalone send-message OutAction BoC", () => {
    const bocHex =
      "b5ee9c720101050100fe00010a0ec3c86d5001036162002aef29b142b4239f0d70edb653da95568b394f6da9c2ef92ad64e546dba508e20000000000000000000000000002360203040842028f452d7a4dfd74066b682365177259ed05734435be76b5fd4bd5d8af2b7c3d68008700800415d66e65d7160a8e2a1b344e2f09454ef19b569454fd5e36f14958df1e9b247002c44ea652d4092859c67da44e4ca3add6565b0e2897d640a2c51bfb370d8877fa00a9178d45190000000000000000402625a008011ac445debca569067cf73f05b9545361d0dd2c5bad6549bafb73bad27e85c7db00235888bbd794ad20cf9ee7e0b72a8a6c3a1ba58b75aca9375f6e775a4fd0b8fb4405"
    const cell = Cell.fromBoc(Buffer.from(bocHex, "hex"))[0]
    if (!cell) throw new Error("Expected an OutAction root")

    expect(parseBlockTlb(cell, {strict: true})).toMatchObject({
      parser: "block.tlb",
      name: "OutAction",
      value: {kind: "OutAction_action_send_msg"},
      consumption: {complete: true, remainingBits: 0, remainingRefs: 0},
    })
  })

  test("parses a user TL-B schema", () => {
    const cell = beginCell().storeUint(42, 32).endCell()
    const result = tryParseCustomTlb(cell, "_ x:# = Foo;")

    expect(result).toMatchObject({
      matched: true,
      data: {kind: "Foo", x: 42},
      provenance: {engine: "custom-tlb", source: "user-schema"},
    })
  })

  test("serializes rich and cyclic values", () => {
    const value: {amount: bigint; bytes: Uint8Array; self?: unknown} = {
      amount: 42n,
      bytes: Uint8Array.from([1, 2, 3]),
    }
    value.self = value

    expect(toSerializable(value)).toEqual({
      amount: "42",
      bytes: "010203",
      self: "[Circular]",
    })
  })

  test("parses StateInit with the generated canonical block.tlb loader", () => {
    const cell = beginCell()
      .store(
        storeStateInit({
          code: beginCell().storeUint(0, 8).endCell(),
          data: beginCell().storeUint(1, 8).endCell(),
        }),
      )
      .endCell()

    expect(parseBlockTlb(cell, {strict: true})).toMatchObject({
      parser: "block.tlb",
      name: "StateInit",
      confidence: 0.55,
      consumption: {complete: true, remainingBits: 0, remainingRefs: 0},
    })
  })
})
