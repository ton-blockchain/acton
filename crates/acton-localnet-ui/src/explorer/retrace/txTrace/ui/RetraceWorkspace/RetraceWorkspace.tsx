import {lazy, memo, Suspense, useCallback, useMemo, useState} from "react"

import type {StackElement} from "ton-assembly/dist/trace"

import {Tooltip} from "@acton/shared-ui"

import {useLineExecutionData, useTraceStepper} from "../../hooks"
import type {RetraceResultAndCode} from "../../lib/types"
import {
  buildInstructionDetails,
  calculateCumulativeGasSinceBegin,
  getImplicitRet,
  getStoredTraceViewMode,
  setStoredTraceViewMode,
  type TraceViewMode,
} from "../../lib/traceViewModel"
import InlineLoader from "../InlineLoader"
import StatusBadge from "../StatusBadge"
import TraceSidePanel from "../TraceSidePanel"
import TraceStepsChainView from "../TraceStepsChainView"
import TraceViewModeToggle from "../TraceViewModeToggle"
import StackItemDetails from "../stack/StackItemDetails"

import styles from "./RetraceWorkspace.module.css"

const CodeEditor = lazy(() => import("../../../ui/CodeEditor"))

interface RetraceWorkspaceProps {
  readonly result: RetraceResultAndCode
  readonly className?: string
}

function RetraceWorkspaceFc({result, className}: RetraceWorkspaceProps) {
  const [selectedStackItem, setSelectedStackItem] = useState<{
    element: StackElement
    title: string
  } | null>(null)
  const [traceViewMode, setTraceViewMode] = useState<TraceViewMode>(() => getStoredTraceViewMode())

  const lineExecutionData = useLineExecutionData(result.trace)
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
  } = useTraceStepper(result.trace)

  const instructionDetails = useMemo(() => buildInstructionDetails(result), [result])
  const cumulativeGasSinceBegin = useMemo(
    () => calculateCumulativeGasSinceBegin(result.trace, selectedStep),
    [result.trace, selectedStep],
  )
  const implicitRet = useMemo(() => getImplicitRet(result.trace, selectedStep), [result.trace, selectedStep])
  const stateUpdateHashOk = result.result.stateUpdateHashOk

  const handleTraceViewModeChange = useCallback((mode: TraceViewMode) => {
    setTraceViewMode(mode)
    setStoredTraceViewMode(mode)
  }, [])

  const handleStackItemClick = useCallback(
    (element: StackElement, title: string) => {
      if (
        selectedStackItem &&
        selectedStackItem.element === element &&
        selectedStackItem.title === title
      ) {
        setSelectedStackItem(null)
        return
      }

      setSelectedStackItem({element, title})
    },
    [selectedStackItem],
  )

  return (
    <section className={`${styles.root} ${className ?? ""}`} aria-label="Transaction retrace">
      <div className={styles.toolbar}>
        <div className={styles.toolbarLeft}>
          <TraceViewModeToggle value={traceViewMode} onChange={handleTraceViewModeChange} />

          {stateUpdateHashOk === false && (
            <div className={styles.statusContainer} role="status" aria-live="polite">
              <Tooltip
                content={
                  "Because the transaction runs in a local sandbox, we can't always reproduce it exactly. Sandbox replay was incomplete, and some values may differ from those on the real blockchain."
                }
                placement="bottom"
              >
                <StatusBadge type="warning" text="Trace Incomplete" />
              </Tooltip>
            </div>
          )}
        </div>
      </div>

      <div className={styles.workspace}>
        <section className={styles.codeArea} aria-label="Trace code">
          <div
            className={`${styles.codeEditorWrapper} ${selectedStackItem ? styles.codeEditorHidden : ""}`}
          >
            {traceViewMode === "assembler" ? (
              <Suspense
                fallback={
                  <div className={styles.editorLoader}>
                    <InlineLoader message="Loading editor" loading={true} />
                  </div>
                }
              >
                <CodeEditor
                  code={result.code}
                  modelPath={`retrace-${result.result.emulatedTx.lt.toString()}.tasm`}
                  highlightLine={highlightLine}
                  implicitRetLine={implicitRet.line}
                  implicitRetLabel={
                    implicitRet.approx ? "implicit RET (approximate position)" : undefined
                  }
                  lineExecutionData={lineExecutionData}
                  onLineClick={findStepByLine}
                  shouldCenter={transitionType === "button"}
                  exitCode={result.exitCode}
                  needBorderRadius={false}
                  compactGutter
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
                onClose={() => setSelectedStackItem(null)}
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
          className={styles.sidePanel}
        />
      </div>
    </section>
  )
}

const RetraceWorkspace = memo(RetraceWorkspaceFc)
RetraceWorkspace.displayName = "RetraceWorkspace"

export default RetraceWorkspace
