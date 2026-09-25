import {Cell} from "@ton/core"
import {codeLookupHashHex} from "@acton/transaction-ui"

import type {TonClient} from "../api/client"
import {addressKey, type ExtendedContractABI} from "../api/compilerAbi"
import type {V3AccountState} from "../api/types"
import {normalizeCodeHash} from "../metadata/codeHash"
import type {ExplorerMetadataRegistry} from "../metadata/types"
import {getContractTypeLabels} from "./contractTypeLabels"

export type WalletV5PluginType =
  | {readonly status: "success"; readonly labels: readonly string[]}
  | {readonly status: "error"}

interface LoadWalletV5PluginTypesOptions {
  readonly client: Pick<TonClient, "getAccountStates">
  readonly metadataRegistry: Pick<ExplorerMetadataRegistry, "getCompilerAbis">
  readonly addresses: readonly string[]
  readonly shouldContinue?: () => boolean
}

const ACCOUNT_BATCH_SIZE = 100

/** Resolves exact plugin implementations, with the same interface fallback as account cards. */
export async function loadWalletV5PluginTypes({
  client,
  metadataRegistry,
  addresses,
  shouldContinue = () => true,
}: LoadWalletV5PluginTypesOptions): Promise<ReadonlyMap<string, WalletV5PluginType> | undefined> {
  if (!shouldContinue()) return undefined
  const keys = [...new Set(addresses.map(addressKey))]
  const results = new Map<string, WalletV5PluginType>(keys.map(key => [key, {status: "error"}]))
  const batches: string[][] = []
  for (let offset = 0; offset < keys.length; offset += ACCOUNT_BATCH_SIZE) {
    batches.push(keys.slice(offset, offset + ACCOUNT_BATCH_SIZE))
  }
  const responses = await Promise.all(
    batches.map(batch => client.getAccountStates(batch, true).catch(() => undefined)),
  )
  if (!shouldContinue()) return undefined

  const accounts = new Map<string, V3AccountState>()
  for (const response of responses) {
    for (const account of response?.accounts ?? []) {
      const key = addressKey(account.address)
      if (results.has(key)) accounts.set(key, account)
    }
  }
  const codeHashes = [
    ...new Set(
      [...accounts.values()]
        .map(account => pluginCodeHash(account))
        .filter((hash): hash is string => hash !== undefined),
    ),
  ]
  const abis = new Map<string, ExtendedContractABI | null>()
  // A failed lookup must not hide another plugin's independently resolved ABI.
  const metadataResults = await Promise.allSettled(
    codeHashes.map(codeHash => metadataRegistry.getCompilerAbis([codeHash], {throwOnError: true})),
  )
  for (const [index, result] of metadataResults.entries()) {
    if (result.status === "fulfilled") {
      const codeHash = codeHashes[index]
      abis.set(codeHash, result.value[codeHash] ?? null)
    }
  }
  if (!shouldContinue()) return undefined

  for (const [key, account] of accounts) {
    const codeHash = pluginCodeHash(account)
    const exactAbi = codeHash ? abis.get(codeHash)?.compiler_abi : undefined
    const labels = getContractTypeLabels(exactAbi, account.interfaces ?? [])
    if (codeHash && !abis.has(codeHash)) {
      continue
    }
    results.set(key, {status: "success", labels})
  }
  return results
}

function pluginCodeHash(account: V3AccountState): string | undefined {
  if (account.code_boc) {
    try {
      return codeLookupHashHex(Cell.fromBase64(account.code_boc))
    } catch {
      // Retain the indexer's hash when the code BoC is unavailable or malformed.
    }
  }
  return normalizeCodeHash(account.code_hash)
}
