import {X} from "lucide-react"
import type {ComponentPropsWithoutRef, ReactNode} from "react"

import {cx} from "../../lib/cx"
import {Tooltip} from "../Tooltip"
import styles from "./Banner.module.css"

export type BannerProps = Readonly<
  Omit<ComponentPropsWithoutRef<"aside">, "children" | "title"> & {
    readonly action?: ReactNode
    readonly compactTitle?: ReactNode
    readonly description?: ReactNode
    readonly dismissLabel?: string
    readonly icon?: ReactNode
    readonly onDismiss?: () => void
    readonly title: ReactNode
  }
>

/**
 * Displays a full-width product announcement with optional responsive copy,
 * action, and dismiss control. Persistence and domain-specific content belong
 * to the consuming application.
 */
export function Banner({
  action,
  className,
  compactTitle,
  description,
  dismissLabel = "Dismiss banner",
  icon,
  onDismiss,
  title,
  ...props
}: BannerProps) {
  const hasDescription = description !== undefined && description !== null && description !== false
  const hasAction = action !== undefined && action !== null && action !== false

  return (
    <aside {...props} className={cx(styles.banner, className)}>
      <div className={styles.inner}>
        <p className={styles.message}>
          {icon ? (
            <span className={styles.icon} aria-hidden="true">
              {icon}
            </span>
          ) : undefined}
          <strong className={styles.title}>
            <span className={compactTitle ? styles.desktopTitle : undefined}>{title}</span>
            {compactTitle ? <span className={styles.compactTitle}>{compactTitle}</span> : undefined}
          </strong>
          {hasDescription ? (
            <>
              <span className={styles.separator} aria-hidden="true">
                ·
              </span>
              <span className={styles.description}>{description}</span>
            </>
          ) : undefined}
          {hasAction ? <span className={styles.action}>{action}</span> : undefined}
        </p>
        {onDismiss ? (
          <Tooltip content={dismissLabel}>
            <button
              type="button"
              className={styles.closeButton}
              aria-label={dismissLabel}
              onClick={onDismiss}
            >
              <X size={16} aria-hidden="true" />
            </button>
          </Tooltip>
        ) : undefined}
      </div>
    </aside>
  )
}
