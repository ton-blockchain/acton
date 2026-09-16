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

const VERIFIER_URL = "https://verifier.ton.org"
const VERIFIER_SOURCE_URL = `${VERIFIER_URL}/api/v1/verification/source`
const VERIFIER_ABI_URL = `${VERIFIER_URL}/api/v1/abi`
const DEFAULT_REQUEST_TIMEOUT_MS = 5000
const MISSING_ABI_CACHE_TTL_MS = 60_000

export function verifierVerificationUrl(codeHash: string): string {
  return `${VERIFIER_URL}/${encodeURIComponent(codeHash)}`
}

export interface VerifierMetadataRegistryOptions {
  readonly requestTimeoutMs?: number
}

/**
 * Resolves verifier metadata by code hash and shares concurrent ABI requests.
 * Verified ABIs are immutable; missing ABIs expire after a minute so newly
 * published verification becomes visible without polling the same 404 each refresh.
 * Transport failures are not cached.
 */
export class VerifierMetadataRegistry extends NullMetadataRegistry {
  private readonly compilerAbiCache = new Map<
    string,
    {readonly abi: ExtendedContractABI | null; readonly expiresAt: number}
  >()
  private readonly compilerAbiRequests = new Map<string, Promise<ExtendedContractABI | null>>()
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
        const cached = this.compilerAbiCache.get(normalized)
        if (cached && cached.expiresAt > Date.now()) {
          result[codeHash] = cached.abi
          return
        }

        try {
          let request = this.compilerAbiRequests.get(normalized)
          if (!request) {
            request = this.fetchCompilerAbi(normalized)
              .then(abi => {
                this.compilerAbiCache.set(normalized, {
                  abi,
                  expiresAt: abi ? Number.POSITIVE_INFINITY : Date.now() + MISSING_ABI_CACHE_TTL_MS,
                })
                return abi
              })
              .finally(() => this.compilerAbiRequests.delete(normalized))
            this.compilerAbiRequests.set(normalized, request)
          }
          result[codeHash] = await request
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
    const codeHash = normalizeCodeHash(options.codeHash)
    // Only positive code-hash lookups are immutable. An address can upgrade its
    // code, and an unverified hash can acquire a source bundle at any moment.
    const cached = !options.address && codeHash ? this.sourceCache.get(codeHash) : undefined
    if (cached) {
      return cached
    }

    try {
      const source = await this.fetchSource(options)
      const resolvedHash = normalizeCodeHash(source.code_hash)
      if (source.verified && source.bundle && resolvedHash) {
        this.sourceCache.set(resolvedHash, source)
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
    if (response.status === 404) {
      return null
    }
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
    if (response.status === 404) {
      return unverifiedSourceResponse(options)
    }
    if (!response.ok) {
      throw new Error(`Verifier source request failed with HTTP ${response.status}`)
    }
    const source = (await response.json()) as VerificationSourceResponse
    if (codeHash && normalizeCodeHash(source.code_hash) !== codeHash) {
      throw new Error("Verifier returned sources for a different code hash")
    }
    return source
  }
}
