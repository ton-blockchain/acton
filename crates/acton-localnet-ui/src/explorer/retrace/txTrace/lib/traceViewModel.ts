import {parse as parseVmLogs} from "ton-assembly/dist/logs"
import type {TraceInfo} from "ton-assembly/dist/trace"

import {normalizeGas} from "./traceTx"
import type {InstructionDetail, RetraceResultAndCode} from "./types"

export type TraceViewMode = "assembler" | "stepsChain"

const TRACE_VIEW_MODE_STORAGE_KEY = "txtracer-trace-view-mode"
const SOURCE_DEBUG_PANEL_WIDTH_STORAGE_KEY = "txtracer-source-debug-panel-width"

function isTraceViewMode(value: string | null): value is TraceViewMode {
  return value === "assembler" || value === "stepsChain"
}

export function getStoredTraceViewMode(): TraceViewMode {
  const stored = localStorage.getItem(TRACE_VIEW_MODE_STORAGE_KEY)
  return isTraceViewMode(stored) ? stored : "assembler"
}

export function setStoredTraceViewMode(mode: TraceViewMode): void {
  localStorage.setItem(TRACE_VIEW_MODE_STORAGE_KEY, mode)
}

export function getStoredSourceDebugPanelWidth(defaultWidth: number): number {
  const stored = Number(localStorage.getItem(SOURCE_DEBUG_PANEL_WIDTH_STORAGE_KEY))
  return Number.isFinite(stored) && stored > 0 ? stored : defaultWidth
}

export function setStoredSourceDebugPanelWidth(width: number): void {
  localStorage.setItem(SOURCE_DEBUG_PANEL_WIDTH_STORAGE_KEY, Math.round(width).toString())
}

function extractFirstTransactionInstructions(vmLogs: string): readonly string[] {
  let parsedLines: ReturnType<typeof parseVmLogs>
  try {
    parsedLines = parseVmLogs(vmLogs)
  } catch {
    return []
  }

  const transactions: string[][] = []
  let currentTransactionInstructions: string[] = []

  for (const line of parsedLines) {
    if (line.$ === "VmExecute") {
      currentTransactionInstructions.push(line.instr.trim())
      continue
    }

    if (
      line.$ === "VmUnknown" &&
      line.text.includes("console.log") &&
      currentTransactionInstructions.length > 0
    ) {
      transactions.push(currentTransactionInstructions)
      currentTransactionInstructions = []
    }
  }

  if (currentTransactionInstructions.length > 0) {
    transactions.push(currentTransactionInstructions)
  }

  return transactions[0] ?? []
}

export function buildInstructionDetails(
  result: RetraceResultAndCode | undefined,
): readonly InstructionDetail[] {
  if (!result?.trace) {
    return []
  }

  const vmInstructions = result.result.emulatedTx.vmLogs
    ? extractFirstTransactionInstructions(result.result.emulatedTx.vmLogs)
    : []

  return result.trace.steps.map((step, index) => ({
    name: step.instructionName,
    gasCost: normalizeGas(step),
    instructionText: vmInstructions[index],
  }))
}

export function calculateCumulativeGasSinceBegin(
  trace: TraceInfo | undefined,
  selectedStep: number,
): number {
  if (!trace?.steps || selectedStep <= 0) {
    return 0
  }

  let totalGas = 0
  for (let index = 0; index < selectedStep; index++) {
    const step = trace.steps[index]
    if (step) {
      const gas = normalizeGas(step)
      if (!Number.isNaN(gas)) {
        totalGas += gas
      }
    }
  }

  return totalGas
}

export function getImplicitRet(
  trace: TraceInfo | undefined,
  selectedStep: number,
): {readonly line: number | undefined; readonly approx: boolean} {
  const steps = trace?.steps
  if (!steps) {
    return {line: undefined, approx: false}
  }

  const current = steps[selectedStep]
  if (!current || current.loc !== undefined) {
    return {line: undefined, approx: false}
  }

  let index = selectedStep - 1
  let chainLength = 1
  while (index >= 0 && steps[index]?.loc === undefined) {
    chainLength++
    index--
  }

  const anchor = index >= 0 ? steps[index] : undefined
  const line = anchor?.loc?.line === undefined ? undefined : anchor.loc.line + 1
  return {line, approx: chainLength > 1}
}

export function getTraceStatusModel(result: RetraceResultAndCode | undefined): {
  readonly exitCode: number | undefined
  readonly txStatus: "success" | "failed" | undefined
  readonly stateUpdateHashOk: boolean | undefined
  readonly shouldShowStatusContainer: boolean
  readonly txStatusText: string
} {
  const exitCode =
    result?.result?.emulatedTx?.computeInfo === "skipped"
      ? undefined
      : result?.result?.emulatedTx?.computeInfo?.exitCode
  const txStatus =
    result?.result?.emulatedTx?.computeInfo === "skipped"
      ? "failed"
      : result?.result?.emulatedTx?.computeInfo?.success && (exitCode === 0 || exitCode === 1)
        ? "success"
        : result
          ? "failed"
          : undefined

  const stateUpdateHashOk = result?.result?.stateUpdateHashOk

  return {
    exitCode,
    txStatus,
    stateUpdateHashOk,
    shouldShowStatusContainer: txStatus !== undefined || stateUpdateHashOk === false,
    txStatusText: `Exit code: ${exitCode?.toString() ?? "unknown"}`,
  }
}
