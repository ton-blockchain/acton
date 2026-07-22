import {describe, expect, test} from "bun:test"
import {Address} from "@ton/core"

import type {TransactionInfo} from "@acton/transaction-ui"

import {buildTraceStateChangeItems} from "../src/explorer/components/TraceStateChangesPanel"

const CONTRACT_ADDRESS = Address.parse("EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c")

function transaction(lt: string, balanceBefore: bigint, balanceAfter: bigint): TransactionInfo {
  return {
    id: `transaction-${lt}`,
    lt,
    address: CONTRACT_ADDRESS,
    transaction: {oldStatus: "active", endStatus: "active"},
    vmLogDiff: "",
    executorLogs: "",
    executorActions: [],
    actions: undefined,
    outActions: [],
    contractName: undefined,
    contractAbi: undefined,
    shardAccountBefore: "",
    shardAccountAfter: "",
    accountBalanceBefore: balanceBefore,
    accountBalanceAfter: balanceAfter,
    parsedBody: undefined,
    parsedStorageBefore: undefined,
    parsedStorageAfter: undefined,
    parent: undefined,
    children: [],
  } as TransactionInfo
}

describe("trace state changes", () => {
  test("aggregates contract balance changes across transactions ordered by LT", () => {
    const laterTransaction = transaction("20", 2_000_000_000n, 1_750_000_000n)
    const earlierTransaction = transaction("10", 3_000_000_000n, 2_500_000_000n)

    expect({
      changed: buildTraceStateChangeItems([laterTransaction, earlierTransaction]),
      unchanged: buildTraceStateChangeItems([
        transaction("10", 3_000_000_000n, 2_500_000_000n),
        transaction("20", 2_500_000_000n, 3_000_000_000n),
      ]),
    }).toMatchSnapshot()
  })
})
