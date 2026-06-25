import {parse as parseVmLogs} from "ton-assembly/dist/logs"
import type {TraceInfo} from "ton-assembly/dist/trace"

import {normalizeGas} from "./traceTx"
import type {InstructionDetail, RetraceResultAndCode} from "./types"

export type TraceViewMode = "assembler" | "stepsChain"

const TRACE_VIEW_MODE_STORAGE_KEY = "txtracer-trace-view-mode"
const SOURCE_DEBUG_PANEL_WIDTH_STORAGE_KEY = "txtracer-source-debug-panel-width"
const SOURCE_DEBUG_SECTION_HEIGHTS_STORAGE_KEY = "txtracer-source-debug-section-heights"
const SOURCE_DEBUG_COLLAPSED_SECTIONS_STORAGE_KEY = "txtracer-source-debug-collapsed-sections"

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

export function getStoredSourceDebugSectionHeights<T extends string>(
  defaultHeights: Record<T, number>,
  minHeight: number,
): Record<T, number> {
  const stored = localStorage.getItem(SOURCE_DEBUG_SECTION_HEIGHTS_STORAGE_KEY)
  if (!stored) {
    return defaultHeights
  }

  try {
    const parsed = JSON.parse(stored) as Partial<Record<T, unknown>>
    return Object.fromEntries(
      (Object.entries(defaultHeights) as [T, number][]).map(([sectionId, defaultHeight]) => {
        const storedHeight = Number(parsed[sectionId])
        return [
          sectionId,
          Number.isFinite(storedHeight) && storedHeight >= minHeight
            ? Math.round(storedHeight)
            : defaultHeight,
        ]
      }),
    ) as Record<T, number>
  } catch {
    return defaultHeights
  }
}

export function setStoredSourceDebugSectionHeights<T extends string>(
  heights: Record<T, number>,
  minHeight: number,
): void {
  const storedHeights = Object.fromEntries(
    (Object.entries(heights) as [T, number][]).map(([sectionId, height]) => [
      sectionId,
      Math.max(minHeight, Math.round(height)),
    ]),
  )
  localStorage.setItem(SOURCE_DEBUG_SECTION_HEIGHTS_STORAGE_KEY, JSON.stringify(storedHeights))
}

export function getStoredSourceDebugCollapsedSections<T extends string>(
  defaultCollapsed: Record<T, boolean>,
): Record<T, boolean> {
  const stored = localStorage.getItem(SOURCE_DEBUG_COLLAPSED_SECTIONS_STORAGE_KEY)
  if (!stored) {
    return defaultCollapsed
  }

  try {
    const parsed = JSON.parse(stored) as Partial<Record<T, unknown>>
    return Object.fromEntries(
      (Object.entries(defaultCollapsed) as [T, boolean][]).map(([sectionId, defaultValue]) => [
        sectionId,
        typeof parsed[sectionId] === "boolean" ? parsed[sectionId] : defaultValue,
      ]),
    ) as Record<T, boolean>
  } catch {
    return defaultCollapsed
  }
}

export function setStoredSourceDebugCollapsedSections<T extends string>(
  collapsedSections: Record<T, boolean>,
): void {
  localStorage.setItem(
    SOURCE_DEBUG_COLLAPSED_SECTIONS_STORAGE_KEY,
    JSON.stringify(collapsedSections),
  )
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
