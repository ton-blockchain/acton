import {retrace, retraceBaseTx} from "txtracer-core"
import type {TraceResult} from "txtracer-core/dist/types"
import {compileCellWithMapping, decompileCell} from "ton-assembly/dist/runtime/instr"
import {createMappingInfo} from "ton-assembly/dist/trace/mapping"
import {type Step, type TraceInfo} from "ton-assembly/dist/trace"
import {createTraceInfoPerTransaction, findInstructionInfo} from "ton-assembly/dist/trace/trace"
import {parse, print} from "ton-assembly/dist/text"
import * as l from "ton-assembly/dist/logs"
import {Cell} from "@ton/core"

import type {AssemblyMapping, InstructionInfo} from "ton-source-map"

import type {NetworkType, RetraceResultAndCode} from "@retrace/txTrace/ui"
import type {TransactionInfo} from "@retrace/sandbox/lib/transaction"
import type {ContractData} from "@retrace/sandbox/lib/contract"

import {
  type ExtractionResult,
  extractTxInfoFromLink,
  SingleHash,
} from "@retrace/txTrace/lib/links"

import {
  NetworkError,
  TooManyRequests,
  TxHashInvalidError,
  TxNotFoundError,
  TxTraceError,
} from "./errors"

export type ExitCode = {
  readonly num: number
  readonly description: string
  readonly info: undefined | InstructionInfo
}

export interface SandboxTraceResult {
  readonly code: string
  readonly exitCode?: ExitCode
  readonly traceInfo: TraceInfo
}

async function retraceAny(info: ExtractionResult): Promise<TraceResult> {
  if (info.$ === "BaseInfo") {
    return retraceBaseTx(info.testnet, info.info)
  }
  if (info.$ === "SingleHash") {
    return retrace(info.testnet, info.hash)
  }
  if (info.$ === "UnknownNetwork") {
    return retrace(info.testnet, info.hash)
  }

  throw new Error("Invalid extraction result")
}

async function maybeTestnet(link: string): Promise<{result: TraceResult; network: NetworkType}> {
  const txLinkInfo = extractTxInfoFromLink(link)
  if (txLinkInfo === undefined && link.startsWith("https://")) {
    throw new Error("Unsupported link format: " + link)
  }

  try {
    const result = await retraceAny(txLinkInfo ?? SingleHash(link, false))
    return {result, network: txLinkInfo?.testnet ? "testnet" : "mainnet"}
  } catch (error: unknown) {
    if (error instanceof Error && error.message.includes("Cannot find transaction info")) {
      console.log("Cannot find in mainnet, trying to find in testnet")
      if (txLinkInfo?.$ === "UnknownNetwork") {
        txLinkInfo.testnet = true
      }

      const result = await retraceAny(txLinkInfo ?? SingleHash(link, true))
      return {result, network: "testnet"}
    }
    throw error
  }
}

async function doTrace(link: string) {
  try {
    return await maybeTestnet(link)
  } catch (e: unknown) {
    let message = "An unknown error occurred."
    if (e instanceof Error) {
      message = e.message
    } else if (e !== null && e !== undefined) {
      // eslint-disable-next-line @typescript-eslint/no-base-to-string
      message = String(e)
    }

    if (/status code 429/i.test(message)) {
      throw new TooManyRequests(undefined, e)
    }

    if (/status code 422/i.test(message)) {
      throw new TxHashInvalidError(undefined, e)
    }

    if (/not found/i.test(message)) {
      throw new TxNotFoundError(undefined, e)
    }

    if (/network|failed to fetch|fetch failed|timeout|ECONN|ENOTFOUND|ERR_NETWORK/i.test(message)) {
      throw new NetworkError(undefined, e)
    }

    throw new TxTraceError(message, e)
  }
}

export function findException(reversedEntries: l.VmLine[]) {
  const mapped = reversedEntries.map(it => {
    if (it.$ === "VmExceptionHandler") {
      return {
        text: "", // default case, no further explanations
        num: it.errno,
      }
    }
    if (it.$ === "VmException") {
      return {
        text: it.message,
        num: it.errno,
      }
    }
    if (it.$ === "VmUnknown" && it.text.includes("unhandled out-of-gas exception")) {
        return {text: it.text, num: -14}
      }
    return undefined
  })
  const exceptionWithDescription = mapped.find(it => {
    const length = it?.text?.length ?? 0
    return length > 0
  })
  if (exceptionWithDescription) {
    return exceptionWithDescription
  }
  return mapped.find(it => it !== undefined)
}

export function findExitCode(vmLogs: string, mappingInfo: AssemblyMapping) {
  const res = l.parse(vmLogs)
  const reversedEntries = [...res].reverse()
  const description = findException(reversedEntries)
  if (description === undefined) {
    return undefined // no exception found
  }

  // find the last position before exception
  const loc = reversedEntries.find(it => it.$ === "VmLoc")
  const info = findInstructionInfo(mappingInfo, {
    hash: loc?.hash?.toLowerCase() ?? "",
    offset: loc?.offset ?? 0,
    stack: [],
    gas: 0,
    gasCost: 0,
    implicit: false,
  })

  if (info === undefined) {
    return undefined
  }

  const [instructionsInfo, index] = info

  const exitCode: ExitCode = {
    info: instructionsInfo[index],
    description: description.text,
    num: description.num,
  }

  return exitCode
}

function extractCodeAndTrace(
  codeCell: Cell | undefined,
  vmLogs: string,
): {
  code: string
  exitCode?: ExitCode
  traceInfo: TraceInfo
} {
  if (!codeCell) {
    return {code: "// No executable code found", traceInfo: {steps: []}}
  }

  const instructions = decompileCell(codeCell)
  const code = print(instructions)

  const instructionsWithPositions = parse("out.tasm", code)
  if (instructionsWithPositions.$ === "ParseFailure") {
    return {code: code, traceInfo: {steps: []}, exitCode: undefined}
  }

  const [, mapping] = compileCellWithMapping(instructionsWithPositions.instructions)
  const mappingInfo = createMappingInfo(mapping)
  const traceInfo = createTraceInfoPerTransaction(vmLogs, mappingInfo, undefined)[0]

  const exitCode = findExitCode(vmLogs, mappingInfo)
  if (exitCode === undefined) {
    return {code, exitCode: undefined, traceInfo}
  }

  return {code, exitCode, traceInfo}
}

export function traceSandboxTransaction(
  tx: TransactionInfo,
  contracts: Map<string, ContractData>,
): SandboxTraceResult | undefined {
  const computeInfo = tx.computeInfo
  if (computeInfo === "skipped") {
    return undefined
  }

  const vmLogs = tx.fields.vmLogs as string | undefined
  if (!vmLogs) {
    return undefined
  }

  const contract = contracts.get(tx.address?.toString() ?? "")
  const codeCell = contract?.stateInit?.code
  if (!codeCell) {
    return undefined
  }

  return extractCodeAndTrace(codeCell, vmLogs)
}

export async function traceTx(link: string): Promise<RetraceResultAndCode> {
  const {result, network} = await doTrace(link)
  const {code, traceInfo, exitCode} = extractCodeAndTrace(result.codeCell, result.emulatedTx.vmLogs)
  return {result, code, trace: traceInfo, exitCode, network}
}

export function normalizeGas(step: Step) {
  if (step.gasCost > 5000) {
    return 26
  }
  return step.gasCost
}
