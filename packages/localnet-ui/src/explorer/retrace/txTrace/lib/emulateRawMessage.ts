import {Cell, loadMessage} from "@ton/core"
import type {
  EmulatedMessage,
  EmulatedTrace,
  EmulatedTraceNode,
  EmulatedTransaction,
  EmulateRawMessageOptions,
  EmulateRawMessageResult as CoreEmulateRawMessageResult,
  TraceResult,
} from "@ton/retracer-core"
import type {TransactionInfo} from "@acton/transaction-ui"

import {buildTraceTransactionInfos} from "../../../api/traceTransactions"
import type {V3CompleteTrace, V3Message, V3TraceNode, V3Transaction} from "../../../api/types"
import type {ExplorerNetworkInfo} from "../../../hooks/useNetworkInfo"
import type {RetraceResultAndCode} from "./types"
import {extractCodeAndTrace, getRetraceNetworkConfig} from "./traceTx"

export type RawMessageEmulationOptions = Pick<
  EmulateRawMessageOptions,
  "accountStateOverrides" | "ignoreChksig" | "maxTransactions" | "mcSeqno" | "now"
>

export interface RawMessageEmulationResult {
  readonly result: CoreEmulateRawMessageResult
  readonly trace: V3CompleteTrace
  readonly transactions: readonly TransactionInfo[]
  readonly retraceResultsByHash: ReadonlyMap<string, RetraceResultAndCode>
}

export function parseRawMessageBoc(rawMessage: string): Cell {
  const trimmed = rawMessage.trim()
  if (!trimmed) {
    throw new Error("Message BOC cannot be empty")
  }

  const cell = parseCell(trimmed)
  loadMessage(cell.asSlice())
  return cell
}

export async function emulateRawMessageBoc(
  rawMessage: string,
  network: ExplorerNetworkInfo,
  options: RawMessageEmulationOptions = {},
): Promise<RawMessageEmulationResult> {
  const messageCell = parseRawMessageBoc(rawMessage)
  const {emulateRawMessage} = await import("@ton/retracer-core")
  const result = await emulateRawMessage(getRetraceNetworkConfig(network), messageCell, options)
  const trace = normalizeEmulatedTrace(result.trace)
  const transactions = buildTraceTransactionInfos(trace.transactions, trace.trace)
  const retraceResultsByHash = new Map(
    Object.entries(result.transactions).map(([txHash, traceResult]) => [
      txHash.toLowerCase(),
      createRetraceResult(traceResult, network),
    ]),
  )

  return {
    result,
    trace,
    transactions,
    retraceResultsByHash,
  }
}

function normalizeEmulatedTrace(trace: EmulatedTrace): V3CompleteTrace {
  return {
    ...trace,
    trace: normalizeTraceNode(trace.trace),
    transactions: Object.fromEntries(
      Object.entries(trace.transactions).map(([hash, transaction]) => [
        hash,
        normalizeTransaction(transaction),
      ]),
    ),
  }
}

function normalizeTraceNode(node: EmulatedTraceNode): V3TraceNode {
  return {
    ...node,
    in_msg: node.in_msg ? normalizeMessage(node.in_msg) : node.in_msg,
    transaction: node.transaction ? normalizeTransaction(node.transaction) : undefined,
    children: node.children?.map(normalizeTraceNode),
  }
}

function normalizeTransaction(transaction: EmulatedTransaction): V3Transaction {
  // Emulated skipped compute and absent action phases are intentionally sparse.
  return {
    ...transaction,
    in_msg: transaction.in_msg ? normalizeMessage(transaction.in_msg) : transaction.in_msg,
    out_msgs: transaction.out_msgs.map(normalizeMessage),
  } as V3Transaction
}

function normalizeMessage(message: EmulatedMessage): V3Message {
  return {
    ...message,
    source: message.source ?? undefined,
    destination: message.destination ?? undefined,
    value: message.value ?? "0",
    fwd_fee: message.fwd_fee ?? "0",
    ihr_fee: message.ihr_fee ?? "0",
    created_lt: message.created_lt ?? "0",
    created_at: message.created_at ?? "0",
    ihr_disabled: message.ihr_disabled ?? true,
    bounce: message.bounce ?? false,
    bounced: message.bounced ?? false,
    import_fee: message.import_fee ?? "0",
    init_state: message.init_state ?? undefined,
  }
}

function createRetraceResult(
  result: TraceResult,
  network: ExplorerNetworkInfo,
): RetraceResultAndCode {
  const {code, traceInfo, exitCode} = extractCodeAndTrace(result.codeCell, result.emulatedTx.vmLogs)

  return {
    result,
    code,
    trace: traceInfo,
    exitCode,
    network,
    sourceTrace: result.sourceTrace,
  }
}

function parseCell(value: string): Cell {
  try {
    return Cell.fromHex(value.replace(/^0x/i, ""))
  } catch {
    try {
      return Cell.fromBase64(value)
    } catch {
      throw new Error("Message BOC must be encoded as hex or base64")
    }
  }
}
