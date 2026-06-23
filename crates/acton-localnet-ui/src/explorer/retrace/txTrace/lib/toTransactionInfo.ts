import {Cell, loadTransaction} from "@ton/core"
import type {TraceResult} from "txtracer-core/dist/types"

import type {TransactionInfo} from "@acton/shared-ui"

export function toTransactionInfo(result: TraceResult): TransactionInfo {
  const transaction = loadTransaction(Cell.fromHex(result.emulatedTx.raw).asSlice())
  const address = result.inMsg.contract
  const lt = result.emulatedTx.lt.toString()

  return {
    id: `${address.toString()}:${lt}`,
    lt,
    address,
    transaction,
    vmLogDiff: result.emulatedTx.vmLogs,
    executorLogs: result.emulatedTx.executorLogs,
    executorActions: [],
    actions: result.emulatedTx.c5,
    outActions: result.emulatedTx.actions,
    contractName: undefined,
    contractAbi: undefined,
    shardAccountBefore: "",
    shardAccountAfter: "",
    accountBalanceBefore: result.money.balanceBefore,
    accountBalanceAfter: result.money.balanceAfter,
    parsedBody: undefined,
    parsedStorageBefore: undefined,
    parsedStorageAfter: undefined,
    parent: undefined,
    children: [],
  }
}
