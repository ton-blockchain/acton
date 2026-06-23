import React, {useEffect, useState, useCallback} from "react"

import {FaArrowLeft, FaArrowRight, FaCheck} from "react-icons/fa"

import styles from "./Tutorial.module.css"

export interface TutorialStep {
  readonly title: string
  readonly content: string
  readonly target?: string
  readonly placement?: "top" | "bottom" | "left" | "right"
  readonly autoAction?: {
    readonly type: "click" | "custom"
    readonly selector?: string
    readonly action?: () => void
    readonly delay?: number
  }
  readonly onStepEnter?: () => void
  readonly onStepExit?: () => void
}

interface TutorialProps {
  readonly steps: TutorialStep[]
  readonly isOpen: boolean
  readonly onClose: () => void
  readonly onComplete?: () => void
}

const Tutorial: React.FC<TutorialProps> = ({steps, isOpen, onClose, onComplete}) => {
  const [currentStep, setCurrentStep] = useState(0)
  const [targetElement, setTargetElement] = useState<HTMLElement | null>(null)
  const [tooltipPosition, setTooltipPosition] = useState({top: 0, left: 0})
  const [isInitialized, setIsInitialized] = useState(false)

  useEffect(() => {
    if (isOpen) {
      const timer = setTimeout(() => {
        setIsInitialized(true)
      }, 100)
      return () => clearTimeout(timer)
    } 
      setCurrentStep(0)
      setIsInitialized(false)
  }, [isOpen])

  const updateTargetAndPosition = useCallback(() => {
    if (!isOpen || steps.length === 0) return

    const step = steps[currentStep]

    if (!step.target) {
      setTargetElement(null)
      const centerTop = window.innerHeight / 2 - 100
      const centerLeft = window.innerWidth / 2 - 190
      setTooltipPosition({
        top: Math.max(20, centerTop),
        left: Math.max(20, Math.min(window.innerWidth - 400, centerLeft)),
      })
      return
    }

    const element: HTMLElement | null = document.querySelector(step.target)
    setTargetElement(element)

    if (element) {
      const rect = element.getBoundingClientRect()
      const placement = step.placement || "bottom"

      let top = 0
      let left = 0

      switch (placement) {
        case "top":
          top = rect.top - 120
          left = rect.left + rect.width / 2 - 200
          break
        case "bottom":
          top = rect.bottom + 20
          left = rect.left + rect.width / 2 - 200
          break
        case "left":
          top = rect.top + rect.height / 2 - 75
          left = rect.left - 420
          break
        case "right":
          top = rect.top + rect.height / 2 - 75
          left = rect.right + 20
          break
      }

      left = Math.max(20, Math.min(window.innerWidth - 420, left))
      top = Math.max(20, Math.min(window.innerHeight - 200, top))

      setTooltipPosition({top, left})
    }
  }, [currentStep, steps, isOpen])

  useEffect(() => {
    updateTargetAndPosition()

    const handleResize = () => updateTargetAndPosition()
    window.addEventListener("resize", handleResize)

    return () => window.removeEventListener("resize", handleResize)
  }, [updateTargetAndPosition])

  const step = steps[currentStep]
  const isFirst = currentStep === 0
  const isLast = currentStep === steps.length - 1

  const handleNext = useCallback(() => {
    const currentStepData = steps[currentStep]

    currentStepData?.onStepExit?.()

    if (isLast) {
      onComplete?.()
      onClose()
    } else {
      setCurrentStep(prev => prev + 1)
    }
  }, [currentStep, steps, isLast, onComplete, onClose])

  const handlePrevious = useCallback(() => {
    if (!isFirst) {
      setCurrentStep(prev => prev - 1)
    }
  }, [isFirst])

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose()
      } else if (event.key === "ArrowLeft" && !isFirst) {
        handlePrevious()
      } else if (event.key === "ArrowRight") {
        handleNext()
      } else if (event.key === "Enter" && isLast) {
        handleNext()
      }
      event.stopPropagation()
    }

    if (isOpen) {
      document.addEventListener("keydown", handleKeyDown)
      document.body.style.overflow = "hidden"
    }

    return () => {
      document.removeEventListener("keydown", handleKeyDown)
      document.body.style.overflow = "unset"
    }
  }, [isOpen, onClose, isFirst, handleNext, handlePrevious, isLast])

  useEffect(() => {
    if (!isOpen || steps.length === 0) return

    const step = steps[currentStep]

    step?.onStepEnter?.()

    if (step?.autoAction) {
      const delay = step.autoAction.delay ?? 500
      const timer = setTimeout(() => {
        if (step.autoAction?.type === "click" && step.autoAction.selector) {
          const element: HTMLElement | null = document.querySelector(step.autoAction.selector)
          if (element) {
            element.click()
          }
        } else if (step.autoAction?.type === "custom" && step.autoAction.action) {
          step.autoAction.action()
        }
      }, delay)

      return () => clearTimeout(timer)
    }
  }, [currentStep, steps, isOpen])

  if (!isOpen || steps.length === 0) return null

  return (
    <>
      <div className={styles.progressBar}>
        <div
          className={styles.progressFill}
          style={{width: `${((currentStep + 1) / steps.length) * 100}%`}}
        />
      </div>

      <div className={`${styles.overlay} ${targetElement ? "" : styles.centeredOverlay}`}>
        {targetElement && (
          <div
            className={`${styles.spotlight} ${isInitialized ? styles.spotlightAnimated : ""}`}
            style={{
              top: targetElement.getBoundingClientRect().top,
              left: targetElement.getBoundingClientRect().left,
              width: targetElement.getBoundingClientRect().width,
              height: targetElement.getBoundingClientRect().height,
            }}
          />
        )}
      </div>

      <div
        className={`${styles.tooltip} ${isInitialized ? styles.animated : ""}`}
        style={{
          top: tooltipPosition.top,
          left: tooltipPosition.left,
        }}
      >
        <div className={styles.header}>
          <h2 className={styles.title}>{step.title}</h2>
          <button className={styles.closeButton} onClick={onClose} aria-label="Close tutorial">
            ×
          </button>
        </div>

        <div className={styles.body}>
          <div
            className={styles.text}
            dangerouslySetInnerHTML={{
              __html: step.content.replace(/\n\n/g, "<br><br>").replace(/\n/g, "<br>"),
            }}
          />
        </div>

        <div className={styles.footer}>
          <div className={styles.stepCounter}>
            {currentStep + 1} of {steps.length}
            <div className={styles.keyboardHint}>
              {isLast ? "Press Enter or → to finish" : "Use ← → keys for navigation"}
            </div>
          </div>

          <div className={styles.actions}>
            <button
              className={styles.button}
              onClick={handlePrevious}
              disabled={isFirst}
              aria-label="Previous step"
            >
              <FaArrowLeft />
            </button>
            <button
              className={`${styles.button} ${styles.primaryButton}`}
              onClick={handleNext}
              aria-label={isLast ? "Finish tutorial" : "Next step"}
            >
              {isLast ? <FaCheck /> : <FaArrowRight />}
            </button>
          </div>
        </div>
      </div>
    </>
  )
}

export default Tutorial
