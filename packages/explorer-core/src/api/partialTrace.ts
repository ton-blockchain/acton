import {hashToHex} from "../components/utils"

import type {V3AddressBookRow, V3Transaction, V3TransactionDetailsResponse} from "./types"

export interface PartialTraceState {
  readonly traceId: string
  readonly totalTransactionCount: number
  readonly originKey: string
  readonly transactionsByKey: ReadonlyMap<string, V3Transaction>
  readonly visibleKeys: ReadonlySet<string>
  readonly parentByChildKey: ReadonlyMap<string, string>
  readonly expandedMessageKeys: ReadonlySet<string>
  readonly pathComplete: boolean
}

export interface PartialTraceLoadResult {
  readonly state: PartialTraceState
  readonly addressBooks: readonly Record<string, V3AddressBookRow>[]
  readonly failedRequests: number
}

type MessageTransactionLookup = (
  messageHash: string,
  direction: "in" | "out",
) => Promise<V3TransactionDetailsResponse>

const TRANSACTION_LOAD_LIMIT = 10

export const partialTraceKey = (reference: string): string =>
  hashToHex(reference) ?? reference.trim().toLowerCase()

export const createPartialTraceState = ({
  traceId,
  totalTransactionCount,
  origin,
  selected,
}: {
  readonly traceId: string
  readonly totalTransactionCount: number
  readonly origin: V3Transaction
  readonly selected: V3Transaction
}): PartialTraceState => {
  const originKey = partialTraceKey(origin.hash)
  const selectedKey = partialTraceKey(selected.hash)
  return {
    traceId,
    totalTransactionCount,
    originKey,
    transactionsByKey: new Map([
      [originKey, origin],
      [selectedKey, selected],
    ]),
    visibleKeys: new Set([selectedKey]),
    parentByChildKey: new Map(),
    expandedMessageKeys: new Set(),
    pathComplete: false,
  }
}

export const partialTraceTransactionsMap = (
  state: PartialTraceState,
): Record<string, V3Transaction> =>
  Object.fromEntries(
    [...state.visibleKeys]
      .map(key => state.transactionsByKey.get(key))
      .filter((transaction): transaction is V3Transaction => transaction !== undefined)
      .map(transaction => [transaction.hash, transaction]),
  )

export async function restorePartialTracePath(
  state: PartialTraceState,
  lookup: MessageTransactionLookup,
  shouldContinue: () => boolean,
): Promise<PartialTraceLoadResult> {
  const transactionsByKey = new Map(state.transactionsByKey)
  const visibleKeys = new Set(state.visibleKeys)
  const parentByChildKey = new Map(state.parentByChildKey)
  const addressBooks: Record<string, V3AddressBookRow>[] = []
  let pathComplete = false
  let currentKey = [...visibleKeys]
    .filter(key => !parentByChildKey.has(key))
    .map(key => [key, transactionsByKey.get(key)] as const)
    .filter((entry): entry is readonly [string, V3Transaction] => entry[1] !== undefined)
    .sort(([, left], [, right]) => compareTransactionsByLt(left, right))[0]?.[0]

  for (let step = 0; step < TRANSACTION_LOAD_LIMIT && currentKey; step += 1) {
    if (currentKey === state.originKey) {
      pathComplete = true
      break
    }

    const currentTransaction = transactionsByKey.get(currentKey)
    const incomingMessage = currentTransaction?.in_msg
    if (!currentTransaction || !incomingMessage?.hash || !incomingMessage.source) {
      throw new Error("The causal parent of this transaction is unavailable.")
    }

    let parentEntry = [...transactionsByKey.entries()].find(
      ([candidateKey, candidate]) =>
        candidateKey !== currentKey && transactionContainsMessage(candidate, incomingMessage.hash),
    )

    if (!parentEntry) {
      const response = await lookup(incomingMessage.hash, "out")
      if (!shouldContinue()) {
        break
      }
      addressBooks.push(response.address_book)

      const parent = response.transactions.find(
        candidate =>
          transactionMatchesTrace(candidate, state.traceId) &&
          transactionContainsMessage(candidate, incomingMessage.hash),
      )
      if (!parent) {
        throw new Error("Toncenter did not return the causal parent transaction.")
      }

      const parentKey = partialTraceKey(parent.hash)
      transactionsByKey.set(parentKey, parent)
      parentEntry = [parentKey, parent]
    }

    const [parentKey] = parentEntry
    visibleKeys.add(parentKey)
    parentByChildKey.set(currentKey, parentKey)
    currentKey = parentKey
    if (parentKey === state.originKey) {
      pathComplete = true
      break
    }
  }

  return {
    state: {
      ...state,
      transactionsByKey,
      visibleKeys,
      parentByChildKey,
      pathComplete,
    },
    addressBooks,
    failedRequests: 0,
  }
}

export async function loadPartialTraceBranches(
  state: PartialTraceState,
  lookup: MessageTransactionLookup,
  shouldContinue: () => boolean,
): Promise<PartialTraceLoadResult> {
  const transactionsByKey = new Map(state.transactionsByKey)
  const visibleKeys = new Set(state.visibleKeys)
  const parentByChildKey = new Map(state.parentByChildKey)
  const expandedMessageKeys = new Set(state.expandedMessageKeys)
  const loadedIncomingMessageKeys = new Set(
    [...visibleKeys]
      .map(key => transactionsByKey.get(key)?.in_msg?.hash)
      .filter((messageHash): messageHash is string => messageHash !== undefined)
      .map(partialTraceKey),
  )
  const candidates = [...visibleKeys]
    .map(key => [key, transactionsByKey.get(key)] as const)
    .filter((entry): entry is readonly [string, V3Transaction] => entry[1] !== undefined)
    .sort(([, left], [, right]) => compareTransactionsByLt(left, right))
    .flatMap(([parentKey, transaction]) =>
      [...transaction.out_msgs]
        .sort((left, right) => compareLt(left.created_lt, right.created_lt))
        .map(message => ({
          parentKey,
          message,
          messageKey: partialTraceKey(message.hash),
        })),
    )
    .filter(candidate => {
      if (!candidate.message.destination) {
        return false
      }
      if (loadedIncomingMessageKeys.has(candidate.messageKey)) {
        expandedMessageKeys.add(candidate.messageKey)
        return false
      }
      return !expandedMessageKeys.has(candidate.messageKey)
    })
    .slice(0, TRANSACTION_LOAD_LIMIT)

  if (candidates.length === 0) {
    throw new Error("No additional adjacent transactions were found.")
  }

  const results = await Promise.allSettled(
    candidates.map(async candidate => ({
      candidate,
      response: await lookup(candidate.message.hash, "in"),
    })),
  )
  const addressBooks: Record<string, V3AddressBookRow>[] = []
  let failedRequests = 0
  let loadedTransactions = 0

  if (!shouldContinue()) {
    return {state, addressBooks, failedRequests}
  }

  for (const result of results) {
    if (result.status === "rejected") {
      failedRequests += 1
      continue
    }

    const {candidate, response} = result.value
    addressBooks.push(response.address_book)
    const child = response.transactions.find(
      transaction =>
        transactionMatchesTrace(transaction, state.traceId) &&
        transaction.in_msg?.hash !== undefined &&
        partialTraceKey(transaction.in_msg.hash) === candidate.messageKey,
    )
    if (!child) {
      failedRequests += 1
      continue
    }

    const childKey = partialTraceKey(child.hash)
    transactionsByKey.set(childKey, child)
    visibleKeys.add(childKey)
    parentByChildKey.set(childKey, candidate.parentKey)
    expandedMessageKeys.add(candidate.messageKey)
    loadedTransactions += 1
  }

  if (loadedTransactions === 0) {
    throw new Error("Toncenter did not return any adjacent transactions.")
  }

  return {
    state: {
      ...state,
      transactionsByKey,
      visibleKeys,
      parentByChildKey,
      expandedMessageKeys,
    },
    addressBooks,
    failedRequests,
  }
}

const compareTransactionsByLt = (left: V3Transaction, right: V3Transaction): number =>
  compareLt(left.lt, right.lt)

const compareLt = (left: string, right: string): number => {
  const leftLt = BigInt(left)
  const rightLt = BigInt(right)
  return leftLt < rightLt ? -1 : leftLt > rightLt ? 1 : 0
}

const transactionMatchesTrace = (transaction: V3Transaction, traceId: string): boolean =>
  !transaction.trace_id || partialTraceKey(transaction.trace_id) === partialTraceKey(traceId)

const transactionContainsMessage = (transaction: V3Transaction, messageHash: string): boolean => {
  const messageKey = partialTraceKey(messageHash)
  return transaction.out_msgs.some(message => partialTraceKey(message.hash) === messageKey)
}
