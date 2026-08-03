import {describe, expect, test} from "bun:test"
import {beginCell} from "@ton/core"
import {Buffer} from "buffer"

import {codeLookupHashHex} from "../src/lib/codeCell"

describe("codeLookupHashHex", () => {
  test("uses the representation hash for ordinary code", () => {
    const code = beginCell().storeUint(0x12_34, 16).endCell()

    expect(codeLookupHashHex(code)).toBe(code.hash().toString("hex"))
  })

  test("uses the referenced library hash for library-reference code", () => {
    const libraryHash = Buffer.alloc(32, 0xab)
    const code = beginCell().storeUint(2, 8).storeBuffer(libraryHash).endCell({exotic: true})

    expect(codeLookupHashHex(code)).toBe(libraryHash.toString("hex"))
    expect(codeLookupHashHex(code)).not.toBe(code.hash().toString("hex"))
  })
})
