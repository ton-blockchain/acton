import type React from "react"
import {useCallback, useEffect, useRef, useState} from "react"

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
  const [tooltip, setTooltip] = useState<TooltipData | undefined>()
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

      const viewportPadding = 8
      const triggerGap = 10
      const maxX = Math.max(viewportPadding, viewport.width - tooltipWidth - viewportPadding)
      const maxY = Math.max(viewportPadding, viewport.height - tooltipHeight - viewportPadding)
      const clampX = (x: number): number => Math.max(viewportPadding, Math.min(x, maxX))
      const clampY = (y: number): number => Math.max(viewportPadding, Math.min(y, maxY))
      const centeredX = triggerRect.left + triggerRect.width / 2 - tooltipWidth / 2
      const centeredY = triggerRect.top + triggerRect.height / 2 - tooltipHeight / 2

      const positions: readonly TooltipPosition[] = [
        {
          placement: "top",
          x: clampX(centeredX),
          y: triggerRect.top - tooltipHeight - triggerGap,
        },
        {
          placement: "bottom",
          x: clampX(centeredX),
          y: triggerRect.bottom + triggerGap,
        },
        {
          placement: "right",
          x: triggerRect.right + triggerGap,
          y: clampY(centeredY),
        },
        {
          placement: "left",
          x: triggerRect.left - tooltipWidth - triggerGap,
          y: clampY(centeredY),
        },
      ]

      const fitsViewport = (position: TooltipPosition): boolean => {
        return (
          position.x >= viewportPadding &&
          position.x + tooltipWidth <= viewport.width - viewportPadding &&
          position.y >= viewportPadding &&
          position.y + tooltipHeight <= viewport.height - viewportPadding
        )
      }

      const visibleArea = (position: TooltipPosition): number => {
        const visibleLeft = Math.max(viewportPadding, position.x)
        const visibleRight = Math.min(viewport.width - viewportPadding, position.x + tooltipWidth)
        const visibleTop = Math.max(viewportPadding, position.y)
        const visibleBottom = Math.min(
          viewport.height - viewportPadding,
          position.y + tooltipHeight,
        )

        return Math.max(0, visibleRight - visibleLeft) * Math.max(0, visibleBottom - visibleTop)
      }

      const bestPosition =
        positions.find(position => fitsViewport(position)) ??
        positions.reduce((best, current) =>
          visibleArea(current) > visibleArea(best) ? current : best,
        )

      const finalX = clampX(bestPosition.x)
      const finalY = clampY(bestPosition.y)

      return {
        x: finalX,
        y: finalY,
        placement: bestPosition.placement,
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
