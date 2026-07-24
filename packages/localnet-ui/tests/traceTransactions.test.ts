import {describe, expect, test} from "bun:test"

import type {TransactionInfo} from "@acton/transaction-ui"
import type {Transaction} from "@ton/core"

import {isTraceSuccessful} from "../src/explorer/api/traceTransactions"
import type {V3Action} from "../src/explorer/api/types"

interface TransactionResult {
  readonly computeSuccess?: boolean
  readonly actionSuccess?: boolean
  readonly aborted?: boolean
}

function transaction(lt: string, result: TransactionResult = {}): TransactionInfo {
  const computeSuccess = result.computeSuccess ?? true
  const actionSuccess = result.actionSuccess ?? true
  return {
    id: `transaction-${lt}`,
    lt,
    transaction: {
      description: {
        type: "generic",
        aborted: result.aborted ?? false,
        computePhase: {
          type: "vm",
          success: computeSuccess,
        },
        actionPhase: {
          success: actionSuccess,
        },
      },
    } as Transaction,
    parent: undefined,
    children: [],
  } as TransactionInfo
}

function action(success: boolean | null): V3Action {
  return {success} as V3Action
}

describe("trace status", () => {
  test("uses semantic actions before technical transaction leaves", () => {
    const root = transaction("10")
    const failedLeaf = transaction("20", {computeSuccess: false, aborted: true})
    failedLeaf.parent = root
    root.children = [failedLeaf]

    expect({
      successfulActionsWithFailedLeaf: isTraceSuccessful(
        [failedLeaf, root],
        [action(true), action(null)],
      ),
      failedActionWithSuccessfulRoot: isTraceSuccessful([root], [action(true), action(false)]),
    }).toMatchInlineSnapshot(`
      {
        "failedActionWithSuccessfulRoot": false,
        "successfulActionsWithFailedLeaf": true,
      }
    `)
  })

  test("falls back to the earliest root transaction", () => {
    const successfulRoot = transaction("10")
    const laterRoot = transaction("30", {actionSuccess: false, aborted: true})
    const failedLeaf = transaction("20", {computeSuccess: false, aborted: true})
    failedLeaf.parent = successfulRoot
    successfulRoot.children = [failedLeaf]

    expect({
      failedLeafListedFirst: isTraceSuccessful([failedLeaf, successfulRoot], []),
      earliestRootWins: isTraceSuccessful([laterRoot, successfulRoot], []),
      abortedRoot: isTraceSuccessful([transaction("10", {aborted: true})], []),
      failedCompute: isTraceSuccessful([transaction("10", {computeSuccess: false})], []),
      failedAction: isTraceSuccessful([transaction("10", {actionSuccess: false})], []),
      emptyTrace: isTraceSuccessful([], []),
    }).toMatchInlineSnapshot(`
      {
        "abortedRoot": false,
        "earliestRootWins": true,
        "emptyTrace": false,
        "failedAction": false,
        "failedCompute": false,
        "failedLeafListedFirst": true,
      }
    `)
  })
})
