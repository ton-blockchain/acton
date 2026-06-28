import type {Address, Cell, OutAction, Transaction} from "@ton/core"
import type {ContractABI} from "@ton/tolk-abi-to-typescript"

import type {BackendExecutorAction} from "./index"

export interface ParsedTransactionBody {
  readonly name: string
  readonly value: ParsedValue
}

export interface ParsedContractStorage {
  readonly name: string
  readonly value: ParsedValue
}

export interface ParsedValueObjectEntry {
  readonly key: string
  readonly value: ParsedValue
}

export interface ParsedValueMapEntry {
  readonly key: ParsedValue
  readonly value: ParsedValue
}

export type ParsedValue =
  | {
      readonly kind: "null"
    }
  | {
      readonly kind: "void"
    }
  | {
      readonly kind: "address"
      readonly value: string
    }
  | {
      readonly kind: "boolean"
      readonly value: boolean
    }
  | {
      readonly kind: "scalar"
      readonly value: string
      readonly rawValue?: string
      readonly typeName?: string
    }
  | {
      readonly kind: "array"
      readonly items: readonly ParsedValue[]
    }
  | {
      readonly kind: "object"
      readonly typeName?: string
      readonly entries: readonly ParsedValueObjectEntry[]
    }
  | {
      readonly kind: "map"
      readonly typeName?: string
      readonly entries: readonly ParsedValueMapEntry[]
    }

// eslint-disable-next-line functional/type-declaration-immutability
export interface TransactionInfo {
  readonly id: string
  readonly lt: string
  readonly address: Address | undefined
  readonly transaction: Transaction
  readonly vmLogDiff: string
  readonly executorLogs: string
  readonly executorActions: readonly BackendExecutorAction[]
  readonly actions: Cell | undefined
  readonly outActions: readonly OutAction[]
  readonly contractName: string | undefined
  contractAbi: ContractABI | undefined
  readonly shardAccountBefore: string
  readonly shardAccountAfter: string
  readonly codeHashBefore?: string
  readonly codeHashAfter?: string
  readonly accountBalanceBefore?: bigint
  readonly accountBalanceAfter?: bigint
  parsedBody: ParsedTransactionBody | undefined
  parsedStorageBefore: ParsedContractStorage | undefined
  parsedStorageAfter: ParsedContractStorage | undefined
  parent: TransactionInfo | undefined
  children: readonly TransactionInfo[]
}

export interface LoadedTransactionActions {
  readonly actions: Cell | undefined
  readonly outActions: readonly OutAction[]
  readonly executorActions?: readonly BackendExecutorAction[]
}

export interface ContractData {
  readonly displayName: string
  readonly address: Address
  readonly letter: string
  readonly abi?: ContractABI
}

export interface ValueFlowItem {
  readonly address: string
  readonly change: bigint
  readonly fee: bigint
}
