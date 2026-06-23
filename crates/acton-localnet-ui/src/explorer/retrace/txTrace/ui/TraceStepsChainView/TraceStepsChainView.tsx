import React, {useEffect, useRef} from "react"

import type {InstructionDetail} from "@retrace/txTrace/lib/types"

import styles from "./TraceStepsChainView.module.css"

interface TraceStepsChainViewProps {
  readonly steps: readonly InstructionDetail[]
  readonly selectedStep: number
  readonly onStepClick?: (index: number) => void
}

interface InstructionArgToken {
  readonly value: string
  readonly type: "number" | "stackRegister" | "controlRegister" | "hexBlob" | "text" | "separator"
}

function classifyInstructionArgToken(
  token: string,
): "number" | "stackRegister" | "controlRegister" | "hexBlob" | "text" {
  if (/^s-?\d{1,2}$/i.test(token)) {
    return "stackRegister"
  }

  if (/^c\d{1,2}$/i.test(token)) {
    return "controlRegister"
  }

  if (/^x\{[0-9a-fA-F_]*\}$/i.test(token) || /^x[0-9a-fA-F_]+(?:\.\.\.)?$/i.test(token)) {
    return "hexBlob"
  }

  if (
    /^-?\d+$/.test(token) ||
    /^b\{[01]*\}$/i.test(token) ||
    /^boc\{[0-9a-fA-F]*\}$/i.test(token)
  ) {
    return "number"
  }

  return "text"
}

function tokenizeInstructionArgs(args: string): readonly InstructionArgToken[] {
  const chunks: readonly string[] = args.match(/(\s+|,|[^\s,]+)/g) ?? []

  return chunks.map((chunk: string): InstructionArgToken => {
    if (/^\s+$/.test(chunk) || chunk === ",") {
      return {value: chunk, type: "separator"}
    }

    return {
      value: chunk,
      type: classifyInstructionArgToken(chunk),
    }
  })
}

function splitInstructionText(step: InstructionDetail): {name: string; args: string | undefined} {
  const instructionText = step.instructionText?.trim()
  if (!instructionText) {
    return {name: step.name, args: undefined}
  }

  const firstSpaceIndex = instructionText.search(/\s/)
  if (firstSpaceIndex === -1) {
    return {name: instructionText, args: undefined}
  }

  const name = instructionText.slice(0, firstSpaceIndex)
  const args = instructionText.slice(firstSpaceIndex).trim()
  return {name, args: args.length > 0 ? args : undefined}
}

const TraceStepsChainView: React.FC<TraceStepsChainViewProps> = ({
  steps,
  selectedStep,
  onStepClick,
}) => {
  const containerRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const activeStep = containerRef.current?.querySelector<HTMLElement>(
      `[data-step-index="${selectedStep}"]`,
    )
    activeStep?.scrollIntoView({block: "nearest"})
  }, [selectedStep])

  if (!steps.length) {
    return (
      <div className={styles.emptyState} role="status" aria-live="polite">
        No trace steps available.
      </div>
    )
  }

  return (
    <div className={styles.container} ref={containerRef} aria-label="Trace step chain">
      {steps.map((step, index) => {
        const isActive = index === selectedStep
        const instruction = splitInstructionText(step)
        const argsTokens = instruction.args ? tokenizeInstructionArgs(instruction.args) : []
        const isImplicitRet = /^implicit\s+(?:ret|jmpref)$/i.test(
          step.instructionText?.trim() ?? step.name,
        )

        const getTokenClassName = (
          tokenType: InstructionArgToken["type"],
          implicitRet: boolean,
        ): string => {
          if (implicitRet) {
            return styles.stepArgImplicitRet
          }

          if (tokenType === "stackRegister") {
            return styles.stepArgStackRegister
          }

          if (tokenType === "controlRegister") {
            return styles.stepArgControlRegister
          }

          if (tokenType === "hexBlob") {
            return styles.stepArgHexBlob
          }

          if (tokenType === "number") {
            return styles.stepArgNumber
          }

          return styles.stepArgText
        }

        return (
          <button
            key={`${step.instructionText ?? step.name}-${index.toString()}`}
            type="button"
            data-step-index={index}
            className={`${styles.stepItem} ${isActive ? styles.stepItemActive : ""}`}
            onClick={() => {
              onStepClick?.(index)
            }}
            aria-current={isActive ? "step" : undefined}
          >
            <span className={styles.stepIndex}>{index + 1}</span>
            <span className={styles.stepInstruction}>
              <span
                className={`${styles.stepName} ${isImplicitRet ? styles.stepNameImplicitRet : ""}`}
              >
                {instruction.name}
              </span>
              {instruction.args && (
                <span className={styles.stepArgs}>
                  {argsTokens.map((token, tokenIndex) => (
                    <React.Fragment key={`${token.value}-${tokenIndex.toString()}`}>
                      <span
                        className={
                          token.type === "separator"
                            ? styles.stepArgSeparator
                            : getTokenClassName(token.type, isImplicitRet)
                        }
                      >
                        {token.value}
                      </span>
                    </React.Fragment>
                  ))}
                </span>
              )}
            </span>
            <span className={styles.stepGas}>{step.gasCost.toString()} gas</span>
          </button>
        )
      })}
    </div>
  )
}

export default TraceStepsChainView
