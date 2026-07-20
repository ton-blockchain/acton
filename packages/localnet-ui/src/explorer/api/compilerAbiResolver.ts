import type {ContractABI} from "@ton/tolk-abi-to-typescript"

import type {ExtendedContractABI} from "./compilerAbi"
import {addressKey} from "./compilerAbi"
import type {ExplorerMetadataRegistry} from "../metadata/types"
import type {TonClient} from "./client"

export interface ResolveCompilerAbisOptions {
  readonly client: TonClient
  readonly metadataRegistry: ExplorerMetadataRegistry
  readonly addresses: readonly string[]
  readonly additionalCodeHashes?: readonly string[]
  readonly shouldContinue?: () => boolean
  readonly onAccountStatesError?: (error: unknown) => void
}

export interface ResolvedCompilerAbis {
  readonly requestedAddresses: readonly {readonly key: string; readonly address: string}[]
  readonly addressToCodeHash: ReadonlyMap<string, string>
  readonly abiByCodeHash: ReadonlyMap<string, ContractABI | undefined>
  readonly abiByAddress: ReadonlyMap<string, ContractABI | undefined>
}

export async function resolveCompilerAbis({
  client,
  metadataRegistry,
  addresses,
  additionalCodeHashes = [],
  shouldContinue = () => true,
  onAccountStatesError,
}: ResolveCompilerAbisOptions): Promise<ResolvedCompilerAbis | undefined> {
  const requestedAddresses = normalizeRequestedAddresses(addresses)

  const states =
    requestedAddresses.length > 0
      ? await client
          .getAccountStates(
            requestedAddresses.map(({address}) => address),
            false,
          )
          .catch(error => {
            onAccountStatesError?.(error)
            return undefined
          })
      : undefined

  if (!shouldContinue()) {
    return undefined
  }

  const addressToCodeHash = new Map<string, string>()
  for (const account of states?.accounts ?? []) {
    if (account.code_hash) {
      addressToCodeHash.set(addressKey(account.address), account.code_hash)
    }
  }

  const codeHashes = [
    ...new Set([...addressToCodeHash.values(), ...additionalCodeHashes].filter(Boolean)),
  ]
  const fetchedAbis =
    codeHashes.length > 0
      ? await metadataRegistry
          .getCompilerAbis(codeHashes)
          .catch((): Record<string, ExtendedContractABI | null> => ({}))
      : {}

  if (!shouldContinue()) {
    return undefined
  }

  const abiByCodeHash = new Map<string, ContractABI | undefined>()
  for (const codeHash of codeHashes) {
    abiByCodeHash.set(codeHash, fetchedAbis[codeHash]?.compiler_abi)
  }

  const abiByAddress = new Map<string, ContractABI | undefined>()
  for (const {key} of requestedAddresses) {
    const codeHash = addressToCodeHash.get(key)
    abiByAddress.set(key, codeHash ? abiByCodeHash.get(codeHash) : undefined)
  }

  return {
    requestedAddresses,
    addressToCodeHash,
    abiByCodeHash,
    abiByAddress,
  }
}

function normalizeRequestedAddresses(
  addresses: readonly string[],
): readonly {readonly key: string; readonly address: string}[] {
  const byKey = new Map<string, string>()
  for (const address of addresses) {
    if (!address) {
      continue
    }
    const key = addressKey(address)
    if (!byKey.has(key)) {
      byKey.set(key, address)
    }
  }
  return [...byKey].map(([key, address]) => ({key, address}))
}
