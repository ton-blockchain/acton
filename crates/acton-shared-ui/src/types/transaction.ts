import type {Address, Cell, OutAction, Transaction} from "@ton/core"

import type {Abi} from "./index"

// eslint-disable-next-line functional/type-declaration-immutability
export interface TransactionInfo {
  readonly lt: string
  readonly address: Address | undefined
  readonly transaction: Transaction
  readonly vmLogDiff: string
  readonly executorLogs: string
  readonly actions: Cell | undefined
  readonly outActions: readonly OutAction[]
  readonly contractName: string | undefined
  readonly shardAccountBefore: string
  readonly shardAccountAfter: string
  parent: TransactionInfo | undefined
  children: readonly TransactionInfo[]
}

export interface ContractData {
  readonly displayName: string
  readonly address: Address
  readonly letter: string
  readonly abi?: Abi
}
