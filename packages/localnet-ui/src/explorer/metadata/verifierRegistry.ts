import type {ExtendedContractABI} from "../api/compilerAbi"
import type {VerificationSourceResponse} from "../api/types"
import {normalizeCodeHash} from "./codeHash"
import {NullMetadataRegistry, unverifiedSourceResponse} from "./nullRegistry"

interface VerifierAbiResponse {
  readonly items?: readonly VerifierAbiItem[]
}

interface VerifierAbiItem {
  readonly code_hash?: string
  readonly abi?: unknown
}

const VERIFIER_SOURCE_URL = "https://verifier.acton.monster/api/v1/verification/source"
const VERIFIER_ABI_URL = "https://verifier.acton.monster/api/v1/abi"

export class VerifierMetadataRegistry extends NullMetadataRegistry {
  private readonly compilerAbiCache = new Map<string, ExtendedContractABI | null>()
  private readonly sourceCache = new Map<string, VerificationSourceResponse>()

  override async getCompilerAbis(
    codeHashes: readonly string[],
  ): Promise<Record<string, ExtendedContractABI | null>> {
    const result: Record<string, ExtendedContractABI | null> = {}
    await Promise.all(
      codeHashes.map(async codeHash => {
        const normalized = normalizeCodeHash(codeHash)
        if (!normalized) {
          result[codeHash] = null
          return
        }
        if (this.compilerAbiCache.has(normalized)) {
          result[codeHash] = this.compilerAbiCache.get(normalized) ?? null
          return
        }

        const abi = await this.fetchCompilerAbi(normalized).catch(error => {
          console.debug(`Failed to fetch verifier ABI for ${normalized}`, error)
          return null
        })
        this.compilerAbiCache.set(normalized, abi)
        result[codeHash] = abi
      }),
    )
    return result
  }

  override async getSource(options: {
    readonly address?: string
    readonly codeHash?: string
  }): Promise<VerificationSourceResponse> {
    const key = sourceCacheKey(options)
    const cached = this.sourceCache.get(key)
    if (cached) {
      return cached
    }

    const source = await this.fetchSource(options).catch(error => {
      console.debug("Verifier source lookup failed", error)
      return unverifiedSourceResponse(options)
    })
    this.sourceCache.set(key, source)
    if (source.code_hash) {
      this.sourceCache.set(sourceCacheKey({codeHash: source.code_hash}), source)
    }
    return source
  }

  private async fetchCompilerAbi(codeHash: string): Promise<ExtendedContractABI | null> {
    const url = new URL(VERIFIER_ABI_URL)
    url.searchParams.set("code_hash", codeHash)
    const response = await fetch(url)
    if (!response.ok) {
      throw new Error(`Verifier ABI request failed with HTTP ${response.status}`)
    }
    const payload = (await response.json()) as VerifierAbiResponse
    const item = payload.items?.find(entry => normalizeCodeHash(entry.code_hash) === codeHash)
    const abi = item?.abi && typeof item.abi === "object" ? item.abi : undefined
    return abi
      ? {
          compiler_abi: abi as ExtendedContractABI["compiler_abi"],
          code_hashes: [codeHash],
          links: [],
        }
      : null
  }

  private async fetchSource(options: {
    readonly address?: string
    readonly codeHash?: string
  }): Promise<VerificationSourceResponse> {
    const url = new URL(VERIFIER_SOURCE_URL)
    if (options.address) {
      url.searchParams.append("address", options.address)
    }
    const codeHash = normalizeCodeHash(options.codeHash)
    if (codeHash) {
      url.searchParams.append("code_hash", codeHash)
    }
    const response = await fetch(url)
    if (!response.ok) {
      throw new Error(`Verifier source request failed with HTTP ${response.status}`)
    }
    return response.json() as Promise<VerificationSourceResponse>
  }
}

function sourceCacheKey(options: {readonly address?: string; readonly codeHash?: string}): string {
  const codeHash = normalizeCodeHash(options.codeHash)
  if (codeHash) {
    return `code_hash:${codeHash}`
  }
  return `address:${options.address?.trim() ?? ""}`
}
