import React from "react"

import type {StackElement} from "ton-assembly/dist/trace"

import Button from "@retrace/ui/Button"
import StackViewer from "@retrace/ui/StackViewer"
import StackEditor from "@retrace/ui/StackEditor"
import {
  StepInstructionBlock,
  type InstructionDetail,
} from "@retrace/txTrace/ui/StepInstructionBlock"

import styles from "./TraceSidePanel.module.css"

export interface TraceSidePanelProps {
  readonly selectedStep?: number
  readonly totalSteps?: number
  readonly currentStep?: {
    instructionName: string
    gasCost?: number
    gasUsed?: number
  }
  readonly currentStack?: readonly StackElement[]

  readonly canGoPrev?: boolean
  readonly canGoNext?: boolean
  readonly onPrev?: () => void
  readonly onNext?: () => void
  readonly onFirst?: () => void
  readonly onLast?: () => void

  readonly instructionDetails?: InstructionDetail[]
  readonly cumulativeGas?: number
  readonly showGas?: boolean

  readonly placeholderMessage?: string
  readonly statusMessage?: string

  readonly showStackSetup?: boolean
  readonly onSetupStack?: () => void

  readonly initialStack?: StackElement[]
  readonly onInitialStackChange?: (stack: StackElement[]) => void
  readonly hasExecutionResults?: boolean
  readonly onStackItemClick?: (element: StackElement, title: string) => void
  readonly className?: string
}

const TraceSidePanel: React.FC<TraceSidePanelProps> = ({
  selectedStep = 0,
  totalSteps = 0,
  currentStep,
  currentStack = [],
  canGoPrev = false,
  canGoNext = false,
  onPrev = () => {},
  onNext = () => {},
  onFirst = () => {},
  onLast = () => {},
  instructionDetails = [],
  cumulativeGas,
  showGas = false,
  placeholderMessage,
  statusMessage,
  showStackSetup = false,
  initialStack = [],
  onInitialStackChange = () => {},
  hasExecutionResults = false,
  onStackItemClick,
  className,
}) => {
  const hasData = totalSteps > 0 && currentStep
  const showInitialStackEditor = showStackSetup && !hasExecutionResults

  return (
    <div className={`${styles.sidePanel} ${className || ""}`}>
      <div className={styles.stepDetails}>
        <div className={styles.stepHeader}>
          <div className={styles.stepHeaderTop}>
            <span className={styles.stepCounter} data-testid="step-counter-info">
              {hasData
                ? `Step ${selectedStep + 1} of ${totalSteps}`
                : placeholderMessage || "Ready"}
            </span>
            {showGas && (
              <span className={styles.cumulativeGasCounter} data-testid="cumulative-gas-counter">
                Used gas: {cumulativeGas}
              </span>
            )}
            {statusMessage && <span className={styles.statusMessage}>{statusMessage}</span>}
          </div>

          {/* StepInstructionBlock for TracePage */}
          {instructionDetails.length > 0 && (
            <StepInstructionBlock
              steps={instructionDetails}
              currentIndex={selectedStep}
              itemHeight={32}
            />
          )}

          {/* Simple instruction block */}
          {instructionDetails.length === 0 && (
            <div className={styles.stepInstructionBlock}>
              <span className={styles.stepInstruction}>
                {currentStep?.instructionName ||
                  placeholderMessage ||
                  "Click Execute to run assembly code"}
              </span>
              {currentStep?.gasCost && showGas && (
                <span className={styles.stepGas}>{currentStep.gasCost} gas</span>
              )}
              {currentStep?.gasUsed && currentStep.gasUsed > 0 && !showGas && (
                <span className={styles.stepGas}>{currentStep.gasUsed} gas</span>
              )}
            </div>
          )}

          <div className={`${styles.navigationControls} navigation-controls`}>
            <Button
              variant="ghost"
              onClick={onFirst}
              className={styles.navButton}
              disabled={!canGoPrev || totalSteps === 0}
              title="Go to First Step"
              data-testid="go-to-first-step-button"
            >
              First
            </Button>
            <Button
              variant="ghost"
              onClick={onPrev}
              className={styles.navButton}
              disabled={!canGoPrev || totalSteps === 0}
              title="Previous Step"
              data-testid="prev-step-button"
            >
              Prev
            </Button>
            <Button
              variant="ghost"
              onClick={onNext}
              className={styles.navButton}
              disabled={!canGoNext || totalSteps === 0}
              title="Next Step"
              data-testid="next-step-button"
            >
              Next
            </Button>
            <Button
              variant="ghost"
              onClick={onLast}
              className={styles.navButton}
              disabled={!canGoNext || totalSteps === 0}
              title="Go to Last Step"
              data-testid="go-to-last-step-button"
            >
              Last
            </Button>
          </div>
        </div>

        <div className={`${styles.stackViewerContainer} stack-viewer`}>
          {showInitialStackEditor ? (
            <StackEditor stack={initialStack} onStackChange={onInitialStackChange} />
          ) : (
            <>
              <div className={styles.stackHeader}>
                <span>Stack</span>
              </div>
              <StackViewer stack={currentStack} title="" onStackItemClick={onStackItemClick} />
            </>
          )}
        </div>
      </div>
    </div>
  )
}

export default TraceSidePanel
