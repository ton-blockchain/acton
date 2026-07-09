import {Cell, loadMessage} from "@ton/core"
import {emulateRawMessage as emulateRawMessageCore} from "@ton/retracer-core"
import type {
  EmulateRawMessageOptions,
  EmulateRawMessageResult as CoreEmulateRawMessageResult,
  TraceResult,
} from "@ton/retracer-core"
import type {TransactionInfo} from "@acton/shared-ui"

import {buildTraceTransactionInfos} from "../../../api/traceTransactions"
import type {V3Trace} from "../../../api/types"
import type {ExplorerNetworkInfo} from "../../../hooks/useNetworkInfo"
import type {RetraceResultAndCode} from "./types"
import {getRetraceNetworkConfig} from "./retraceNetwork"
import {extractCodeAndTrace} from "./traceTx"

export type RawMessageEmulationOptions = Pick<
  EmulateRawMessageOptions,
  "accountStateOverrides" | "ignoreChksig" | "maxTransactions" | "mcSeqno"
>

export interface RawMessageEmulationResult {
  readonly result: CoreEmulateRawMessageResult
  readonly trace: V3Trace
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
  const result = await emulateRawMessageCore(getRetraceNetworkConfig(network), messageCell, options)
  const trace: V3Trace = result.trace
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
