import {Info} from "lucide-react"
import {createPortal} from "react-dom"
import {
  type CSSProperties,
  type JSX,
  type ReactNode,
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react"

import styles from "./InfoPopover.module.css"

type InfoPopoverPlacement = "right" | "left" | "bottom" | "top"

interface RectSnapshot {
  readonly left: number
  readonly top: number
  readonly right: number
  readonly bottom: number
  readonly width: number
  readonly height: number
}

interface InfoPopoverPosition {
  readonly left: number
  readonly top: number
  readonly placement: InfoPopoverPlacement
  readonly arrowX: number
  readonly arrowY: number
}

interface InfoPopoverProps {
  readonly id: string
  readonly children: ReactNode
  readonly ariaLabel?: string
}

const INFO_POPOVER_MARGIN = 12
const INFO_POPOVER_GAP = 12
const INFO_POPOVER_ARROW_MIN = 16

export function InfoPopover({
  id,
  children,
  ariaLabel = "Show information",
}: InfoPopoverProps): JSX.Element {
  const triggerRef = useRef<HTMLButtonElement | null>(null)
  const popoverRef = useRef<HTMLSpanElement | null>(null)
  const closeTimerRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)
  const [isOpen, setIsOpen] = useState(false)
  const [triggerRect, setTriggerRect] = useState<RectSnapshot | undefined>()
  const [position, setPosition] = useState<InfoPopoverPosition | undefined>()

  const clearCloseTimer = useCallback((): void => {
    if (closeTimerRef.current) {
      clearTimeout(closeTimerRef.current)
      closeTimerRef.current = undefined
    }
  }, [])

  const openPopover = useCallback((): void => {
    clearCloseTimer()

    if (isOpen) {
      return
    }

    const rect = triggerRef.current?.getBoundingClientRect()
    if (rect) {
      setTriggerRect(snapshotRect(rect))
      setPosition(undefined)
    }
    setIsOpen(true)
  }, [clearCloseTimer, isOpen])

  const closePopover = useCallback((): void => {
    clearCloseTimer()

    closeTimerRef.current = setTimeout(() => {
      setIsOpen(false)
      setPosition(undefined)
    }, 120)
  }, [clearCloseTimer])

  const forceClosePopover = useCallback((): void => {
    clearCloseTimer()
    setIsOpen(false)
    setPosition(undefined)
  }, [clearCloseTimer])

  useLayoutEffect(() => {
    if (!isOpen || !triggerRect || !popoverRef.current) return

    const popoverRect = popoverRef.current.getBoundingClientRect()
    setPosition(calculateInfoPopoverPosition(triggerRect, popoverRect.width, popoverRect.height))
  }, [isOpen, triggerRect, children])

  useEffect(() => {
    if (!isOpen) return

    const updateTriggerRect = (): void => {
      const rect = triggerRef.current?.getBoundingClientRect()
      if (rect) {
        setTriggerRect(snapshotRect(rect))
      }
    }

    const handlePointerDown = (event: MouseEvent): void => {
      const target = event.target as Node
      if (triggerRef.current?.contains(target) || popoverRef.current?.contains(target)) {
        return
      }
      forceClosePopover()
    }

    const handleKeyDown = (event: KeyboardEvent): void => {
      if (event.key === "Escape") {
        forceClosePopover()
      }
    }

    window.addEventListener("resize", updateTriggerRect)
    window.addEventListener("scroll", updateTriggerRect, true)
    document.addEventListener("mousedown", handlePointerDown)
    document.addEventListener("keydown", handleKeyDown)

    return () => {
      window.removeEventListener("resize", updateTriggerRect)
      window.removeEventListener("scroll", updateTriggerRect, true)
      document.removeEventListener("mousedown", handlePointerDown)
      document.removeEventListener("keydown", handleKeyDown)
    }
  }, [forceClosePopover, isOpen])

  useEffect(() => {
    return () => {
      if (closeTimerRef.current) {
        clearTimeout(closeTimerRef.current)
      }
    }
  }, [])

  const popoverStyle = {
    left: position?.left ?? INFO_POPOVER_MARGIN,
    top: position?.top ?? INFO_POPOVER_MARGIN,
    "--info-popover-arrow-x": `${position?.arrowX ?? INFO_POPOVER_ARROW_MIN}px`,
    "--info-popover-arrow-y": `${position?.arrowY ?? INFO_POPOVER_ARROW_MIN}px`,
  } as CSSProperties

  return (
    <span className={styles.infoPopover}>
      <button
        ref={triggerRef}
        type="button"
        className={styles.infoPopoverButton}
        aria-label={ariaLabel}
        aria-describedby={isOpen ? id : undefined}
        aria-expanded={isOpen}
        onMouseEnter={openPopover}
        onMouseLeave={closePopover}
        onFocus={openPopover}
        onBlur={closePopover}
        onClick={() => {
          if (isOpen) {
            forceClosePopover()
          } else {
            openPopover()
          }
        }}
      >
        <Info size={12} />
      </button>
      {isOpen &&
        createPortal(
          <span
            ref={popoverRef}
            id={id}
            className={styles.infoPopoverPanel}
            data-placement={position?.placement ?? "right"}
            data-positioned={position ? "true" : "false"}
            role="tooltip"
            style={popoverStyle}
            onMouseEnter={clearCloseTimer}
            onMouseLeave={closePopover}
          >
            <span className={styles.infoPopoverContent}>{children}</span>
          </span>,
          document.body,
        )}
    </span>
  )
}

function snapshotRect(rect: DOMRect): RectSnapshot {
  return {
    left: rect.left,
    top: rect.top,
    right: rect.right,
    bottom: rect.bottom,
    width: rect.width,
    height: rect.height,
  }
}

function calculateInfoPopoverPosition(
  triggerRect: RectSnapshot,
  popoverWidth: number,
  popoverHeight: number,
): InfoPopoverPosition {
  const viewportWidth = window.innerWidth
  const viewportHeight = window.innerHeight
  const triggerCenterX = triggerRect.left + triggerRect.width / 2
  const triggerCenterY = triggerRect.top + triggerRect.height / 2

  const candidates: Array<{
    readonly placement: InfoPopoverPlacement
    readonly left: number
    readonly top: number
    readonly preference: number
  }> = [
    {
      placement: "right",
      left: triggerRect.right + INFO_POPOVER_GAP,
      top: triggerCenterY - popoverHeight / 2,
      preference: 0,
    },
    {
      placement: "left",
      left: triggerRect.left - popoverWidth - INFO_POPOVER_GAP,
      top: triggerCenterY - popoverHeight / 2,
      preference: 1,
    },
    {
      placement: "bottom",
      left: triggerCenterX - popoverWidth / 2,
      top: triggerRect.bottom + INFO_POPOVER_GAP,
      preference: 2,
    },
    {
      placement: "top",
      left: triggerCenterX - popoverWidth / 2,
      top: triggerRect.top - popoverHeight - INFO_POPOVER_GAP,
      preference: 3,
    },
  ]

  const best = candidates
    .map(candidate => {
      const horizontalOverflow =
        Math.max(INFO_POPOVER_MARGIN - candidate.left, 0) +
        Math.max(candidate.left + popoverWidth - (viewportWidth - INFO_POPOVER_MARGIN), 0)
      const verticalOverflow =
        Math.max(INFO_POPOVER_MARGIN - candidate.top, 0) +
        Math.max(candidate.top + popoverHeight - (viewportHeight - INFO_POPOVER_MARGIN), 0)

      return {
        ...candidate,
        score: horizontalOverflow * 2 + verticalOverflow * 2 + candidate.preference,
      }
    })
    .sort((a, b) => a.score - b.score)[0]

  const left = clamp(
    best.left,
    INFO_POPOVER_MARGIN,
    viewportWidth - popoverWidth - INFO_POPOVER_MARGIN,
  )
  const top = clamp(
    best.top,
    INFO_POPOVER_MARGIN,
    viewportHeight - popoverHeight - INFO_POPOVER_MARGIN,
  )

  return {
    left,
    top,
    placement: best.placement,
    arrowX: clamp(
      triggerCenterX - left,
      INFO_POPOVER_ARROW_MIN,
      popoverWidth - INFO_POPOVER_ARROW_MIN,
    ),
    arrowY: clamp(
      triggerCenterY - top,
      INFO_POPOVER_ARROW_MIN,
      popoverHeight - INFO_POPOVER_ARROW_MIN,
    ),
  }
}

function clamp(value: number, min: number, max: number): number {
  if (max < min) {
    return min
  }

  return Math.min(Math.max(value, min), max)
}
