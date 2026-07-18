import type {BackendTransaction} from "@acton/transaction-ui"

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

export interface FailedMessage {
  readonly error: string
  readonly vm_log_diff?: string
  readonly vm_exit_code?: number
  readonly executor_logs?: string
}

export interface TransactionList {
  readonly name?: string
  readonly is_treasury_deploy?: boolean
  readonly transactions: BackendTransaction[]
  readonly failed_messages?: FailedMessage[]
}

export interface Trace {
  readonly name: string
  readonly traces: TransactionList[]
  readonly skipped_traces_count?: number
  readonly contracts: string[]
  readonly wallets: Record<string, string>
}
