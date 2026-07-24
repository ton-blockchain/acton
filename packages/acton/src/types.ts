import type {Buffer} from "node:buffer"

export interface LocalnetOptions {
  readonly endpoint?: string
  readonly authToken?: string
}

export interface StartLocalnetOptions {
  readonly command?: string
  readonly projectRoot?: string
  readonly port?: number
  readonly forkNet?: string
  readonly forkBlockNumber?: number
  readonly accounts?: readonly string[]
  readonly dbPath?: string
  readonly rateLimit?: number
  readonly responseDelayMs?: number
  readonly blockIntervalMs?: number
  readonly noMining?: boolean
  readonly requireAuth?: boolean
  readonly authToken?: string
  readonly loadState?: string
  readonly dumpState?: string
  readonly env?: Readonly<Record<string, string | undefined>>
  readonly startupTimeoutMs?: number
  readonly pollIntervalMs?: number
  readonly stdio?: "ignore" | "inherit"
  readonly autoClose?: boolean
}

export interface WaitUntilReadyOptions {
  readonly timeoutMs?: number
  readonly pollIntervalMs?: number
}

export interface CloseLocalnetOptions {
  readonly timeoutMs?: number
  readonly signal?: NodeJS.Signals
}

export interface LocalnetNodeInfo {
  readonly uptime_seconds: number
  readonly last_block_seqno: number
  readonly current_unix_time: number
  readonly time_offset_seconds: number
  readonly next_block_timestamp: number | null
  readonly state_source: string
  readonly fork_network: string | null
  readonly fork_block_number: number | null
  readonly network_conditions: LocalnetNetworkConditions
}

export interface LocalnetNetworkConditions {
  readonly response_delay_ms: number
}

export interface LocalnetNetworkConditionsOptions {
  readonly responseDelayMs: number
}

export type LocalnetAccountStateChange =
  | {readonly type: "nonexist"}
  | {readonly type: "uninit"; readonly balance?: bigint | number | string}
  | {readonly type: "frozen"; readonly source: "current"}
  | {
      readonly type: "frozen"
      readonly frozenHash: Buffer | string
      readonly balance?: bigint | number | string
    }

export interface SendBocResult {
  readonly "@type": "ok"
  readonly hash?: string
  readonly hash_norm?: string
}

export interface LocalnetBlockId {
  readonly workchain: number
  readonly shard: number
  readonly seqno: number
  readonly root_hash: string
  readonly file_hash: string
}

export interface LocalnetMineResult {
  readonly blocks_mined: number
  readonly last_block_seqno: number
  readonly blocks: readonly LocalnetBlockId[]
}

export interface LocalnetRecoveryPointResult {
  readonly id: number
  readonly block_seqno: number
}

export interface LocalnetClockInfo {
  readonly current_unix_time: number
  readonly time_offset_seconds: number
  readonly next_block_timestamp: number | null
}

export type LocalnetApiCallStatus = "success" | "failed"
export type LocalnetApiCallType = "read" | "write"
export type LocalnetApiCallFamily = "control" | "emulate" | "json_rpc" | "streaming" | "v2" | "v3"

export interface LocalnetApiCallRecord {
  readonly sequence: number
  readonly status: LocalnetApiCallStatus
  readonly status_code: number
  readonly call_type: LocalnetApiCallType
  readonly api_family: LocalnetApiCallFamily
  readonly http_method: string
  readonly path: string
  readonly method: string
  readonly request_id: unknown
  readonly timestamp_ms: number
  readonly duration_ns: number
}

export interface LocalnetApiCallLog {
  readonly calls: readonly LocalnetApiCallRecord[]
  readonly total_retained: number
  readonly max_retained: number
}

export interface LocalnetStartupWallet {
  readonly name: string
  readonly mnemonic: readonly string[]
  readonly version: string
  readonly network: string
  readonly address: string
  readonly public_key: string
  readonly wallet_id: number
}

export interface LocalnetContractAbiLink {
  readonly kind: string
  readonly title: string
  readonly url: string
  readonly scope: string
}

export interface LocalnetExtendedContractAbi<T = unknown> {
  readonly compiler_abi: T
  readonly display_name?: string
  readonly code_hashes: readonly string[]
  readonly links: readonly LocalnetContractAbiLink[]
}

export interface LocalnetCompilerAbiRegistration<T = unknown> {
  readonly codeHash: string
  readonly compilerAbi: T
}

export interface LocalnetVerifiedSourceRequest {
  readonly address?: string
  readonly codeHash?: string
}

export interface TransactionsOptions {
  readonly limit?: number
  readonly lt?: bigint | number | string
  readonly hash?: Buffer | string
  readonly toLt?: bigint | number | string
}

export interface TrackTransactionsOptions {
  readonly limit?: number
  readonly timeoutMs?: number
  readonly pollIntervalMs?: number
}

export interface ApiEnvelope<T> {
  readonly ok: boolean
  readonly result?: T
  readonly error?: string
  readonly code?: number
}

export interface AccountInfoResult {
  readonly balance: string
  readonly last_transaction_id: {
    readonly lt: string
    readonly hash: string
  }
  readonly code: string
  readonly data: string
  readonly frozen_hash: string
  readonly state: "active" | "uninitialized" | "frozen"
}

export interface RunGetMethodResult {
  readonly gas_used?: number
  readonly stack: readonly unknown[]
  readonly exit_code: number
  readonly vm_log?: string
}

export interface RawTransaction {
  readonly data: string
}
