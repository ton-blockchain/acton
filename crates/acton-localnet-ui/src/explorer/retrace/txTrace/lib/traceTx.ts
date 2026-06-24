import {RETRACE_MAINNET_NETWORK, RETRACE_TESTNET_NETWORK, retrace} from "@ton/retracer-core"
import type {RetraceNetworkConfig, TraceResult} from "@ton/retracer-core"
import {compileCellWithMapping, decompileCell} from "ton-assembly/dist/runtime/instr"
import {createMappingInfo} from "ton-assembly/dist/trace/mapping"
import {type Step, type TraceInfo} from "ton-assembly/dist/trace"
import {createTraceInfoPerTransaction, findInstructionInfo} from "ton-assembly/dist/trace/trace"
import {parse, print} from "ton-assembly/dist/text"
import * as l from "ton-assembly/dist/logs"
import {Cell} from "@ton/core"

import type {AssemblyMapping} from "ton-source-map"

import type {ExplorerNetworkInfo} from "../../../hooks/useNetworkInfo"
import type {ExitCode, RetraceResultAndCode} from "./types"

import {
  NetworkError,
  TooManyRequests,
  TxHashInvalidError,
  TxNotFoundError,
  TxTraceError,
} from "./errors"

function absoluteApiBaseUrl(baseUrl: string): string {
  const fullBase = baseUrl.startsWith("http")
    ? baseUrl
    : `${globalThis.location.origin}${baseUrl}`
  return new URL(fullBase).toString().replace(/\/$/, "")
}

function getRetraceNetworkConfig(network: ExplorerNetworkInfo): RetraceNetworkConfig {
  if (network.api) {
    return {
      testnet: network.testOnly,
      v2BaseUrl: absoluteApiBaseUrl(network.api.v2BaseUrl),
      v3BaseUrl: absoluteApiBaseUrl(network.api.v3BaseUrl),
      toncenterApiKey: network.api.toncenterApiKey,
    }
  }

  if (network.id === "mainnet") {
    return RETRACE_MAINNET_NETWORK
  }
  if (network.id === "testnet") {
    return RETRACE_TESTNET_NETWORK
  }

  throw new TxTraceError(`Retrace is not configured for ${network.label}.`)
}

async function doTrace(
  hash: string,
  network: ExplorerNetworkInfo,
): Promise<{readonly result: TraceResult; readonly network: ExplorerNetworkInfo}> {
  try {
    const result = await retrace(getRetraceNetworkConfig(network), hash.toLowerCase())
    return {result, network}
  } catch (e: unknown) {
    let message = "An unknown error occurred."
    if (e instanceof Error) {
      message = e.message
    } else if (e !== null && e !== undefined) {
      // eslint-disable-next-line @typescript-eslint/no-base-to-string
      message = String(e)
    }

    if (/status code 429|HTTP 429|\(429\)/i.test(message)) {
      throw new TooManyRequests(undefined, e)
    }

    if (/status code 422|HTTP 422|\(422\)/i.test(message)) {
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

export async function traceTx(
  hash: string,
  network: ExplorerNetworkInfo,
): Promise<RetraceResultAndCode> {
  const {result} = await doTrace(hash, network)
  const {code, traceInfo, exitCode} = extractCodeAndTrace(result.codeCell, result.emulatedTx.vmLogs)
  return {result, code, trace: traceInfo, exitCode, network}
}

export function normalizeGas(step: Step) {
  if (step.gasCost > 5000) {
    return 26
  }
  return step.gasCost
}
