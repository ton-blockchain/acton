export enum TestStatus {
  Passed = "Passed",
  Failed = "Failed",
  Skipped = "Skipped",
  Todo = "Todo",
}

export interface TestReport {
  readonly name: string
  readonly suite_name: string
  readonly status: TestStatus
  readonly message?: string
  readonly trace_path?: string
}

export interface Transaction {
  readonly dest_contract_info: string
  readonly vm_log_diff: string
  readonly executor_logs: string
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

export interface BackendContractInfo {
  readonly name: string
  readonly code_boc64: string
  readonly source_map: any
  readonly abi?: any
}

export interface BackendTransaction {
  readonly lt: string
  readonly raw_transaction: string
  readonly parent_transaction: string | null
  readonly child_transactions: string[]
  readonly shard_account_before: string
  readonly shard_account: string
  readonly vm_log_diff: string
  readonly executor_logs: string
  readonly actions?: string
  readonly dest_contract_info?: string
}

export interface AppState {
  readonly reports: TestReport[]
  readonly currentTrace?: Trace
}
