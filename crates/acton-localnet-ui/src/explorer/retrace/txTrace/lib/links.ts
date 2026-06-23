import type {BaseTxInfo} from "txtracer-core"
import {Address} from "@ton/core"

// eslint-disable-next-line functional/type-declaration-immutability
export type ExtractionResult = BaseInfo | SingleHash | UnknownNetwork

export type BaseInfo = {
  readonly $: "BaseInfo"
  readonly info: BaseTxInfo
  readonly testnet: boolean
}
export const BaseInfo = (info: BaseTxInfo, testnet: boolean): BaseInfo => ({
  $: "BaseInfo",
  info,
  testnet,
})

export type SingleHash = {
  readonly $: "SingleHash"
  readonly hash: string
  readonly testnet: boolean
}
export const SingleHash = (hash: string, testnet: boolean): SingleHash => ({
  $: "SingleHash",
  hash: hash.toLowerCase(),
  testnet,
})

// eslint-disable-next-line functional/type-declaration-immutability
export type UnknownNetwork = {
  readonly $: "UnknownNetwork"
  readonly hash: string
  testnet: boolean
}
export const UnknownNetwork = (hash: string): UnknownNetwork => ({
  $: "UnknownNetwork",
  hash: hash.toLowerCase(),
  testnet: false,
})

/**
 * Extracts transaction data from a URL pointing to one of the supported TON block explorers.
 *
 * Supported URL formats:
 * - `https://ton.cx/tx/...` and `https://testnet.ton.cx/tx/...`
 * - `https://tonviewer.com/transaction/...` and `https://testnet.tonviewer.com/transaction/...`
 * - `https://tonscan.org/tx/...` and `https://testnet.tonscan.org/tx/...`
 * - `https://explorer.toncoin.org/transaction?...` and `https://test-explorer.toncoin.org/transaction?...`
 * - `https://dton.io/tx/...` and `https://testnet.dton.io/tx/...`
 *
 * Based on the domain, determines whether the link refers to mainnet or testnet,
 * parses the relevant parameters (`lt`, `hash`, `address`), and returns:
 * - `BaseInfo` — when `lt`, `hash`, and `address` are available;
 * - `SingleHash` — when only `hash` is available.
 *
 * For unsupported URLs, returns `undefined`.
 *
 * @param txLink – A transaction URL.
 * @returns An `ExtractionResult`, or `undefined` if the URL format is unrecognized.
 * @throws Error if a link contains malformed data.
 */
export function extractTxInfoFromLink(txLink: string): ExtractionResult | undefined {
  if (txLink.startsWith("https://ton.cx/tx/") || txLink.startsWith("https://testnet.ton.cx/tx/")) {
    // https://ton.cx/tx/56166043000001:T6Y6ZoW71mrznFA0RyU/xV5ILpz9WUPJ9i9/4xPq1Is=:EQCqKZrrce8Ss6SZaLI-OkH2w8-xtPP9_ZvyyIZLhy9Hmpf8
    const testnet = txLink.includes("testnet.")
    const data = testnet ? txLink.slice(26) : txLink.slice(18)
    const [lt, hash, address] = data.split(":")
    return BaseInfo(fromTriple(lt, hash, address), testnet)
  }

  if (
    txLink.startsWith("https://tonviewer.com/") ||
    txLink.startsWith("https://testnet.tonviewer.com/")
  ) {
    // https://tonviewer.com/transaction/7a236ab8bdec69ae46c02a5142dfe0dc45bf03b30607c5f88fdf86daeb8e393b
    const testnet = txLink.includes("testnet.")
    const hash = testnet ? txLink.slice(42) : txLink.slice(34)
    return SingleHash(hash, testnet)
  }

  if (
    txLink.startsWith("https://tonscan.org/tx/") ||
    txLink.startsWith("https://testnet.tonscan.org/tx/")
  ) {
    // https://tonscan.org/tx/7a236ab8bdec69ae46c02a5142dfe0dc45bf03b30607c5f88fdf86daeb8e393b
    const testnet = txLink.includes("testnet.")
    const hashMaybeBase64 = testnet ? txLink.slice(31) : txLink.slice(23)
    if (hashMaybeBase64.endsWith("=")) {
      const hash = Buffer.from(hashMaybeBase64, "base64")
      return SingleHash(hash.toString("hex"), testnet)
    }
    return SingleHash(hashMaybeBase64, testnet)
  }

  if (
    txLink.startsWith("https://explorer.toncoin.org/transaction") ||
    txLink.startsWith("https://test-explorer.toncoin.org/transaction")
  ) {
    // https://explorer.toncoin.org/transaction?account=EQDa4VOnTYlLvDJ0gZjNYm5PXfSmmtL6Vs6A_CZEtXCNICq_&lt=47670702000009&hash=3e5f49798de239da5d8f80b4dc300204d37613e4203a3f7b877c04a88c81856b
    const testnet = txLink.includes("test-")
    const url = new URL(txLink)
    const lt = url.searchParams.get("lt") ?? undefined
    const hash = url.searchParams.get("hash") ?? undefined
    const address = url.searchParams.get("account") ?? undefined
    return BaseInfo(fromTriple(lt, hash, address), testnet)
  }

  if (txLink.startsWith("https://dton.io/tx") || txLink.startsWith("https://testnet.dton.io/tx")) {
    // https://dton.io/tx/F64C6A3CDF3FAD1D786AACF9A6130F18F3F76EEB71294F53BBD812AD3703E70A
    const testnet = txLink.includes("testnet.")
    const hash = testnet ? txLink.slice(27) : txLink.slice(19)
    return SingleHash(hash, testnet)
  }

  if (txLink.startsWith("https://retracer.ton.org/?tx=")) {
    const hash = txLink.slice(29)
    return UnknownNetwork(hash)
  }

  return undefined
}

function fromTriple(lt: string | undefined, hash: string | undefined, address: string | undefined) {
  if (lt === undefined || hash === undefined || address === undefined) {
    throw new Error("Invalid ton.cx link")
  }
  const bufferHash = hash.endsWith("=") ? Buffer.from(hash, "base64") : Buffer.from(hash, "hex")
  return {
    lt: BigInt(lt),
    hash: bufferHash,
    address: Address.parse(address),
  }
}
