import {lookupTargetToQuery, type LookupTarget} from "./target"

export interface VerificationSourceResponse {
  readonly code_hash: string
  readonly verified: boolean
  readonly bundle: SourceBundle | null
}

export interface SourceBundle {
  readonly source_bundle_hash: string
  readonly verified_at: number
  readonly storage_revision: string
  readonly entrypoint: string
  readonly compiler: CompilerMetadata
  readonly files: readonly SourceFile[]
}

export interface CompilerMetadata {
  readonly language: string
  readonly version: string
  readonly params: unknown
}

export interface SourceFile {
  readonly path: string
  readonly content_hash: string
  readonly include_in_command: boolean | null
  readonly is_stdlib: boolean | null
  readonly has_include_directives: boolean | null
  readonly content: string
}

export interface LastVerifiedResponse {
  readonly items: readonly LastVerifiedItem[]
  readonly total: number
}

export interface LastVerifiedItem {
  readonly code_hash: string
  readonly source_bundle_hash: string
  readonly verified_at: number
  readonly storage_revision: string
  readonly entrypoint: string
  readonly compiler: CompilerMetadata
  readonly file_count: number
  readonly has_tolk_abi: boolean
  readonly abi_name: string | null
}

export interface VerificationStatisticsResponse {
  readonly total: number
  readonly languages: readonly VerificationLanguageStatistics[]
}

export interface VerificationLanguageStatistics {
  readonly language: string
  readonly total: number
  readonly versions: readonly VerificationVersionStatistics[]
}

export interface VerificationVersionStatistics {
  readonly version: string
  readonly total: number
}

export interface VerificationStatisticsHistoryResponse {
  readonly items: readonly VerificationStatisticsHistoryItem[]
}

export interface VerificationStatisticsHistoryItem {
  readonly timestamp: number
  readonly compiler: string
  readonly version: string
}

export class ApiRequestError extends Error {
  readonly status: number

  constructor(status: number, message: string) {
    super(message)
    this.name = "ApiRequestError"
    this.status = status
  }
}

export interface VerifierApi {
  readonly fetchLastVerified: (limit?: number, offset?: number) => Promise<LastVerifiedResponse>
  readonly fetchStatistics: () => Promise<VerificationStatisticsResponse>
  readonly fetchStatisticsHistory: () => Promise<VerificationStatisticsHistoryResponse>
  readonly fetchVerificationSource: (target: LookupTarget) => Promise<VerificationSourceResponse>
}

export interface VerifierApiOptions {
  readonly baseUrl?: string
  readonly fetch?: typeof globalThis.fetch
}

export function createVerifierApi({
  baseUrl = "/api/v1",
  fetch: fetchImplementation = globalThis.fetch,
}: VerifierApiOptions = {}): VerifierApi {
  const normalizedBaseUrl = baseUrl.replace(/\/$/, "")

  const request = async <T>(path: string): Promise<T> => {
    const response = await fetchImplementation(`${normalizedBaseUrl}${path}`, {
      headers: {
        accept: "application/json",
      },
    })

    const body = (await response.json().catch(() => undefined)) as
      | ({error?: string} & T)
      | undefined
    if (!response.ok) {
      throw new ApiRequestError(
        response.status,
        body?.error || `Request failed: ${response.status}`,
      )
    }

    return body as T
  }

  return {
    fetchLastVerified(limit = 12, offset = 0) {
      const params = new URLSearchParams({
        limit: String(limit),
        offset: String(offset),
      })
      return request<LastVerifiedResponse>(`/last_verified?${params.toString()}`)
    },
    fetchStatistics() {
      return request<VerificationStatisticsResponse>("/statistics")
    },
    fetchStatisticsHistory() {
      return request<VerificationStatisticsHistoryResponse>("/statistics/history")
    },
    fetchVerificationSource(target) {
      return request<VerificationSourceResponse>(
        `/verification/source?${lookupTargetToQuery(target)}`,
      )
    },
  }
}
