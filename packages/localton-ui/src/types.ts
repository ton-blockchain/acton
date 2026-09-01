export interface ChainHead {
  readonly seqno: number
  readonly root_hash: string
  readonly file_hash: string
  readonly gen_utime: number
  readonly observed_at: number
  readonly shard_count: number
}

export interface ShardHead {
  readonly workchain: number
  readonly shard: string
  readonly seqno: number
  readonly root_hash: string
  readonly file_hash: string
  readonly gen_utime: number
  readonly before_split: boolean
  readonly before_merge: boolean
  readonly want_split: boolean
  readonly want_merge: boolean
}

export interface NetworkTotals {
  readonly observers: number
  readonly online_observers: number
  readonly nodes: number
  readonly online_nodes: number
  readonly synchronized_nodes: number
  readonly catching_up_nodes: number
  readonly configured_validators: number
  readonly active_validators: number
  readonly full_nodes: number
  readonly masterchain_blocks: number
  readonly shard_blocks: number
}

export interface ObserverView {
  readonly observer_id: string
  readonly endpoint: string
  readonly generated_at: number
  readonly expires_at: number
  readonly online: boolean
  readonly node_count: number
}

export interface StateDownloadProgress {
  readonly downloaded_bytes: number
  readonly total_bytes: number
  readonly bytes_per_second: number
  readonly remaining_seconds: number
}

export interface InitialSyncProgress {
  readonly stage:
    | "starting"
    | "discovering_key_blocks"
    | "downloading_masterchain_state"
    | "downloading_shard_states"
    | "preparing"
  readonly masterchain_seqno: number | null
  readonly current_part: number | null
  readonly total_parts: number | null
  readonly state_download: StateDownloadProgress | null
}

export interface NodeView {
  readonly observer_id: string
  readonly generated_at: number
  readonly expires_at: number
  readonly online: boolean
  readonly sync_status: "synced" | "catching_up" | "unknown" | "offline"
  readonly active_validator: boolean
  readonly validator_status:
    | "not_configured"
    | "validating"
    | "leaving"
    | "joining"
    | "waiting"
    | "inactive"
    | "unknown"
  readonly produced_masterchain_blocks: number
  readonly produced_shard_blocks: number
  readonly name: string
  readonly public_ip: string
  readonly roles: readonly string[]
  readonly running: boolean
  readonly process_id: number | null
  readonly status: string
  readonly last_error: string | null
  readonly head_seqno: number | null
  readonly network_head_seqno: number | null
  readonly sync_initial_masterchain_block_time: number | null
  readonly sync_masterchain_block_time: number | null
  readonly sync_target_time: number | null
  readonly initial_sync_progress: InitialSyncProgress | null
  readonly sync_progressed_at: number | null
  readonly sync_lag_blocks: number | null
  readonly participate_in_elections: boolean
  readonly current_validator: boolean | null
  readonly next_validator: boolean | null
  readonly validator_public_key: string | null
  readonly validator_public_keys: readonly string[]
  readonly validator_adnl: string | null
}

export interface ProductionView {
  readonly creator: string
  readonly masterchain_blocks: number
  readonly shard_blocks: number
  readonly last_block_at: number
}

export interface ElectionObservation {
  readonly round_id: number
  readonly stage:
    | "validation"
    | "accepting_entries"
    | "finalizing"
    | "next_set_ready"
    | "retrying"
    | "activation_overdue"
  readonly validation_started_at: number
  readonly elections_open_at: number
  readonly elections_close_at: number
  readonly next_set_activation_at: number
  readonly validators_elected_for: number
  readonly stake_held_for: number
  readonly current_validators: number
  readonly current_main_validators: number
  readonly next_validators: number | null
}

export interface NetworkView {
  readonly protocol_version: number
  readonly network_id: string
  readonly generated_at: number
  readonly chain: ChainHead | null
  readonly chain_source: "local_verification" | "peer_attestation" | "unavailable"
  readonly shards: readonly ShardHead[]
  readonly election: ElectionObservation | null
  readonly totals: NetworkTotals
  readonly observers: readonly ObserverView[]
  readonly nodes: readonly NodeView[]
  readonly production: readonly ProductionView[]
}
