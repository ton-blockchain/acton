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

export interface Trace {
  readonly name: string
  readonly txs: {
    readonly transactions: Transaction[]
  }
}

export interface AppState {
  readonly reports: TestReport[]
  readonly currentTrace?: Trace
}
