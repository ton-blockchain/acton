import type {TransactionInfo} from "@retrace/sandbox/lib/transaction"
import type {ContractData} from "@retrace/sandbox/lib/contract"

type ContractStateChange = unknown
type ValueFlow = unknown

/**
 * Represent single test from sandbox
 */
export interface TestData {
  readonly testName: string
  readonly transactions: TransactionInfo[]
  readonly timestamp?: number
  readonly contracts: Map<string, ContractData>
  readonly changes: readonly ContractStateChange[]
  readonly valueFlow?: Map<string, ValueFlow>
}
