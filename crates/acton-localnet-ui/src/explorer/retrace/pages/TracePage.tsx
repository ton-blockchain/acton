import React, {Suspense, useCallback, useEffect, useMemo, useRef, useState, lazy} from "react"

import {type StackElement} from "ton-assembly/dist/trace"

import {Tooltip, useToast} from "@acton/shared-ui"

import {useNetworkInfo} from "../../hooks/useNetworkInfo"
import type {RetraceResultAndCode} from "@retrace/txTrace/lib/types"
import {RetraceResultView} from "@retrace/txTrace/ui/RetraceResultView"
import TraceSidePanel from "@retrace/txTrace/ui/TraceSidePanel"
import {traceTx} from "@retrace/txTrace/lib/traceTx"
import {
  buildInstructionDetails,
  calculateCumulativeGasSinceBegin,
  getImplicitRet,
  getStoredTraceViewMode,
  getTraceStatusModel,
  setStoredTraceViewMode,
  type TraceViewMode,
} from "@retrace/txTrace/lib/traceViewModel"
import {useLineExecutionData, useTraceStepper} from "@retrace/txTrace/hooks"
import InlineLoader from "@retrace/txTrace/ui/InlineLoader"
import StatusBadge from "@retrace/txTrace/ui/StatusBadge"

import StackItemDetails from "@retrace/txTrace/ui/stack/StackItemDetails"
import TraceStepsChainView from "@retrace/txTrace/ui/TraceStepsChainView"
import TraceViewModeToggle from "@retrace/txTrace/ui/TraceViewModeToggle"

import styles from "./TracePage.module.css"

const CodeEditor = lazy(() => import("@retrace/ui/CodeEditor"))

interface TracePageProps {
  readonly initialTx?: string
}

function TracePage({initialTx}: TracePageProps) {
  const [result, setResult] = useState<RetraceResultAndCode | undefined>(undefined)
  const [loading, setLoading] = useState(false)
  const [detailsExpanded, setDetailsExpanded] = useState(false)
  const {showToast} = useToast()
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

  const instructionDetails = useMemo(() => buildInstructionDetails(result), [result])
  const cumulativeGasSinceBegin = useMemo(
    () => calculateCumulativeGasSinceBegin(result?.trace, selectedStep),
    [result?.trace, selectedStep],
  )

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

  const implicitRet = useMemo(
    () => getImplicitRet(result?.trace, selectedStep),
    [result?.trace, selectedStep],
  )
  const {exitCode, txStatus, stateUpdateHashOk, shouldShowStatusContainer, txStatusText} =
    useMemo(() => getTraceStatusModel(result), [result])

  return (
    <>
      {!result && (
        <main className={styles.inputPage}>
          <div id="trace-status" className={styles.srOnly} aria-live="polite" aria-atomic="true">
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
          <div
            id="trace-results-status"
            className={styles.srOnly}
            aria-live="polite"
            aria-atomic="true"
          >
            {loading && "Loading new transaction trace..."}
            {result && !loading && "Transaction trace loaded successfully"}
          </div>

          <div className={styles.traceToolbar}>
            <TraceViewModeToggle value={traceViewMode} onChange={handleTraceViewModeChange} />

            <div className={styles.headerContent}>
              {shouldShowStatusContainer && (
                <div className={styles.txStatusContainer} role="status" aria-live="polite">
                  {txStatus && (
                    <StatusBadge type={txStatus} text={txStatusText} exitCode={exitCode} />
                  )}
                  {stateUpdateHashOk === false && (
                    <Tooltip
                      content={
                        "Because the transaction runs in a local sandbox, we can't always reproduce it exactly. Sandbox replay was incomplete, and some values may differ from those on the real blockchain."
                      }
                      placement="bottom"
                    >
                      <StatusBadge type="warning" text="Trace Incomplete" />
                    </Tooltip>
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
                <h2 id="code-viewer-heading" className={styles.srOnly}>
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
