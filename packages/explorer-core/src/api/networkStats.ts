export interface NetworkTpsWindow {
  readonly window_seconds: number
  readonly coverage_seconds: number
  readonly transactions: number
  readonly tps: number
  readonly complete: boolean
}

export interface NetworkTpsSnapshot {
  readonly status: "syncing" | "ready"
  readonly latest_masterchain_seqno?: number
  readonly latest_block_time?: number
  readonly windows: readonly NetworkTpsWindow[]
}

export type LoadNetworkTps = (signal: AbortSignal) => Promise<NetworkTpsSnapshot>
