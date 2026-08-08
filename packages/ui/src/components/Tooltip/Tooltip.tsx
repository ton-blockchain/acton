import {Tooltip as TooltipBase} from "@base-ui/react/tooltip"
import type {ReactElement, ReactNode} from "react"

import styles from "./Tooltip.module.css"

export type TooltipPlacement = "top" | "right" | "bottom" | "left"

export interface TooltipProps {
  readonly children: ReactElement
  readonly closeDelay?: number
  readonly content: ReactNode
  readonly delay?: number
  readonly disabled?: boolean
  readonly offset?: number
  readonly placement?: TooltipPlacement
  readonly width?: "default" | "wide" | "extra-wide"
}

export function Tooltip({
  children,
  closeDelay = 80,
  content,
  delay = 450,
  disabled = false,
  offset = 8,
  placement = "top",
  width = "default",
}: TooltipProps) {
  const hasContent =
    content !== null &&
    content !== undefined &&
    typeof content !== "boolean" &&
    (typeof content !== "string" || content.trim().length > 0)

  if (!hasContent) return children

  return (
    <TooltipBase.Root disabled={disabled}>
      <TooltipBase.Trigger render={children} delay={delay} closeDelay={closeDelay} />
      <TooltipBase.Portal>
        <TooltipBase.Positioner
          className={styles.positioner}
          data-width={width}
          side={placement}
          sideOffset={offset}
        >
          <TooltipBase.Popup className={styles.popup} data-width={width}>
            {content}
          </TooltipBase.Popup>
        </TooltipBase.Positioner>
      </TooltipBase.Portal>
    </TooltipBase.Root>
  )
}
