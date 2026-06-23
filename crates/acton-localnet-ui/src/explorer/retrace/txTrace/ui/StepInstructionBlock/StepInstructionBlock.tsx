import React, {useState, useEffect, useRef, memo} from "react"

import type {InstructionDetail} from "../../lib/types"

import styles from "./StepInstructionBlock.module.css"

interface StepInstructionBlockProps {
  readonly steps: readonly InstructionDetail[]
  readonly currentIndex: number
  readonly itemHeight?: number
}

export const StepInstructionBlock: React.FC<StepInstructionBlockProps> = ({
  steps,
  currentIndex,
  itemHeight = 48,
}) => {
  const [transformY, setTransformY] = useState(0)
  const prevIndexRef = useRef(currentIndex)

  useEffect(() => {
    const targetTransformY = -currentIndex * itemHeight
    setTransformY(targetTransformY)
    prevIndexRef.current = currentIndex
  }, [currentIndex, steps.length, itemHeight])

  if (steps.length === 0) {
    return (
      <div className={styles.stepInstructionContainer} style={{height: `${itemHeight}px`}}>
        <div className={styles.instructionItem} style={{height: `${itemHeight}px`}}>
          No instructions to display.
        </div>
      </div>
    )
  }

  return (
    <div className={styles.stepInstructionContainer} style={{height: `${itemHeight}px`}}>
      <div
        className={styles.instructionWindow}
        style={{
          transform: `translateY(${transformY}px)`,
          height: `${steps.length * itemHeight}px`,
        }}
      >
        <StepInstructionsListMemo steps={steps} itemHeight={itemHeight} />
      </div>
    </div>
  )
}

interface StepInstructionsListProps {
  readonly steps: readonly InstructionDetail[]
  readonly itemHeight?: number
}

export const StepInstructionsList: React.FC<StepInstructionsListProps> = ({steps, itemHeight}) => {
  return (
    <>
      {steps.map((instruction, index) => (
        <div key={index} className={styles.instructionItem} style={{height: `${itemHeight}px`}}>
          <span className={styles.instructionName}>{instruction.name}</span>
          <span className={styles.instructionGas}>{instruction.gasCost.toString()} gas</span>
        </div>
      ))}
    </>
  )
}

export const StepInstructionsListMemo = memo(StepInstructionsList)
StepInstructionsListMemo.displayName = "StepInstructionsListMemo"

export default memo(StepInstructionBlock)
