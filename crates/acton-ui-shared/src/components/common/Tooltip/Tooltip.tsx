import { type ReactNode, useMemo, useState } from "react"
import styles from "./Tooltip.module.css"

interface TooltipProps {
  readonly children: ReactNode
  readonly content: ReactNode | string
  readonly variant?: "hover" | "positioned"
  readonly position?: { x: number; y: number }
  readonly placement?: "top" | "bottom" | "right"
  readonly className?: string
}

function renderContent(content: ReactNode | string) {
  return content
}

export function Tooltip({
  children,
  content,
  variant = "hover",
  position,
  placement = "top",
  className,
}: TooltipProps) {
  const [isVisible, setIsVisible] = useState(false)

  const tooltipClassName = useMemo(() => {
    if (placement === "bottom") return `${styles.tooltip} ${styles.bottom}`
    if (placement === "right") return `${styles.tooltip} ${styles.right}`
    return styles.tooltip
  }, [placement])

  if (variant === "positioned" && position) {
    return (
      <div
        className={styles.tooltipPositioned}
        style={{
          left: Math.max(10, Math.min(position.x - 80, window.innerWidth - 280)),
          top: Math.max(10, position.y - 265),
        }}
      >
        {renderContent(content)}
      </div>
    )
  }

  return (
    <div
      className={`${styles.triggerContainer} ${className ?? ""}`.trim()}
      onMouseEnter={() => setIsVisible(true)}
      onMouseLeave={() => setIsVisible(false)}
      role="button"
      tabIndex={0}
    >
      {children}
      {isVisible && <div className={tooltipClassName}>{renderContent(content)}</div>}
    </div>
  )
}
