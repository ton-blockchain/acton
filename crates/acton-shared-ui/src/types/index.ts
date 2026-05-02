import type {ContractABI} from "gen-typescript-from-tolk-dev"

export enum TestStatus {
  Passed = "Passed",
  Failed = "Failed",
  Skipped = "Skipped",
  Todo = "Todo",
}

export interface FailedTransactionContext {
  readonly from_address?: string
  readonly to_address?: string
  readonly params: [string, string][]
}

export interface TestReport {
  readonly name: string
  readonly suite_name: string
  readonly file_path: string
  readonly row: number
  readonly column: number
  readonly duration: {secs: number; nanos: number}
  readonly status: TestStatus
  readonly message?: string
  readonly detailed_message?: string
  readonly failed_transactions?: BackendTransaction[]
  readonly failed_transaction_context?: FailedTransactionContext
  readonly details?: string
  readonly trace_path?: string
}

export interface TestExecutionLogs {
  readonly stdout?: string
  readonly stderr?: string
  readonly vm_log?: string
}

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

export interface FailedMessage {
  readonly error: string
  readonly vm_log_diff?: string
  readonly vm_exit_code?: number
  readonly executor_logs?: string
}

export type BackendExecutorActionFailureReason =
  | {
      readonly type: "not_enough_toncoin_to_send"
      readonly remaining_balance: string
      readonly required: string
    }
  | {
      readonly type: "cannot_reserve_toncoin"
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

export interface TransactionList {
  readonly name?: string
  readonly transactions: BackendTransaction[]
  readonly failed_messages?: FailedMessage[]
}

export interface Trace {
  readonly name: string
  readonly traces: TransactionList[]
  readonly contracts: string[]
  readonly wallets: Record<string, string>
}

export interface BackendContractInfo {
  readonly name: string
  readonly code_boc64: string
  readonly source_map: unknown
  readonly abi?: ContractABI
}

export * from "./transaction"
