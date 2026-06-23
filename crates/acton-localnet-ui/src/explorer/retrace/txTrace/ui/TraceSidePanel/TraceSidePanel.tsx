import React from "react"

import type {StackElement} from "ton-assembly/dist/trace"

import {Button} from "@acton/shared-ui"

import type {InstructionDetail} from "../../lib/types"
import {StepInstructionBlock} from "../StepInstructionBlock"
import StackViewer from "../stack/StackViewer"
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

  readonly instructionDetails?: readonly InstructionDetail[]
  readonly cumulativeGas?: number
  readonly showGas?: boolean

  readonly placeholderMessage?: string
  readonly statusMessage?: string

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
  onStackItemClick,
  className,
}) => {
  const hasData = totalSteps > 0 && Boolean(currentStep)

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

          {instructionDetails.length > 0 && (
            <StepInstructionBlock
              steps={instructionDetails}
              currentIndex={selectedStep}
              itemHeight={32}
            />
          )}

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

          <div className={styles.navigationControls}>
            <Button
              variant="ghost"
              size="sm"
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
              size="sm"
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
              size="sm"
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
              size="sm"
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

        <div className={styles.stackViewerContainer}>
          <div className={styles.stackHeader}>
            <span>Stack</span>
          </div>
          <StackViewer stack={currentStack} title="" onStackItemClick={onStackItemClick} />
        </div>
      </div>
    </div>
  )
}

export default TraceSidePanel
