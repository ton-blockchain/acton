import React, {Suspense, useCallback, useEffect, useRef, useState, lazy} from "react"
import {FiCode, FiList} from "react-icons/fi"

import {parse as parseVmLogs} from "ton-assembly/dist/logs"
import {type StackElement} from "ton-assembly/dist/trace"

import {useToast} from "@acton/shared-ui"

import {useNetworkInfo} from "../../hooks/useNetworkInfo"
import type {RetraceResultAndCode} from "@retrace/txTrace/ui"
import {RetraceResultView} from "@retrace/txTrace/ui"
import TraceSidePanel from "@retrace/ui/TraceSidePanel"
import {normalizeGas, traceTx} from "@retrace/txTrace/lib/traceTx"
import {useLineExecutionData, useTraceStepper} from "@retrace/txTrace/hooks"
import InlineLoader from "@retrace/ui/InlineLoader"
import {type InstructionDetail} from "@retrace/txTrace/ui/StepInstructionBlock"
import StatusBadge from "@retrace/ui/StatusBadge"

import {TooltipHint} from "@retrace/ui/TooltipHint"

import StackItemDetails from "@retrace/ui/StackItemDetails"
import {TraceStepsChainView} from "@retrace/txTrace/ui"

import styles from "./TracePage.module.css"

const CodeEditor = lazy(() => import("@retrace/ui/CodeEditor"))

type TraceViewMode = "assembler" | "stepsChain"

const TRACE_VIEW_OPTIONS: ReadonlyArray<{
  readonly value: TraceViewMode
  readonly label: string
  readonly icon: typeof FiCode
}> = [
  {value: "assembler", label: "Assembler", icon: FiCode},
  {value: "stepsChain", label: "Steps chain", icon: FiList},
]

const TRACE_VIEW_MODE_STORAGE_KEY = "txtracer-trace-view-mode"

function isTraceViewMode(value: string | null): value is TraceViewMode {
  return value === "assembler" || value === "stepsChain"
}

function getStoredTraceViewMode(): TraceViewMode {
  const stored = localStorage.getItem(TRACE_VIEW_MODE_STORAGE_KEY)
  return isTraceViewMode(stored) ? stored : "assembler"
}

function setStoredTraceViewMode(mode: TraceViewMode): void {
  localStorage.setItem(TRACE_VIEW_MODE_STORAGE_KEY, mode)
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

    if (line.$ === "VmUnknown" && line.text.includes("console.log") && currentTransactionInstructions.length > 0) {
        transactions.push(currentTransactionInstructions)
        currentTransactionInstructions = []
      }
  }

  if (currentTransactionInstructions.length > 0) {
    transactions.push(currentTransactionInstructions)
  }

  return transactions[0] ?? []
}

interface TracePageProps {
  readonly initialTx?: string
}

function TracePage({initialTx}: TracePageProps) {
  const [result, setResult] = useState<RetraceResultAndCode | undefined>(undefined)
  const [loading, setLoading] = useState(false)
  const [detailsExpanded, setDetailsExpanded] = useState(false)
  const {showToast} = useToast()
  const [instructionDetails, setInstructionDetails] = useState<InstructionDetail[]>([])
  const [cumulativeGasSinceBegin, setCumulativeGasSinceBegin] = useState<number>(0)
  const [selectedStackItem, setSelectedStackItem] = useState<{
    element: StackElement
    title: string
  } | null>(null)
  const [traceViewMode, setTraceViewMode] = useState<TraceViewMode>(() => getStoredTraceViewMode())
  const autoSubmittedTxRef = useRef<string | undefined>(undefined)
  const {network} = useNetworkInfo()

  const lineExecutionData = useLineExecutionData(result?.trace)
  const {
    selectedStep,
    highlightLine,
    currentStep,
    currentStack,
    goToStep,
    handlePrev,
    handleNext,
    goToFirstStep,
    goToLastStep,
    canGoPrev,
    canGoNext,
    findStepByLine,
    transitionType,
    totalSteps,
  } = useTraceStepper(result?.trace)

  useEffect(() => {
    if (result?.trace) {
      const vmInstructions = result.result.emulatedTx.vmLogs
        ? extractFirstTransactionInstructions(result.result.emulatedTx.vmLogs)
        : []

      setInstructionDetails(
        result.trace.steps.map((step, index) => ({
          name: step.instructionName,
          gasCost: normalizeGas(step),
          instructionText: vmInstructions[index],
        })),
      )
    } else {
      setInstructionDetails([])
    }
  }, [result])

  useEffect(() => {
    if (result?.trace?.steps && selectedStep > 0) {
      let totalGas = 0
      for (let i = 0; i < selectedStep; i++) {
        const step = result.trace.steps[i]
        if (step) {
          const gasNum = normalizeGas(step)
          if (!Number.isNaN(gasNum)) {
            totalGas += gasNum
          }
        }
      }
      setCumulativeGasSinceBegin(totalGas)
    } else {
      setCumulativeGasSinceBegin(0)
    }
  }, [selectedStep, result?.trace?.steps])

  const traceTransaction = useCallback(
    async (txHash: string) => {
      const textToSubmit = txHash.trim()
      if (!textToSubmit) return
      setLoading(true)
      try {
        const rr = await traceTx(textToSubmit, network)
        setResult(rr)
        setSelectedStackItem(null)
      } catch (e) {
        console.error(e)
        showToast({
          title: "Failed to trace transaction",
          description: e instanceof Error ? e.message : "Failed to trace transaction",
          variant: "error",
        })
      } finally {
        setLoading(false)
      }
    },
    [network, showToast],
  )

  useEffect(() => {
    const tx = initialTx ?? ""
    if (!tx) {
      autoSubmittedTxRef.current = undefined
      setResult(undefined)
      setSelectedStackItem(null)
      return
    }

    const autoSubmitKey = `${network.id}:${tx}`

    if (autoSubmittedTxRef.current === autoSubmitKey) {
      return
    }

    autoSubmittedTxRef.current = autoSubmitKey
    void traceTransaction(tx)
  }, [traceTransaction, initialTx, network.id])

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && selectedStackItem) {
        event.preventDefault()
        setSelectedStackItem(null)
      }
    }

    document.addEventListener("keydown", handleKeyDown)
    return () => {
      document.removeEventListener("keydown", handleKeyDown)
    }
  }, [selectedStackItem])

  const toggleDetails = useCallback(() => setDetailsExpanded(prev => !prev), [])

  const handleDetailsKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLDivElement>) => {
      if (event.key === "Enter" || event.key === " ") {
        toggleDetails()
      }
    },
    [toggleDetails],
  )

  const handleStackItemClick = useCallback(
    (element: StackElement, title: string) => {
      if (
        selectedStackItem &&
        selectedStackItem.element === element &&
        selectedStackItem.title === title
      ) {
        setSelectedStackItem(null)
      } else {
        setSelectedStackItem({element, title})
      }
    },
    [selectedStackItem],
  )

  const handleBackToCode = useCallback(() => {
    setSelectedStackItem(null)
  }, [])

  const handleTraceViewModeChange = useCallback((mode: TraceViewMode) => {
    setTraceViewMode(mode)
    setStoredTraceViewMode(mode)
  }, [])

  const implicitRet = (() => {
    const steps = result?.trace?.steps
    if (!steps) return {line: undefined as number | undefined, approx: false}
    const current = steps[selectedStep]
    if (!current || current.loc !== undefined)
      return {line: undefined as number | undefined, approx: false}

    let idx = selectedStep - 1
    let chainLen = 1
    while (idx >= 0 && steps[idx]?.loc === undefined) {
      chainLen++
      idx--
    }
    const anchor = idx >= 0 ? steps[idx] : undefined
    const line = anchor?.loc?.line === undefined ? undefined : anchor.loc.line + 1
    const approx = chainLen > 1
    return {line, approx}
  })()

  const exitCode =
    result?.result?.emulatedTx?.computeInfo === "skipped"
      ? undefined
      : result?.result?.emulatedTx?.computeInfo?.exitCode
  const txStatus =
    result?.result?.emulatedTx?.computeInfo === "skipped"
      ? "failed"
      : result?.result?.emulatedTx?.computeInfo?.success && (exitCode === 0 || exitCode === 1)
        ? "success"
        : "failed"

  const stateUpdateHashOk = result?.result?.stateUpdateHashOk
  const shouldShowStatusContainer = txStatus !== undefined || stateUpdateHashOk === false
  const txStatusText = `Exit code: ${exitCode?.toString() ?? "unknown"}`

  return (
    <>
      {!result && (
        <main className={styles.inputPage}>
          <div id="trace-status" className="sr-only" aria-live="polite" aria-atomic="true">
            {loading && "Tracing transaction..."}
            {result && !loading && "Transaction traced successfully"}
          </div>

          <div className={styles.centeredInputContainer}>
            {loading ? (
              <InlineLoader
                message="Tracing transaction"
                subtext="This may take a few moments"
                loading={loading}
              />
            ) : (
              <div className={styles.emptyTraceState}>No transaction hash</div>
            )}
          </div>
        </main>
      )}

      {result && (
        <div className={styles.traceViewWrapper}>
          <div id="trace-results-status" className="sr-only" aria-live="polite" aria-atomic="true">
            {loading && "Loading new transaction trace..."}
            {result && !loading && "Transaction trace loaded successfully"}
          </div>

          <div className={styles.traceToolbar}>
            <div className={styles.viewModeControl}>
              <div className={styles.traceViewToggle} role="group" aria-label="Trace view mode">
                {TRACE_VIEW_OPTIONS.map(option => {
                  const isActive = traceViewMode === option.value
                  const Icon = option.icon

                  return (
                    <button
                      key={option.value}
                      type="button"
                      className={`${styles.traceViewToggleButton} ${isActive ? styles.traceViewToggleButtonActive : ""}`}
                      onClick={() => {
                        handleTraceViewModeChange(option.value)
                      }}
                      aria-pressed={isActive}
                    >
                      <Icon className={styles.traceViewToggleIcon} aria-hidden="true" />
                      <span>{option.label}</span>
                    </button>
                  )
                })}
              </div>
            </div>

            <div className={styles.headerContent}>
              {shouldShowStatusContainer && (
                <div className={styles.txStatusContainer} role="status" aria-live="polite">
                  {txStatus && (
                    <StatusBadge type={txStatus} text={txStatusText} exitCode={exitCode} />
                  )}
                  {stateUpdateHashOk === false && (
                    <TooltipHint
                      tooltipText={
                        "Because the transaction runs in a local sandbox, we can't always reproduce it exactly. Sandbox replay was incomplete, and some values may differ from those on the real blockchain."
                      }
                      placement="bottom"
                    >
                      <StatusBadge type="warning" text="Trace Incomplete" />
                    </TooltipHint>
                  )}
                </div>
              )}
            </div>
          </div>

          <main className={styles.appContainer}>
            <div
              className={`${styles.mainContent} ${detailsExpanded ? styles.mainContentMinimized : ""}`}
            >
              <section
                aria-labelledby="code-viewer-heading"
                data-testid="code-editor-container"
                className={styles.codeEditorArea}
              >
                <h2 id="code-viewer-heading" className="sr-only">
                  Transaction Code Viewer
                </h2>
                <div
                  className={`${styles.codeEditorWrapper} ${selectedStackItem ? styles.codeEditorHidden : ""}`}
                >
                  {traceViewMode === "assembler" ? (
                    <Suspense
                      fallback={<InlineLoader message="Loading Editor..." loading={true} />}
                    >
                      <CodeEditor
                        code={result.code}
                        highlightLine={highlightLine}
                        implicitRetLine={implicitRet.line}
                        implicitRetLabel={
                          implicitRet.approx ? "↵ implicit RET (approximate position)" : undefined
                        }
                        lineExecutionData={lineExecutionData}
                        onLineClick={findStepByLine}
                        shouldCenter={transitionType === "button"}
                        exitCode={result.exitCode}
                      />
                    </Suspense>
                  ) : (
                    <TraceStepsChainView
                      steps={instructionDetails}
                      selectedStep={selectedStep}
                      onStepClick={goToStep}
                    />
                  )}
                </div>

                {selectedStackItem && (
                  <div className={styles.stackItemOverlay}>
                    <StackItemDetails
                      itemData={selectedStackItem.element}
                      title={selectedStackItem.title}
                      onClose={handleBackToCode}
                    />
                  </div>
                )}
              </section>
              <TraceSidePanel
                selectedStep={selectedStep}
                totalSteps={totalSteps}
                currentStep={currentStep}
                currentStack={currentStack}
                canGoPrev={canGoPrev}
                canGoNext={canGoNext}
                onPrev={handlePrev}
                onNext={handleNext}
                onFirst={goToFirstStep}
                onLast={goToLastStep}
                showGas={true}
                placeholderMessage="No trace steps available."
                instructionDetails={instructionDetails}
                cumulativeGas={cumulativeGasSinceBegin}
                onStackItemClick={handleStackItemClick}
                className={styles.sidebarArea}
              />
            </div>
            <section
              className={`${styles.detailsSection} ${detailsExpanded ? styles.detailsSectionExpanded : ""}`}
              aria-labelledby="transaction-details-heading"
            >
              <div
                data-testid="details-header"
                className={styles.detailsHeader}
                onClick={toggleDetails}
                onKeyDown={handleDetailsKeyDown}
                role="button"
                tabIndex={0}
                aria-expanded={detailsExpanded}
                aria-controls="transaction-details-content"
              >
                <div className={styles.detailsTitle}>
                  <span id="transaction-details-heading">TRANSACTION DETAILS</span>
                </div>
              </div>
              {detailsExpanded && (
                <div
                  id="transaction-details-content"
                  data-testid="details-content"
                  className={styles.detailsContent}
                >
                  <div className={styles.transactionDetailsPanelInTracePage}>
                    <RetraceResultView result={result} />
                  </div>
                </div>
              )}
            </section>
          </main>
          {loading && result && (
            <div className={styles.loadingOverlay} role="status" aria-live="polite">
              <InlineLoader
                message="Tracing new transaction..."
                subtext="This may take a few moments"
                loading={true}
              />
            </div>
          )}
        </div>
      )}
    </>
  )
}

export default TracePage
