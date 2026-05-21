import type {Buffer} from "node:buffer"

export type LocalnetOptions = {
  readonly endpoint?: string
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
  readonly state_source: string
  readonly fork_network: string | null
  readonly fork_block_number: number | null
}

export type SendBocResult = {
  readonly "@type": "ok"
  readonly hash?: string
  readonly hash_norm?: string
}

export type TransactionsOptions = {
  readonly limit?: number
  readonly lt?: bigint | number | string
  readonly hash?: Buffer | string
  readonly toLt?: bigint | number | string
}

export type TrackTransactionsOptions = {
  readonly limit?: number
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
