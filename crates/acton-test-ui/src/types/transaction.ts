import type { Address, Cell, OutAction, Transaction } from "@ton/core"

export interface TransactionInfo {
  readonly lt: string
  readonly address: Address | undefined
  readonly transaction: Transaction
  readonly vmLogDiff: string
  readonly executorLogs: string
  readonly actions: Cell | undefined
  readonly outActions: OutAction[]
  readonly contractName: string | undefined
  readonly shardAccountBefore: string
  readonly shardAccountAfter: string
  parent: TransactionInfo | undefined
  children: readonly TransactionInfo[]
}

export interface AbiMessage {
  readonly name: string
  readonly opcode: number | undefined
}

export interface Abi {
  readonly messages: AbiMessage[]
  readonly exitCodes?: Record<number, string>
}

export interface ContractData {
  readonly displayName: string
  readonly address: Address
  readonly letter: string
  readonly abi?: Abi
}
