import type {ContractABI} from "@ton/tolk-abi-to-typescript"

export interface SourceLocation {
  readonly file: string
  readonly line: number
  readonly column: number
  readonly end_line: number
  readonly end_column: number
  readonly length: number
}

export interface BackendTransaction {
  readonly lt: string
  readonly raw_transaction: string
  readonly parent_transaction: string | undefined
  readonly child_transactions: readonly string[]
  readonly shard_account_before: string
  readonly shard_account: string
  readonly vm_log_diff: string
  readonly executor_logs: string
  readonly executor_actions?: readonly BackendExecutorAction[]
  readonly actions?: string
  readonly dest_contract_info?: string
}

export type BackendExecutorActionFailureReason =
  | {
      readonly type: "not_enough_grams_to_send"
      readonly remaining_balance: string
      readonly required: string
    }
  | {
      readonly type: "cannot_reserve_grams"
      readonly requested: string
      readonly available: string
    }

export type BackendExecutorAction =
  | {
      readonly type: "send_message"
      readonly hash: string
      readonly remaining_balance: string
      readonly location?: SourceLocation
      readonly failure_reason?: BackendExecutorActionFailureReason
      readonly failure_code?: number
    }
  | {
      readonly type: "reserve_currency"
      readonly mode: number
      readonly reserve: string
      readonly balance: string
      readonly original_balance: string
      readonly changed_remaining_balance: string
      readonly changed_reserved_balance: string
      readonly location?: SourceLocation
      readonly failure_reason?: BackendExecutorActionFailureReason
      readonly failure_code?: number
    }
  | {
      readonly type: "set_code"
      readonly location?: SourceLocation
      readonly failure_code?: number
    }
  | {
      readonly type: "change_library"
      readonly location?: SourceLocation
      readonly failure_code?: number
    }

export interface BackendContractInfo {
  readonly name: string
  readonly display_name?: string
  readonly code_boc64: string
  readonly source_map: unknown
  readonly abi?: ContractABI
}
