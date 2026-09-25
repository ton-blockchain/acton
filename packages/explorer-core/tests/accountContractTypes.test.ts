import {describe, expect, test} from "bun:test"

import {
  getAccountContractTypeByCodeHash,
  hasAccountContractHint,
  hasAccountInterface,
  hasTokenInfoType,
} from "../src/pages/accountContractTypes"
import bundledAbiCatalog from "../../../crates/acton-abi-catalog/data/data-abis.json"

describe("Wallet V5 code hash detection", () => {
  test("recognizes every bundled Wallet V5r1 implementation without indexed interfaces", () => {
    const wallet = bundledAbiCatalog.contracts.find(
      contract => contract.id === "wallets.WalletV5r1",
    )
    if (!wallet) throw new Error("Wallet V5r1 is missing from the ABI catalog")
    expect(wallet.hashes.length).toBeGreaterThan(0)
    for (const hash of wallet.hashes) {
      expect(getAccountContractTypeByCodeHash(hash)).toBe("wallet_v5r1")
    }
  })

  test("does not infer Wallet V5 from missing or unknown code", () => {
    expect(getAccountContractTypeByCodeHash(undefined)).toBeUndefined()
    expect(getAccountContractTypeByCodeHash("0".repeat(64))).toBeUndefined()
  })
})

describe("account interface detection", () => {
  test("normalizes whitespace and letter case", () => {
    expect(hasAccountInterface([" NFT_ITEM_SIMPLE "], "nft_item_simple")).toBe(true)
  })
})

describe("token info type detection", () => {
  const tokenInfo = [{type: "jetton_masters"}, {type: "nft_items"}]

  test("matches an indexed token type", () => {
    expect(hasTokenInfoType(tokenInfo, "jetton_masters")).toBe(true)
  })

  test("rejects missing and differently cased token types", () => {
    expect(hasTokenInfoType(tokenInfo, "jetton_wallets")).toBe(false)
    expect(hasTokenInfoType(tokenInfo, "NFT_ITEMS")).toBe(false)
  })
})

describe("account contract type detection", () => {
  test("recognizes indexed jetton masters when account interfaces are empty", () => {
    expect(
      hasAccountContractHint([], [{type: "jetton_masters", symbol: "JETTON"}], "jetton_master"),
    ).toBe(true)
  })

  test("recognizes simple NFT item interfaces", () => {
    expect(hasAccountContractHint(["nft_item_simple"], [], "nft_item")).toBe(true)
  })

  test("does not accept unknown NFT interface suffixes", () => {
    expect(hasAccountContractHint(["nft_item_unknown"], [], "nft_item")).toBe(false)
  })

  test("recognizes NFT items and collections from indexed metadata", () => {
    expect(hasAccountContractHint([], [{type: "nft_items"}], "nft_item")).toBe(true)
    expect(hasAccountContractHint([], [{type: "nft_collections"}], "nft_collection")).toBe(true)
  })

  test("does not infer an unrelated contract type", () => {
    expect(hasAccountContractHint(["wallet_v5r1"], [], "nft_item")).toBe(false)
  })
})
