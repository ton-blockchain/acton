import React, {type ReactNode} from "react"
import {FiInfo} from "react-icons/fi"

import styles from "./TooltipHint.module.css"

export type Placement = "bottom" | "right"

interface TooltipHintProps {
  readonly children: ReactNode
  readonly tooltipText: string
  readonly iconSize?: number
  readonly placement?: Placement
}

export const TooltipHint: React.FC<TooltipHintProps> = ({
  children,
  tooltipText,
  iconSize = 16,
  placement = "right",
}) => {
  const tooltipClasses = `${styles.tooltip} ${styles[`tooltip-${placement}`]}`

  return (
    <span className={styles.tooltipContainer}>
      {children}
      <span className={styles.iconWrapper} role="button" tabIndex={0}>
        <FiInfo size={iconSize} aria-hidden="true" />
        <span className={tooltipClasses} role="tooltip">
          {tooltipText}
        </span>
      </span>
    </span>
  )
}
