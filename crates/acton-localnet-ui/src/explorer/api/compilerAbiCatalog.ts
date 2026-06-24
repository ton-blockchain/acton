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

export interface BundledCompilerAbiCatalogEntry extends ExtendedContractABI {
  readonly slug: string
}

let catalogBundlePromise: Promise<CatalogBundle> | undefined
let catalogByCodeHashPromise: Promise<ReadonlyMap<string, ExtendedContractABI>> | undefined
let catalogEntriesPromise: Promise<readonly BundledCompilerAbiCatalogEntry[]> | undefined

export async function getBundledCompilerAbis(
  codeHashes: readonly string[],
): Promise<Record<string, ExtendedContractABI | null>> {
  try {
    const catalog = await loadCatalogByCodeHash()
    return Object.fromEntries(
      codeHashes.map(codeHash => [codeHash, catalog.get(normalizeCodeHash(codeHash)) ?? null]),
    )
  } catch (error) {
    console.error("Failed to load bundled ABI catalog", error)
    return Object.fromEntries(codeHashes.map(codeHash => [codeHash, null]))
  }
}

export async function getBundledCompilerAbiCatalog(): Promise<
  readonly BundledCompilerAbiCatalogEntry[]
> {
  try {
    return await loadCatalogEntries()
  } catch (error) {
    console.error("Failed to load bundled ABI catalog", error)
    return []
  }
}

async function loadCatalogBundle(): Promise<CatalogBundle> {
  catalogBundlePromise ??= fetch(dataAbisUrl).then(response => {
    if (!response.ok) {
      throw new Error(`Failed to fetch bundled ABI catalog: ${response.status}`)
    }
    return response.json() as Promise<CatalogBundle>
  })

  return catalogBundlePromise
}

async function loadCatalogByCodeHash(): Promise<ReadonlyMap<string, ExtendedContractABI>> {
  catalogByCodeHashPromise ??= loadCatalogBundle().then(buildCatalogByCodeHash)
  return catalogByCodeHashPromise
}

async function loadCatalogEntries(): Promise<readonly BundledCompilerAbiCatalogEntry[]> {
  catalogEntriesPromise ??= loadCatalogBundle().then(buildCatalogEntries)
  return catalogEntriesPromise
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

function buildCatalogEntries(bundle: CatalogBundle): readonly BundledCompilerAbiCatalogEntry[] {
  const baseSlugs = bundle.contracts.map((contract, index) =>
    slugifyCatalogName(catalogDisplayName(contract, index)),
  )
  const duplicatedSlugs = new Set(
    baseSlugs.filter((slug, index) => baseSlugs.indexOf(slug) !== index),
  )

  return bundle.contracts.map((contract, index) => {
    const codeHashes = contract.hashes.map(normalizeCodeHash).filter(Boolean)
    const baseSlug = baseSlugs[index]
    const slug = duplicatedSlugs.has(baseSlug)
      ? `${baseSlug}-${codeHashes[0]?.slice(0, 8) ?? index + 1}`
      : baseSlug

    return {
      slug,
      compiler_abi: contract.compilerAbi,
      display_name: contract.displayName,
      code_hashes: codeHashes,
      links: contract.links ?? [],
    }
  })
}

function catalogDisplayName(contract: CatalogContract, index: number): string {
  return contract.displayName || contract.compilerAbi.contract_name || `ABI ${index + 1}`
}

function slugifyCatalogName(name: string): string {
  const slug = name
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")

  return slug || "abi"
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
