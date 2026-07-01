import type {RefCallback} from "react"
import {useCallback, useLayoutEffect, useState} from "react"

export interface AvailableFlowMetrics {
  readonly offset: number
  readonly width: number
}

export function useAvailableFlowMetrics<TElement extends HTMLElement>(
  maxWidth: number,
): {
  readonly flowMetrics: AvailableFlowMetrics
  readonly rootRef: RefCallback<TElement>
} {
  const [rootElement, setRootElement] = useState<TElement | null>(null)
  const [flowMetrics, setFlowMetrics] = useState<AvailableFlowMetrics>({offset: 0, width: 0})
  const rootRef = useCallback((element: TElement | null) => {
    setRootElement(element)
  }, [])

  useLayoutEffect(() => {
    const updateFlowMetrics = () => {
      const root = rootElement
      if (!root) {
        return
      }

      const anchor = root.parentElement ?? root
      const viewportWidth = Math.round(
        document.documentElement.clientWidth || globalThis.innerWidth,
      )
      const availableRect = root.closest("main")?.getBoundingClientRect()
      const availableLeft = Math.max(0, Math.round(availableRect?.left ?? 0))
      const availableRight = Math.min(
        viewportWidth,
        Math.round(availableRect?.right ?? viewportWidth),
      )
      const availableWidth = Math.max(0, availableRight - availableLeft)
      const flowContainerLeft = availableWidth > 0 ? availableLeft : 0
      const flowContainerWidth = availableWidth || viewportWidth
      const width = Math.min(flowContainerWidth, maxWidth)
      const flowLeft = flowContainerLeft + Math.round((flowContainerWidth - width) / 2)
      const offset = Math.round(anchor.getBoundingClientRect().left - flowLeft)
      setFlowMetrics(current =>
        current.offset === offset && current.width === width ? current : {offset, width},
      )
    }

    updateFlowMetrics()

    const resizeObserver =
      typeof ResizeObserver === "undefined" ? undefined : new ResizeObserver(updateFlowMetrics)
    const flowContainer = rootElement?.closest("main")
    const observedElements = [
      rootElement?.parentElement,
      flowContainer,
      flowContainer?.parentElement,
    ]
    if (resizeObserver) {
      for (const element of observedElements) {
        if (element) {
          resizeObserver.observe(element)
        }
      }
    }

    globalThis.addEventListener("resize", updateFlowMetrics)

    return () => {
      resizeObserver?.disconnect()
      globalThis.removeEventListener("resize", updateFlowMetrics)
    }
  }, [maxWidth, rootElement])

  return {flowMetrics, rootRef}
}
