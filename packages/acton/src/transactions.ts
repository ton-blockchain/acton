import type {Address, Transaction} from "@ton/core"

import {formatAddress, parseAddress} from "./address.js"
import {ActonError} from "./errors.js"

export type TransactionEndpointsMatch = {
  readonly from?: Address | string
  readonly to?: Address | string
}

export type TransactionMatch = TransactionEndpointsMatch & {
  readonly deploy?: boolean
  readonly exitCode?: number
  readonly success?: boolean
}

export type FailedTransactionMatch = TransactionEndpointsMatch & {
  readonly exitCode?: number
}

export class TransactionAssertionError extends ActonError {
  constructor(
    readonly label: string,
    readonly transactions: readonly Transaction[],
    readonly match: TransactionMatch,
    message: string,
  ) {
    super(message)
  }
}

export function isTransactionAssertionError(error: unknown): error is TransactionAssertionError {
  return error instanceof TransactionAssertionError
}

export function findTransaction(
  transactions: readonly Transaction[],
  match: TransactionMatch = {},
): Transaction | undefined {
  return transactions.find(transaction => matchesTransaction(transaction, match))
}

export function expectSuccessfulDeploy(
  transactions: readonly Transaction[],
  match: TransactionEndpointsMatch = {},
): Transaction {
  return expectMatchingTransaction(
    transactions,
    {...match, deploy: true, exitCode: 0, success: true},
    "successful deploy transaction",
  )
}

export function expectSuccessfulTx(
  transactions: readonly Transaction[],
  match: TransactionEndpointsMatch = {},
): Transaction {
  return expectMatchingTransaction(
    transactions,
    {...match, exitCode: 0, success: true},
    "successful transaction",
  )
}

export function expectFailedTx(
  transactions: readonly Transaction[],
  match: FailedTransactionMatch = {},
): Transaction {
  return expectMatchingTransaction(transactions, {...match, success: false}, "failed transaction")
}

export function transactionExitCode(transaction: Transaction): number | undefined {
  if (transaction.description.type !== "generic") {
    return undefined
  }

  const phase = transaction.description.computePhase
  return phase.type === "vm" ? phase.exitCode : undefined
}

export function transactionSucceeded(transaction: Transaction): boolean {
  if (transaction.description.type !== "generic") {
    return false
  }

  const phase = transaction.description.computePhase
  if (phase.type !== "vm" || !phase.success || phase.exitCode !== 0) {
    return false
  }

  const actionPhase = transaction.description.actionPhase
  return !transaction.description.aborted && (!actionPhase || actionPhase.success)
}

function expectMatchingTransaction(
  transactions: readonly Transaction[],
  match: TransactionMatch,
  label: string,
): Transaction {
  const transaction = findTransaction(transactions, match)
  if (transaction) {
    return transaction
  }

  const message = `Expected ${label} matching ${describeMatch(match)}, got ${describeTransactions(
    transactions,
  )}`
  throw new TransactionAssertionError(label, transactions, match, message)
}

function matchesTransaction(transaction: Transaction, match: TransactionMatch): boolean {
  if (match.from !== undefined && !matchesAddress(inboundSource(transaction), match.from)) {
    return false
  }

  if (match.to !== undefined && !matchesAddress(inboundDestination(transaction), match.to)) {
    return false
  }

  if (match.deploy !== undefined && isDeployTransaction(transaction) !== match.deploy) {
    return false
  }

  if (match.exitCode !== undefined && transactionExitCode(transaction) !== match.exitCode) {
    return false
  }

  if (match.success !== undefined && transactionSucceeded(transaction) !== match.success) {
    return false
  }

  return true
}

function isDeployTransaction(transaction: Transaction): boolean {
  return transaction.oldStatus !== "active" && transaction.endStatus === "active"
}

function inboundSource(transaction: Transaction): Address | undefined {
  const info = transaction.inMessage?.info
  return info?.type === "internal" ? info.src : undefined
}

function inboundDestination(transaction: Transaction): Address | undefined {
  const info = transaction.inMessage?.info
  if (info?.type === "internal" || info?.type === "external-in") {
    return info.dest
  }
  return undefined
}

function matchesAddress(actual: Address | undefined, expected: Address | string): boolean {
  return actual !== undefined && actual.equals(parseAddress(expected))
}

function describeMatch(match: TransactionMatch): string {
  const parts: string[] = []
  if (match.from !== undefined) {
    parts.push(`from=${formatAddress(parseAddress(match.from))}`)
  }
  if (match.to !== undefined) {
    parts.push(`to=${formatAddress(parseAddress(match.to))}`)
  }
  if (match.deploy !== undefined) {
    parts.push(`deploy=${String(match.deploy)}`)
  }
  if (match.exitCode !== undefined) {
    parts.push(`exitCode=${String(match.exitCode)}`)
  }
  if (match.success !== undefined) {
    parts.push(`success=${String(match.success)}`)
  }
  return parts.length === 0 ? "{}" : `{ ${parts.join(", ")} }`
}

function describeTransactions(transactions: readonly Transaction[]): string {
  if (transactions.length === 0) {
    return "no transactions"
  }

  return transactions.map(transaction => describeTransaction(transaction)).join("; ")
}

function describeTransaction(transaction: Transaction): string {
  const from = inboundSource(transaction)
  const to = inboundDestination(transaction)
  const exitCode = transactionExitCode(transaction)

  return `{ from=${formatOptionalAddress(from)}, to=${formatOptionalAddress(to)}, deploy=${String(
    isDeployTransaction(transaction),
  )}, exitCode=${exitCode === undefined ? "none" : String(exitCode)}, success=${String(
    transactionSucceeded(transaction),
  )} }`
}

function formatOptionalAddress(address: Address | undefined): string {
  return address === undefined ? "none" : formatAddress(address)
}
