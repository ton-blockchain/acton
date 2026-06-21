import type {ContractABI} from "@ton/tolk-abi-to-typescript"

import dataAbisUrl from "../../../../acton-abi-catalog/data/data-abis.json?url"

import type {ContractAbiLink, ExtendedContractABI} from "./compilerAbi"

interface CatalogBundle {
  readonly contracts: readonly CatalogContract[]
}

interface CatalogContract {
  readonly displayName: string
  readonly hashes: readonly string[]
  readonly compilerAbi: ContractABI
  readonly links?: readonly ContractAbiLink[]
}

let catalogPromise: Promise<ReadonlyMap<string, ExtendedContractABI>> | undefined

export async function getBundledCompilerAbis(
  codeHashes: readonly string[],
): Promise<Record<string, ExtendedContractABI | null>> {
  try {
    const catalog = await loadCatalog()
    return Object.fromEntries(
      codeHashes.map(codeHash => [codeHash, catalog.get(normalizeCodeHash(codeHash)) ?? null]),
    )
  } catch (error) {
    console.error("Failed to load bundled ABI catalog", error)
    return Object.fromEntries(codeHashes.map(codeHash => [codeHash, null]))
  }
}

async function loadCatalog(): Promise<ReadonlyMap<string, ExtendedContractABI>> {
  catalogPromise ??= fetch(dataAbisUrl)
    .then(response => {
      if (!response.ok) {
        throw new Error(`Failed to fetch bundled ABI catalog: ${response.status}`)
      }
      return response.json() as Promise<CatalogBundle>
    })
    .then(buildCatalogByCodeHash)

  return catalogPromise
}

function buildCatalogByCodeHash(bundle: CatalogBundle): ReadonlyMap<string, ExtendedContractABI> {
  const byCodeHash = new Map<string, ExtendedContractABI>()

  for (const contract of bundle.contracts) {
    const codeHashes = contract.hashes.map(normalizeCodeHash).filter(Boolean)
    const extendedAbi: ExtendedContractABI = {
      compiler_abi: contract.compilerAbi,
      display_name: contract.displayName,
      code_hashes: codeHashes,
      links: contract.links ?? [],
    }

    for (const codeHash of codeHashes) {
      if (!byCodeHash.has(codeHash)) {
        byCodeHash.set(codeHash, extendedAbi)
      }
    }
  }

  return byCodeHash
}

function normalizeCodeHash(codeHash: string): string {
  const trimmed = codeHash.trim()
  const hex = trimmed.replace(/^0x/i, "").toLowerCase()
  if (/^[0-9a-f]{64}$/.test(hex)) {
    return hex
  }

  return base64ToHex(trimmed) ?? hex
}

function base64ToHex(value: string): string | undefined {
  try {
    const base64 = value.replace(/-/g, "+").replace(/_/g, "/")
    const padded = base64.padEnd(Math.ceil(base64.length / 4) * 4, "=")
    const binary = atob(padded)
    if (binary.length !== 32) {
      return undefined
    }

    return Array.from(binary, char => char.charCodeAt(0).toString(16).padStart(2, "0")).join("")
  } catch {
    return undefined
  }
}
