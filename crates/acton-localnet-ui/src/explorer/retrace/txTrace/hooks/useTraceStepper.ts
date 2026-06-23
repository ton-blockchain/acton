import {useCallback, useEffect, useMemo, useState} from "react"
import type {TraceInfo, StackElement, Step} from "ton-assembly/dist/trace"

export interface UseTraceStepperReturn {
  readonly selectedStep: number
  readonly highlightLine: number | undefined
  readonly currentStep: ReturnType<typeof getCurrentStep>
  readonly currentStack: readonly StackElement[]
  readonly goToStep: (stepIndex: number) => void
  readonly handlePrev: () => void
  readonly handleNext: () => void
  readonly goToFirstStep: () => void
  readonly goToLastStep: () => void
  readonly canGoPrev: boolean
  readonly canGoNext: boolean
  readonly findStepByLine: (line: number) => void
  readonly transitionType: "button" | "click" | null
  readonly totalSteps: number
}

function getCurrentStep(trace: TraceInfo | undefined, index: number): Step | undefined {
  if (!trace || index < 0 || index >= trace.steps.length) {
    return undefined
  }
  return trace.steps[index]
}

export function useTraceStepper(trace: TraceInfo | undefined): UseTraceStepperReturn {
  const [selectedStep, setSelectedStep] = useState(0)
  const [transitionType, setTransitionType] = useState<"button" | "click" | null>(null)
  const totalSteps = useMemo(() => trace?.steps.length ?? 0, [trace])

  const canGoPrev = useMemo(() => selectedStep > 0, [selectedStep])
  const canGoNext = useMemo(() => selectedStep < totalSteps - 1, [selectedStep, totalSteps])

  useEffect(() => {
    setSelectedStep(0)
  }, [trace])

  useEffect(() => {
    if (!transitionType) return
    const id = setTimeout(() => setTransitionType(null), 100)
    return () => clearTimeout(id)
  }, [transitionType, selectedStep])

  const handlePrev = useCallback(() => {
    if (totalSteps === 0 || !canGoPrev) return
    setTransitionType("button")
    setSelectedStep(prev => prev - 1)
  }, [totalSteps, canGoPrev])

  const handleNext = useCallback(() => {
    if (totalSteps === 0 || !canGoNext) return
    setTransitionType("button")
    setSelectedStep(prev => prev + 1)
  }, [totalSteps, canGoNext])

  const goToStep = useCallback(
    (stepIndex: number) => {
      if (totalSteps === 0) return
      const boundedIndex = Math.max(0, Math.min(stepIndex, totalSteps - 1))
      if (boundedIndex === selectedStep) return
      setTransitionType("click")
      setSelectedStep(boundedIndex)
    },
    [totalSteps, selectedStep],
  )

  const goToFirstStep = useCallback(() => {
    if (totalSteps === 0 || !canGoPrev) return
    setTransitionType("button")
    setSelectedStep(0)
  }, [totalSteps, canGoPrev])

  const goToLastStep = useCallback(() => {
    if (totalSteps === 0 || !canGoNext) return
    setTransitionType("button")
    setSelectedStep(totalSteps - 1)
  }, [totalSteps, canGoNext])

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) {
        return
      }
      if (e.key === "ArrowLeft") handlePrev()
      if (e.key === "ArrowRight") handleNext()
    }
    window.addEventListener("keydown", onKey)
    return () => window.removeEventListener("keydown", onKey)
  }, [handlePrev, handleNext])

  const highlightLine = useMemo(() => {
    const step = getCurrentStep(trace, selectedStep)
    if (step && step.loc !== undefined) return step.loc.line + 1
    return undefined
  }, [trace, selectedStep])

  const currentStep = useMemo(() => getCurrentStep(trace, selectedStep), [trace, selectedStep])
  const currentStack: readonly StackElement[] = currentStep?.stack ?? []

  const findStepByLine = useCallback(
    (line: number) => {
      if (!trace || !trace.steps.length) {
        return
      }
      const map: Record<number, number[]> = {}
      trace.steps.forEach((step, idx) => {
        if (step.loc !== undefined) {
          const stepLine = step.loc.line + 1
          ;(map[stepLine] ??= []).push(idx)
        }
      })
      if (!map[line]?.length) {
        return
      }
      const idxInLine = map[line].indexOf(selectedStep)
      setTransitionType("click")
      if (idxInLine !== -1 && idxInLine < map[line].length - 1) {
        setSelectedStep(map[line][idxInLine + 1])
      } else {
        setSelectedStep(map[line][0])
      }
    },
    [trace, selectedStep],
  )

  return {
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
  }
}
