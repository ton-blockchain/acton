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

const VERIFIER_SOURCE_URL = "https://verifier-staging.actonscan.com/api/v1/verification/source"
const VERIFIER_ABI_URL = "https://verifier-staging.actonscan.com/api/v1/abi"
const DEFAULT_REQUEST_TIMEOUT_MS = 5000

export interface VerifierMetadataRegistryOptions {
  readonly requestTimeoutMs?: number
}

export class VerifierMetadataRegistry extends NullMetadataRegistry {
  private readonly compilerAbiCache = new Map<string, ExtendedContractABI | null>()
  private readonly sourceCache = new Map<string, VerificationSourceResponse>()
  private readonly requestTimeoutMs: number

  constructor(options: VerifierMetadataRegistryOptions = {}) {
    super()
    this.requestTimeoutMs = options.requestTimeoutMs ?? DEFAULT_REQUEST_TIMEOUT_MS
  }

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

        try {
          const abi = await this.fetchCompilerAbi(normalized)
          this.compilerAbiCache.set(normalized, abi)
          result[codeHash] = abi
        } catch (error) {
          console.debug(`Failed to fetch verifier ABI for ${normalized}`, error)
          result[codeHash] = null
        }
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

    try {
      const source = await this.fetchSource(options)
      this.sourceCache.set(key, source)
      if (source.code_hash) {
        this.sourceCache.set(sourceCacheKey({codeHash: source.code_hash}), source)
      }
      return source
    } catch (error) {
      console.debug("Verifier source lookup failed", error)
      return unverifiedSourceResponse(options)
    }
  }

  private async fetchCompilerAbi(codeHash: string): Promise<ExtendedContractABI | null> {
    const url = new URL(VERIFIER_ABI_URL)
    url.searchParams.set("code_hash", codeHash)
    const response = await fetch(url, {signal: AbortSignal.timeout(this.requestTimeoutMs)})
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
    const response = await fetch(url, {signal: AbortSignal.timeout(this.requestTimeoutMs)})
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
