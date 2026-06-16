import type {Buffer} from "node:buffer"

export type LocalnetOptions = {
  readonly endpoint?: string
  readonly authToken?: string
}

export type StartLocalnetOptions = {
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
  readonly autoReset?: boolean
}

export type WaitUntilReadyOptions = {
  readonly timeoutMs?: number
  readonly pollIntervalMs?: number
}

export type CloseLocalnetOptions = {
  readonly timeoutMs?: number
  readonly signal?: NodeJS.Signals
}

export type LocalnetNodeInfo = {
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

export type LocalnetNetworkConditions = {
  readonly response_delay_ms: number
}

export type LocalnetNetworkConditionsOptions = {
  readonly responseDelayMs: number
}

export type SendBocResult = {
  readonly "@type": "ok"
  readonly hash?: string
  readonly hash_norm?: string
}

export type LocalnetBlockId = {
  readonly workchain: number
  readonly shard: number
  readonly seqno: number
  readonly root_hash: string
  readonly file_hash: string
}

export type LocalnetMineResult = {
  readonly blocks_mined: number
  readonly last_block_seqno: number
  readonly blocks: readonly LocalnetBlockId[]
}

export type LocalnetRecoveryPointResult = {
  readonly id: number
  readonly block_seqno: number
}

export type LocalnetClockInfo = {
  readonly current_unix_time: number
  readonly time_offset_seconds: number
  readonly next_block_timestamp: number | null
}

export type LocalnetApiCallStatus = "success" | "failed"
export type LocalnetApiCallType = "read" | "write"
export type LocalnetApiCallFamily = "control" | "emulate" | "json_rpc" | "streaming" | "v2" | "v3"

export type LocalnetApiCallRecord = {
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

export type LocalnetApiCallLog = {
  readonly calls: readonly LocalnetApiCallRecord[]
  readonly total_retained: number
  readonly max_retained: number
}

export type LocalnetStartupWallet = {
  readonly name: string
  readonly mnemonic: readonly string[]
  readonly version: string
  readonly network: string
  readonly address: string
  readonly public_key: string
  readonly wallet_id: number
}

export type LocalnetContractAbiLink = {
  readonly kind: string
  readonly title: string
  readonly url: string
  readonly scope: string
}

export type LocalnetExtendedContractAbi<T = unknown> = {
  readonly compiler_abi: T
  readonly display_name?: string
  readonly code_hashes: readonly string[]
  readonly links: readonly LocalnetContractAbiLink[]
}

export type LocalnetCompilerAbiRegistration<T = unknown> = {
  readonly codeHash: string
  readonly compilerAbi: T
}

export type LocalnetVerifiedSourceRequest = {
  readonly address?: string
  readonly codeHash?: string
}

export type TransactionsOptions = {
  readonly limit?: number
  readonly lt?: bigint | number | string
  readonly hash?: Buffer | string
  readonly toLt?: bigint | number | string
}

export type TrackTransactionsOptions = {
  readonly limit?: number
  readonly timeoutMs?: number
  readonly pollIntervalMs?: number
}

export type ApiEnvelope<T> = {
  readonly ok: boolean
  readonly result?: T
  readonly error?: string
  readonly code?: number
}

export type AccountInfoResult = {
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

export type RunGetMethodResult = {
  readonly gas_used?: number
  readonly stack: readonly unknown[]
  readonly exit_code: number
  readonly vm_log?: string
}

export type RawTransaction = {
  readonly data: string
}

export type LocalnetCoverageRecord = {
  readonly code: string
  readonly vmLog: string
}

export type LocalnetTraceRecord = {
  readonly rawTransaction: string
  readonly shardAccountBefore: string
  readonly shardAccount: string
  readonly parentTransaction?: number
  readonly code?: string
  readonly vmLog: string
  readonly executorLogs?: string
  readonly actions?: string
}

export type LocalnetTreasuryRecord = {
  readonly address: string
  readonly name: string
}

export type EmulateTraceResult = {
  readonly acton_trace_records?: readonly LocalnetTraceRecord[]
  readonly code_cells?: Readonly<Record<string, string>>
  readonly vm_log?: string
}
