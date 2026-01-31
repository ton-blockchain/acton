import type React from "react"
import { useCallback, useEffect, useRef, useState } from "react"

export interface TooltipPosition {
  readonly x: number
  readonly y: number
  readonly placement: "top" | "bottom" | "left" | "right"
}

export interface TooltipData {
  readonly id: string
  readonly x: number
  readonly y: number
  readonly content: React.ReactNode
  readonly triggerElement?: HTMLElement | SVGElement
}

interface UseTooltipReturn {
  readonly tooltip: TooltipData | undefined
  readonly showTooltip: (data: Omit<TooltipData, "id">) => void
  readonly hideTooltip: (force?: boolean) => void
  readonly forceHideTooltip: () => void
  readonly isTooltipHovered: boolean
  readonly setIsTooltipHovered: (hovered: boolean) => void
  readonly calculateOptimalPosition: (
    triggerRect: DOMRect,
    tooltipWidth: number,
    tooltipHeight: number,
  ) => TooltipPosition
}

export function useTooltip(): UseTooltipReturn {
  const [tooltip, setTooltip] = useState<TooltipData | undefined>(undefined)
  const [isTooltipHovered, setIsTooltipHovered] = useState(false)
  const hideTimeoutRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)
  const tooltipIdRef = useRef(0)

  const clearHideTimeout = useCallback(() => {
    if (hideTimeoutRef.current) {
      clearTimeout(hideTimeoutRef.current)
      hideTimeoutRef.current = undefined
    }
  }, [])

  const calculateOptimalPosition = useCallback(
    (triggerRect: DOMRect, tooltipWidth: number, tooltipHeight: number): TooltipPosition => {
      const viewport = {
        width: window.innerWidth,
        height: window.innerHeight,
      }

      const margin = 10
      const positions = [
        {
          placement: "right" as const,
          x: triggerRect.right + margin,
          y: triggerRect.top + triggerRect.height / 2 - tooltipHeight / 2,
          score: 0,
        },
        {
          placement: "left" as const,
          x: triggerRect.left - tooltipWidth - margin,
          y: triggerRect.top + triggerRect.height / 2 - tooltipHeight / 2,
          score: 0,
        },
        {
          placement: "bottom" as const,
          x: triggerRect.left + triggerRect.width / 2 - tooltipWidth / 2,
          y: triggerRect.bottom + margin,
          score: 0,
        },
        {
          placement: "top" as const,
          x: triggerRect.left + triggerRect.width / 2 - tooltipWidth / 2,
          y: triggerRect.top - tooltipHeight - margin,
          score: 0,
        },
      ]

      positions.forEach((pos) => {
        const fitsHorizontally = pos.x >= margin && pos.x + tooltipWidth <= viewport.width - margin
        const fitsVertically = pos.y >= margin && pos.y + tooltipHeight <= viewport.height - margin

        if (fitsHorizontally && fitsVertically) {
          pos.score += 100
        }

        if (pos.x < margin) pos.score -= (margin - pos.x) * 2
        if (pos.x + tooltipWidth > viewport.width - margin) {
          pos.score -= (pos.x + tooltipWidth - viewport.width + margin) * 2
        }
        if (pos.y < margin) pos.score -= (margin - pos.y) * 2
        if (pos.y + tooltipHeight > viewport.height - margin) {
          pos.score -= (pos.y + tooltipHeight - viewport.height + margin) * 2
        }

        const distanceFromLeft = pos.x
        const distanceFromRight = viewport.width - (pos.x + tooltipWidth)
        const distanceFromTop = pos.y
        const distanceFromBottom = viewport.height - (pos.y + tooltipHeight)

        pos.score += Math.min(
          distanceFromLeft,
          distanceFromRight,
          distanceFromTop,
          distanceFromBottom,
        )
      })

      const bestPosition = positions.reduce((best, current) =>
        current.score > best.score ? current : best,
      )

      const finalX = Math.max(
        margin,
        Math.min(bestPosition.x, viewport.width - tooltipWidth - margin),
      )
      const finalY = Math.max(
        margin,
        Math.min(bestPosition.y, viewport.height - tooltipHeight - margin),
      )

      return {
        x: finalX,
        y: finalY,
        placement: bestPosition.placement as "top" | "bottom" | "left" | "right",
      }
    },
    [],
  )

  const showTooltip = useCallback(
    (data: Omit<TooltipData, "id">) => {
      clearHideTimeout()
      const id = `tooltip-${++tooltipIdRef.current}`
      setTooltip({
        ...data,
        id,
      })
    },
    [clearHideTimeout],
  )

  const hideTooltip = useCallback(
    (force = false) => {
      if (!force && isTooltipHovered) {
        return
      }

      clearHideTimeout()
      hideTimeoutRef.current = setTimeout(() => {
        setTooltip(undefined)
        setIsTooltipHovered(false)
      }, 0)
    },
    [isTooltipHovered, clearHideTimeout],
  )

  const forceHideTooltip = useCallback(() => {
    clearHideTimeout()
    setTooltip(undefined)
    setIsTooltipHovered(false)
  }, [clearHideTimeout])

  const setIsTooltipHoveredWithClear = useCallback(
    (hovered: boolean) => {
      if (hovered) {
        clearHideTimeout()
        setIsTooltipHovered(true)
      } else {
        setIsTooltipHovered(false)
        hideTooltip(true)
      }
    },
    [clearHideTimeout, hideTooltip],
  )

  useEffect(() => {
    return () => {
      clearHideTimeout()
    }
  }, [clearHideTimeout])

  return {
    tooltip,
    showTooltip,
    hideTooltip,
    forceHideTooltip,
    isTooltipHovered,
    setIsTooltipHovered: setIsTooltipHoveredWithClear,
    calculateOptimalPosition,
  }
}
