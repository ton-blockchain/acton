import {useCallback, useEffect, useMemo, useState} from "react"
import {trace} from "ton-assembly"
import type {InstructionInfo} from "ton-source-map"

import type {UseTraceStepperReturn} from "./useTraceStepper"

export interface CompilationResult {
  readonly mapping?: Map<number, InstructionInfo[]>
  readonly assembly?: string
  readonly funcSourceMap?: trace.FuncMapping
}

export interface UseFuncLineStepperOptions {
  readonly sourceMap: trace.FuncMapping | undefined
  readonly compilationResult: CompilationResult | undefined
  readonly traceInfo: trace.TraceInfo | undefined
  readonly isEnabled: boolean
}

export interface UseFuncLineStepperReturn extends UseTraceStepperReturn {
  readonly funcSteps: Array<{stepIndex: number; funcLine: number}>
  readonly currentFuncStepIndex?: number
  readonly handlePrevFunc?: () => void
  readonly handleNextFunc?: () => void
  readonly totalFuncSteps?: number
}

export function useFuncLineStepper(
  baseStepperReturn: UseTraceStepperReturn,
  {sourceMap, compilationResult, traceInfo, isEnabled}: UseFuncLineStepperOptions,
): UseFuncLineStepperReturn {
  const funcSteps = useMemo(() => {
    if (!isEnabled || !traceInfo || !sourceMap || !compilationResult?.mapping) {
      return []
    }

    const funcLineToSteps = new Map<number, number[]>()

    for (let i = 0; i < traceInfo.steps.length; i++) {
      const step = traceInfo.steps[i]
      if (!step?.loc?.line) continue

      const asmLine = step.loc.line + 1

      for (const [debugSection, instructions] of compilationResult.mapping.entries()) {
        for (const instr of instructions) {
          if (instr.loc?.line !== undefined && instr.loc.line + 1 === asmLine) {
            for (const [debugId, location] of sourceMap.locations.entries()) {
              if (debugId === debugSection && location.file === "main.fc") {
                const funcLine = location.line
                if (!funcLineToSteps.has(funcLine)) {
                  funcLineToSteps.set(funcLine, [])
                }
                funcLineToSteps.get(funcLine)?.push(i)
                break
              }
            }
            break
          }
        }
      }
    }

    const steps: Array<{stepIndex: number; funcLine: number}> = []
    const processedLines = new Set<number>()

    for (let i = 0; i < traceInfo.steps.length; i++) {
      const step = traceInfo.steps[i]
      if (!step?.loc?.line) continue

      const asmLine = step.loc.line + 1

      for (const [debugSection, instructions] of compilationResult.mapping.entries()) {
        for (const instr of instructions) {
          if (instr.loc?.line !== undefined && instr.loc.line + 1 === asmLine) {
            for (const [debugId, location] of sourceMap.locations.entries()) {
              if (debugId === debugSection && location.file === "main.fc") {
                const funcLine = location.line
                const stepIndices = funcLineToSteps.get(funcLine)

                if (stepIndices && !processedLines.has(funcLine)) {
                  const lastStepIndex = stepIndices[stepIndices.length - 1]
                  if (i === lastStepIndex) {
                    steps.push({stepIndex: i, funcLine})
                    processedLines.add(funcLine)
                  }
                }
                break
              }
            }
            break
          }
        }
      }
    }

    return steps
  }, [isEnabled, traceInfo, sourceMap, compilationResult?.mapping])

  const [currentFuncStepIndex, setCurrentFuncStepIndex] = useState(0)

  useEffect(() => {
    if (isEnabled && funcSteps.length > 0) {
      setCurrentFuncStepIndex(0)
    }
  }, [isEnabled, funcSteps.length])

  const currentFuncStep = funcSteps[currentFuncStepIndex]
  const actualSelectedStep =
    isEnabled && currentFuncStep ? currentFuncStep.stepIndex : baseStepperReturn.selectedStep

  const canGoPrevFunc = isEnabled ? currentFuncStepIndex > 0 : baseStepperReturn.canGoPrev
  const canGoNextFunc = isEnabled
    ? currentFuncStepIndex < funcSteps.length - 1
    : baseStepperReturn.canGoNext

  const handlePrevFunc = useCallback(() => {
    if (isEnabled) {
      setCurrentFuncStepIndex(prev => Math.max(0, prev - 1))
    } else {
      baseStepperReturn.handlePrev()
    }
  }, [isEnabled, baseStepperReturn])

  const handleNextFunc = useCallback(() => {
    if (isEnabled) {
      setCurrentFuncStepIndex(prev => Math.min(funcSteps.length - 1, prev + 1))
    } else {
      baseStepperReturn.handleNext()
    }
  }, [isEnabled, funcSteps.length, baseStepperReturn])

  const goToFirstStepFunc = useCallback(() => {
    if (isEnabled) {
      setCurrentFuncStepIndex(0)
    } else {
      baseStepperReturn.goToFirstStep()
    }
  }, [isEnabled, baseStepperReturn])

  const goToLastStepFunc = useCallback(() => {
    if (isEnabled) {
      setCurrentFuncStepIndex(funcSteps.length - 1)
    } else {
      baseStepperReturn.goToLastStep()
    }
  }, [isEnabled, funcSteps.length, baseStepperReturn])

  const actualCurrentStep = useMemo(() => {
    if (!traceInfo || actualSelectedStep < 0 || actualSelectedStep >= traceInfo.steps.length) {
      return undefined
    }
    return traceInfo.steps[actualSelectedStep]
  }, [traceInfo, actualSelectedStep])

  const actualCurrentStack = useMemo(() => {
    return actualCurrentStep?.stack ?? []
  }, [actualCurrentStep])

  return {
    ...baseStepperReturn,
    selectedStep: actualSelectedStep,
    currentStep: actualCurrentStep,
    currentStack: actualCurrentStack,
    totalSteps: isEnabled ? funcSteps.length : baseStepperReturn.totalSteps,
    canGoPrev: canGoPrevFunc,
    canGoNext: canGoNextFunc,
    handlePrev: handlePrevFunc,
    handleNext: handleNextFunc,
    goToFirstStep: goToFirstStepFunc,
    goToLastStep: goToLastStepFunc,
    funcSteps,
    currentFuncStepIndex: isEnabled ? currentFuncStepIndex : undefined,
    handlePrevFunc: isEnabled ? handlePrevFunc : undefined,
    handleNextFunc: isEnabled ? handleNextFunc : undefined,
    totalFuncSteps: isEnabled ? funcSteps.length : undefined,
  }
}
