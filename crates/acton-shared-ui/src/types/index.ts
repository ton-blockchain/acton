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

export interface BackendTransaction {
  readonly lt: string
  readonly raw_transaction: string
  readonly parent_transaction: string | undefined
  readonly child_transactions: readonly string[]
  readonly shard_account_before: string
  readonly shard_account: string
  readonly vm_log_diff: string
  readonly executor_logs: string
  readonly actions?: string
  readonly dest_contract_info?: string
}

export interface TransactionList {
  readonly transactions: BackendTransaction[]
}

export interface Trace {
  readonly name: string
  readonly traces: TransactionList[]
  readonly contracts: string[]
  readonly wallets: Record<string, string>
}

export interface AbiMessage {
  readonly name: string
  readonly opcode: number | undefined
}

export interface Abi {
  readonly messages: AbiMessage[]
  readonly exitCodes?: Record<number, string>
}

export interface BackendContractInfo {
  readonly name: string
  readonly code_boc64: string
  readonly source_map: unknown
  readonly abi?: Abi
}

export * from "./transaction"
