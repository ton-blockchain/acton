import React, {Suspense, useCallback, useEffect, useRef, useState, lazy} from "react"
import {
  FiBook,
  FiClock,
  FiCode,
  FiCpu,
  FiGithub,
  FiList,
  FiMoreHorizontal,
  FiPlay,
  FiSearch,
  FiX,
} from "react-icons/fi"

import {parse as parseVmLogs} from "ton-assembly/dist/logs"
import {type StackElement} from "ton-assembly/dist/trace"

import type {RetraceResultAndCode} from "@retrace/txTrace/ui"
import {RetraceResultView} from "@retrace/txTrace/ui"
import TraceSidePanel from "@retrace/ui/TraceSidePanel"
import {normalizeGas, traceTx} from "@retrace/txTrace/lib/traceTx"
import {useLineExecutionData, useTraceStepper} from "@retrace/txTrace/hooks"
import SearchInput from "@retrace/ui/SearchInput"
import InlineLoader from "@retrace/ui/InlineLoader"
import {type InstructionDetail} from "@retrace/txTrace/ui/StepInstructionBlock"
import {type TxHistoryEntry, useTxHistory} from "@retrace/lib/useTxHistory"
import {shortenHash} from "@retrace/lib/format"
import StatusBadge, {type StatusType} from "@retrace/ui/StatusBadge"

import {TooltipHint} from "@retrace/ui/TooltipHint"
import Badge from "@retrace/ui/Badge"

import {StackItemViewer} from "@retrace/pages/StackItemViewer"
import TraceStepsChainView from "@retrace/pages/TraceStepsChainView"

import {useGlobalError} from "@retrace/lib/useGlobalError"

import {getRawQueryParam} from "@retrace/common/lib/query-params"

import styles from "./TracePage.module.css"

const CodeEditor = lazy(() => import("@retrace/ui/CodeEditor"))
const PageHeader = lazy(() => import("@retrace/ui/PageHeader"))

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
  const [inputText, setInputText] = useState("")
  const [headerInputText, setHeaderInputText] = useState("")
  const [result, setResult] = useState<RetraceResultAndCode | undefined>(undefined)
  const [loading, setLoading] = useState(false)
  const [detailsExpanded, setDetailsExpanded] = useState(false)
  const {setError} = useGlobalError()
  const [instructionDetails, setInstructionDetails] = useState<InstructionDetail[]>([])
  const [cumulativeGasSinceBegin, setCumulativeGasSinceBegin] = useState<number>(0)
  const {history, addToHistory, removeFromHistory} = useTxHistory()
  const [showHistoryDropdown, setShowHistoryDropdown] = useState(false)
  const [isInputFocused, setIsInputFocused] = useState(false)
  const [selectedStackItem, setSelectedStackItem] = useState<{
    element: StackElement
    title: string
  } | null>(null)
  const [traceViewMode, setTraceViewMode] = useState<TraceViewMode>(() => getStoredTraceViewMode())
  const autoSubmittedTxRef = useRef<string | undefined>(undefined)

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
    const tx = initialTx ?? getRawQueryParam("tx") ?? ""
    setInputText(tx)
    setHeaderInputText(tx)
  }, [initialTx])

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
          if (!isNaN(gasNum)) {
            totalGas += gasNum
          }
        }
      }
      setCumulativeGasSinceBegin(totalGas)
    } else {
      setCumulativeGasSinceBegin(0)
    }
  }, [selectedStep, result?.trace?.steps])

  const handleSubmit = useCallback(
    async (fromHeader: boolean = false, initialTx?: string) => {
      const textToSubmit = initialTx ?? (fromHeader ? headerInputText : inputText)
      if (!textToSubmit.trim()) return
      setLoading(true)
      try {
        const rr = await traceTx(textToSubmit)
        setResult(rr)
        setSelectedStackItem(null)
        if (!fromHeader) {
          const computeInfo = rr?.result?.emulatedTx?.computeInfo
          const exitCode = computeInfo === "skipped" ? undefined : computeInfo?.exitCode
          addToHistory({hash: textToSubmit, exitCode, testnet: rr.network === "testnet"})
        }
        setShowHistoryDropdown(false)
        window.history.pushState({}, "", `?tx=${encodeURIComponent(textToSubmit)}`)
      } catch (e) {
        console.error(e)
        if (e instanceof Error) {
          setError(`Failed to trace transaction: ${e.message}`)
        } else {
          setError("Failed to trace transaction")
        }
      } finally {
        setLoading(false)
      }
    },
    [headerInputText, inputText, setError, addToHistory],
  )

  useEffect(() => {
    const tx = initialTx ?? getRawQueryParam("tx") ?? ""
    if (!tx || autoSubmittedTxRef.current === tx) {
      return
    }

    autoSubmittedTxRef.current = tx
    void handleSubmit(false, tx)
  }, [handleSubmit, initialTx])

  const handleHeaderSubmit = useCallback(() => {
    void handleSubmit(true)
  }, [handleSubmit])

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key === "Enter") {
        event.preventDefault()
        if (!loading) {
          void handleSubmit(false)
        }
      } else if (event.key === "Escape" && selectedStackItem) {
        event.preventDefault()
        setSelectedStackItem(null)
      }
    }

    document.addEventListener("keydown", handleKeyDown)
    return () => {
      document.removeEventListener("keydown", handleKeyDown)
    }
  }, [handleSubmit, loading, selectedStackItem])

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

          <div className="sr-only">Press Ctrl+Enter or Cmd+Enter to trace the transaction</div>

          <div className={styles.externalLinksContainer}>
            <a
              href="https://github.com/ton-blockchain/txtracer"
              target="_blank"
              rel="noopener noreferrer"
              title="GitHub Repository"
              className={styles.iconLink}
              aria-label="View TxTracer source code on GitHub"
            >
              <FiGithub size={24} aria-hidden="true" />
            </a>
          </div>

          <div className={styles.centeredInputContainer}>
            <header className={styles.txtracerLogo}>
              <div className={styles.logoDiamond} aria-hidden="true"></div>
              <h1 data-testid="app-title" className={styles.txtracerLogoH1}>
                <span>TxTracer</span>
                <span className={styles.titleTon}>The Open Network</span>
              </h1>
            </header>

            <section aria-labelledby="search-heading" className={styles.inputCard}>
              <h2 id="search-heading" className="sr-only">
                Transaction Search
              </h2>
              <SearchInput
                value={inputText}
                onChange={setInputText}
                onSubmit={() => {
                  void handleSubmit(false)
                }}
                placeholder="Search by transaction hash or explorer link"
                loading={loading}
                autoFocus={true}
                onFocus={() => {
                  setIsInputFocused(true)
                }}
                onBlur={() => {
                  setIsInputFocused(false)
                  setTimeout(() => setShowHistoryDropdown(false), 100)
                }}
                onInputClick={() => {
                  if (isInputFocused) {
                    setShowHistoryDropdown(true)
                  }
                }}
              />
              {showHistoryDropdown && history.length > 0 && (
                <ul
                  className={styles.historyDropdown}
                  onMouseDown={e => e.preventDefault()}
                  role="listbox"
                  aria-label="Transaction history"
                >
                  {history.slice(0, Math.min(4, history.length)).map((entry: TxHistoryEntry) => {
                    const statusType: StatusType =
                      entry.exitCode === undefined || entry.exitCode === 0 ? "success" : "failed"
                    return (
                      <li
                        key={entry.hash}
                        onClick={() => {
                          setInputText(entry.hash)
                          setShowHistoryDropdown(false)
                          void handleSubmit(false, entry.hash)
                        }}
                        onKeyDown={e => {
                          if (e.key === "Enter" || e.key === " ") {
                            e.preventDefault()
                            setInputText(entry.hash)
                            setShowHistoryDropdown(false)
                            void handleSubmit(false, entry.hash)
                          }
                        }}
                        className={styles.historyItem}
                        role="option"
                        tabIndex={0}
                        aria-selected={false}
                      >
                        <FiClock size={16} className={styles.historyItemIcon} aria-hidden="true" />
                        <span className={styles.historyItemLeft}>
                          <span className={styles.historyItemText}>
                            {shortenHash(entry.hash, 16, 16)}
                          </span>
                          {entry.testnet && <Badge color="red">Testnet</Badge>}
                        </span>
                        <div className={styles.historyItemBadges}>
                          {entry.exitCode !== undefined && (
                            <StatusBadge
                              type={statusType}
                              exitCode={entry.exitCode}
                              text={`Exit code: ${entry.exitCode}`}
                            />
                          )}
                        </div>
                        <button
                          className={styles.historyItemDeleteButton}
                          onClick={e => {
                            e.stopPropagation()
                            removeFromHistory(entry.hash)
                          }}
                          title="Remove from history"
                          aria-label={`Remove transaction ${shortenHash(entry.hash, 8, 8)} from history`}
                        >
                          <FiX size={16} aria-hidden="true" />
                        </button>
                      </li>
                    )
                  })}
                </ul>
              )}
            </section>

            {loading ? (
              <InlineLoader
                message="Tracing transaction"
                subtext="This may take a few moments"
                loading={loading}
              />
            ) : (
              <section aria-labelledby="features-heading" className={styles.featureCards}>
                <h2 id="features-heading" className="sr-only">
                  Available Tools
                </h2>
                <a href="/play/" className={styles.featureCard}>
                  <div className={`${styles.featureCardIcon} ${styles.playgroundIcon}`}>
                    <FiPlay aria-hidden="true" />
                  </div>
                  <h3 className={styles.featureCardTitle}>Playground</h3>
                  <p className={styles.featureCardDescription}>
                    Experiment with TVM assembly and FunC code directly in your browser. Write,
                    test, and debug assembly instructions with real-time execution.
                  </p>
                  <span className={styles.featureCardBadge}>Play and Learn</span>
                </a>

                <a href="/code-explorer/" className={styles.featureCard}>
                  <div className={`${styles.featureCardIcon} ${styles.explorerIcon}`}>
                    <FiSearch aria-hidden="true" />
                  </div>
                  <h3 className={styles.featureCardTitle}>Code Explorer</h3>
                  <p className={styles.featureCardDescription}>
                    Compile FunC or Tolk code to assembly and explore the generated bytecode.
                    Perfect for understanding how your smart contracts work under the hood.
                  </p>
                  <span className={styles.featureCardBadge}>Explore</span>
                </a>

                <a href="/spec/" className={styles.featureCard}>
                  <div className={`${styles.featureCardIcon} ${styles.specIcon}`}>
                    <FiBook aria-hidden="true" />
                  </div>
                  <h3 className={styles.featureCardTitle}>TVM Specification</h3>
                  <p className={styles.featureCardDescription}>
                    Browse the complete TVM instruction reference with detailed descriptions,
                    opcodes, stack effects, and control flow information for every instruction.
                  </p>
                  <span className={styles.featureCardBadge}>Reference</span>
                </a>

                <a href="/emulate/" className={styles.featureCard}>
                  <div className={`${styles.featureCardIcon} ${styles.emulateIcon}`}>
                    <FiCpu aria-hidden="true" />
                  </div>
                  <h3 className={styles.featureCardTitle}>Emulate</h3>
                  <p className={styles.featureCardDescription}>
                    Emulate raw messages on TON blockchain. Send single messages or batch multiple
                    messages together to see the full transaction tree and trace execution flow.
                  </p>
                  <span className={styles.featureCardBadge}>Emulator</span>
                </a>

                <div className={`${styles.featureCard} ${styles.placeholderCard}`}>
                  <div className={`${styles.featureCardIcon} ${styles.placeholderIcon}`}>
                    <FiMoreHorizontal aria-hidden="true" />
                  </div>
                  <h3 className={styles.featureCardTitle}>More Tools</h3>
                  <p className={styles.featureCardDescription}>
                    Additional developer tools and features are coming soon. Stay tuned for updates
                    to enhance your TON blockchain development experience.
                  </p>
                  <span className={styles.featureCardBadge}>Coming Soon</span>
                </div>
              </section>
            )}
          </div>

          <footer>
            <span className={styles.createBy}>
              Created by{" "}
              <a href="https://tonstudio.io" target="_blank" rel="noreferrer">
                TON Studio
              </a>
              {" and "}
              <a href="https://t.me/toncore" target="_blank" rel="noreferrer">
                TON Core
              </a>
              , powered by{" "}
              <a href="https://toncenter.com/" target="_blank" rel="noreferrer">
                TON Center
              </a>
            </span>
          </footer>
        </main>
      )}

      {result && (
        <div className={styles.traceViewWrapper}>
          <div id="trace-results-status" className="sr-only" aria-live="polite" aria-atomic="true">
            {loading && "Loading new transaction trace..."}
            {result && !loading && "Transaction trace loaded successfully"}
          </div>

          <PageHeader
            pageTitle={""}
            network={result?.network ?? "mainnet"}
            beforeLinks={
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
            }
          >
            <div className={styles.headerContent}>
              <div
                className={styles.searchInputContainer}
                role="search"
                aria-label="Search for another transaction"
              >
                <SearchInput
                  value={headerInputText}
                  onChange={setHeaderInputText}
                  onSubmit={handleHeaderSubmit}
                  placeholder="Trace another transaction hash"
                  loading={loading}
                  autoFocus={false}
                  compact={true}
                />
              </div>

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
          </PageHeader>

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
                    <StackItemViewer
                      element={selectedStackItem.element}
                      title={selectedStackItem.title}
                      onBack={handleBackToCode}
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
