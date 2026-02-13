import * as React from "react"
import {useEffect, useRef, useState} from "react"
import {createPortal} from "react-dom"

import styles from "./SmartTooltip.module.css"
import type {TooltipPosition} from "./useTooltip"

interface SmartTooltipProps {
  readonly content: React.ReactNode
  readonly triggerRect: DOMRect
  readonly onMouseEnter: () => void
  readonly onMouseLeave: () => void
  readonly onForceHide?: () => void
  readonly calculateOptimalPosition: (
    triggerRect: DOMRect,
    tooltipWidth: number,
    tooltipHeight: number,
  ) => TooltipPosition
}

function calculateBridgeRect(
  triggerRect: DOMRect,
  tooltipRect: DOMRect,
  placement: TooltipPosition["placement"],
): DOMRect {
  const margin = 20

  switch (placement) {
    case "right": {
      return new DOMRect(
        triggerRect.right,
        Math.min(triggerRect.top, tooltipRect.top) - margin,
        tooltipRect.left - triggerRect.right,
        Math.max(triggerRect.bottom, tooltipRect.bottom + margin) -
          Math.min(triggerRect.top, tooltipRect.top) +
          margin * 2,
      )
    }
    case "left": {
      return new DOMRect(
        tooltipRect.right,
        Math.min(triggerRect.top, tooltipRect.top) - margin,
        triggerRect.left - tooltipRect.right,
        Math.max(triggerRect.bottom, tooltipRect.bottom + margin) -
          Math.min(triggerRect.top, tooltipRect.top) +
          margin * 2,
      )
    }
    case "bottom": {
      return new DOMRect(
        Math.min(triggerRect.left, tooltipRect.left) - margin,
        triggerRect.bottom,
        Math.max(triggerRect.right, tooltipRect.right + margin) -
          Math.min(triggerRect.left, tooltipRect.left) +
          margin * 2,
        tooltipRect.top - triggerRect.bottom,
      )
    }
    case "top": {
      return new DOMRect(
        Math.min(triggerRect.left, tooltipRect.left) - margin,
        tooltipRect.bottom,
        Math.max(triggerRect.right, tooltipRect.right + margin) -
          Math.min(triggerRect.left, tooltipRect.left) +
          margin * 2,
        triggerRect.top - tooltipRect.bottom,
      )
    }
  }

  return new DOMRect(0, 0, 0, 0)
}

export function SmartTooltip({
  content,
  triggerRect,
  onMouseEnter,
  onMouseLeave,
  onForceHide,
  calculateOptimalPosition,
}: SmartTooltipProps): React.JSX.Element {
  const tooltipRef = useRef<HTMLDivElement | null>(null)
  const bridgeRef = useRef<HTMLDivElement | null>(null)
  const [position, setPosition] = useState<TooltipPosition>({x: 0, y: 0, placement: "right"})
  const [isVisible, setIsVisible] = useState(false)
  const [bridgeRect, setBridgeRect] = useState<DOMRect | undefined>()

  useEffect(() => {
    if (!tooltipRef.current) return

    const tooltipRect = tooltipRef.current.getBoundingClientRect()
    const optimalPosition = calculateOptimalPosition(
      triggerRect,
      tooltipRect.width || 400,
      tooltipRect.height || 200,
    )

    setPosition(optimalPosition)

    const tooltipDOMRect = new DOMRect(
      optimalPosition.x,
      optimalPosition.y,
      tooltipRect.width || 400,
      tooltipRect.height || 200,
    )

    const bridge = calculateBridgeRect(triggerRect, tooltipDOMRect, optimalPosition.placement)
    setBridgeRect(bridge)

    setIsVisible(true)
  }, [triggerRect, calculateOptimalPosition])

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent): void => {
      if (
        tooltipRef.current &&
        !tooltipRef.current.contains(event.target as Node) &&
        bridgeRef.current &&
        !bridgeRef.current.contains(event.target as Node)
      ) {
        if (onForceHide) {
          onForceHide()
        } else {
          onMouseLeave()
        }
      }
    }

    const handleEscape = (event: KeyboardEvent): void => {
      if (event.key === "Escape") {
        if (onForceHide) {
          onForceHide()
        } else {
          onMouseLeave()
        }
      }
    }

    const handleFocusOut = (): void => {
      if (onForceHide) {
        onForceHide()
      } else {
        onMouseLeave()
      }
    }

    document.addEventListener("mousedown", handleClickOutside)
    document.addEventListener("keydown", handleEscape)
    window.addEventListener("blur", handleFocusOut)

    return () => {
      document.removeEventListener("mousedown", handleClickOutside)
      document.removeEventListener("keydown", handleEscape)
      window.removeEventListener("blur", handleFocusOut)
    }
  }, [onMouseLeave, onForceHide])

  const tooltipElement = (
    <>
      {bridgeRect && (
        <div
          ref={bridgeRef}
          role="button"
          tabIndex={-1}
          aria-label="Tooltip bridge"
          className={`${styles.bridge} ${styles[`bridge--${position.placement}`]}`}
          style={{
            left: bridgeRect.x,
            top: bridgeRect.y,
            width: bridgeRect.width,
            height: bridgeRect.height,
          }}
          onMouseEnter={onMouseEnter}
          onMouseLeave={onMouseLeave}
        />
      )}

      <div
        ref={tooltipRef}
        role="tooltip"
        className={`${styles.tooltip} ${styles[`tooltip--${position.placement}`]} ${isVisible ? styles.tooltipVisible : ""}`}
        style={{
          left: position.x,
          top: position.y,
          pointerEvents: "auto",
        }}
        onMouseEnter={onMouseEnter}
        onMouseLeave={onMouseLeave}
      >
        <div className={styles.content}>{content}</div>
      </div>
    </>
  )

  return createPortal(tooltipElement, document.body)
}
