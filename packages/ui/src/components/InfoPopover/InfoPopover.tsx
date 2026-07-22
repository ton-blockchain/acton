import {Info} from "lucide-react"
import type {ReactNode} from "react"

import {cx} from "../../lib/cx"
import {Popover, type PopoverProps} from "../Popover"
import styles from "./InfoPopover.module.css"

export type InfoPopoverProps = Readonly<
  Omit<
    PopoverProps,
    "aria-label" | "ariaLabel" | "children" | "content" | "panelId" | "triggerClassName"
  > & {
    readonly ariaLabel?: string
    readonly children: ReactNode
    readonly id?: string
  }
>

export function InfoPopover({
  ariaLabel = "Show information",
  children,
  className,
  contentClassName,
  id,
  offset = 8,
  placement = "right",
  ...props
}: InfoPopoverProps) {
  return (
    <Popover
      {...props}
      aria-label={ariaLabel}
      ariaLabel={ariaLabel}
      className={cx(styles.infoPopover, className)}
      content={<div className={styles.content}>{children}</div>}
      contentClassName={cx(styles.panel, contentClassName)}
      offset={offset}
      panelId={id}
      placement={placement}
      triggerClassName={styles.trigger}
    >
      <span className={styles.button} aria-hidden="true">
        <Info size={12} strokeWidth={2.4} />
      </span>
    </Popover>
  )
}
