import {describe, expect, test} from "bun:test"
import {Address, beginCell, Dictionary} from "@ton/core"

import {parseWalletV5Storage} from "../src/components/walletV5"

const wallet = new Address(0, Buffer.alloc(32, 17))
const lowHash = Buffer.alloc(32)
lowHash[31] = 1
const highHash = Buffer.alloc(32, 255)

function storageHeader(isSignatureAllowed = true) {
  return beginCell()
    .storeBit(isSignatureAllowed)
    .storeUint(0xff_ff_ff_ff, 32)
    .storeUint(0x80_00_00_11, 32)
    .storeUint((1n << 256n) - 1n, 256)
}

function storageBoc(extensions: readonly [Buffer, boolean][] = [], isSignatureAllowed = true) {
  const dictionary = Dictionary.empty(Dictionary.Keys.Buffer(32), Dictionary.Values.Bool())
  for (const [key, value] of extensions) {
    dictionary.set(key, value)
  }
  return storageHeader(isSignatureAllowed)
    .storeDict(dictionary)
    .endCell()
    .toBoc()
    .toString("base64")
}

describe("Wallet V5 storage", () => {
  test("decodes an empty extension dictionary and unsigned wallet counters", () => {
    expect(parseWalletV5Storage(storageBoc(), wallet.toRawString())).toEqual({
      isSignatureAllowed: true,
      seqno: 0xff_ff_ff_ff,
      walletId: 0x80_00_00_11,
      extensions: [],
    })
  })

  test("reconstructs sorted addresses without losing leading zeroes or high hash bits", () => {
    expect(
      parseWalletV5Storage(
        storageBoc([
          [highHash, true],
          [lowHash, true],
        ]),
        wallet.toString({bounceable: false, testOnly: true}),
      ).extensions,
    ).toEqual([
      "0:0000000000000000000000000000000000000000000000000000000000000001",
      "0:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
    ])
  })

  test("uses the wallet's workchain and keeps every dictionary member, including false values", () => {
    const masterchainWallet = new Address(-1, wallet.hash)
    const parsed = parseWalletV5Storage(
      storageBoc([[lowHash, false]], false),
      masterchainWallet.toString(),
    )

    expect(parsed.isSignatureAllowed).toBe(false)
    expect(parsed.extensions).toEqual([
      "-1:0000000000000000000000000000000000000000000000000000000000000001",
    ])
  })

  test("rejects absent or malformed storage instead of reporting an empty plugin list", () => {
    const malformed = [
      "",
      "invalid-base64",
      beginCell().endCell().toBoc().toString("base64"),
      storageHeader().endCell().toBoc().toString("base64"),
      storageHeader().storeBit(true).endCell().toBoc().toString("base64"),
      storageHeader()
        .storeBit(true)
        .storeRef(beginCell().endCell())
        .endCell()
        .toBoc()
        .toString("base64"),
    ]

    for (const boc of malformed) {
      expect(() => parseWalletV5Storage(boc, wallet.toRawString())).toThrow(
        "Wallet V5 storage is invalid or incomplete.",
      )
    }
  })

  test("rejects unexpected storage tails and malformed extension values", () => {
    const invalidDictionary = Dictionary.empty(
      Dictionary.Keys.Buffer(32),
      Dictionary.Values.Uint(2),
    )
    invalidDictionary.set(lowHash, 3)
    const malformed = [
      storageHeader().storeBit(false).storeBit(true).endCell(),
      storageHeader().storeBit(false).storeRef(beginCell().endCell()).endCell(),
      storageHeader().storeDict(invalidDictionary).endCell(),
    ]

    for (const cell of malformed) {
      expect(() =>
        parseWalletV5Storage(cell.toBoc().toString("base64"), wallet.toRawString()),
      ).toThrow("Wallet V5 storage is invalid or incomplete.")
    }
  })

  test("rejects exotic dictionary references instead of silently hiding their extensions", () => {
    const exotic = beginCell().storeUint(2, 8).storeUint(0, 256).endCell({exotic: true})
    const boc = storageHeader().storeBit(true).storeRef(exotic).endCell().toBoc().toString("base64")

    expect(() => parseWalletV5Storage(boc, wallet.toRawString())).toThrow(
      "Wallet V5 storage is invalid or incomplete.",
    )
  })

  test("rejects an unknown wallet workchain instead of assuming the basechain", () => {
    expect(() => parseWalletV5Storage(storageBoc([[lowHash, true]]), "invalid-address")).toThrow()
  })
})
