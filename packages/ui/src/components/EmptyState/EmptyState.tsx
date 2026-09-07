import type {ComponentPropsWithoutRef, ReactNode} from "react"

import {cx} from "../../lib/cx"

import styles from "./EmptyState.module.css"

export type EmptyStateProps = Readonly<
  Omit<ComponentPropsWithoutRef<"div">, "children" | "title"> & {
    readonly action?: ReactNode
    readonly description?: ReactNode
    readonly icon?: ReactNode
    readonly title: ReactNode
    readonly variant?: "default" | "error"
  }
>

/**
 * Explains why a resource view is empty and offers the next useful action when one exists.
 * Feature code owns the domain copy so search results, initial states, and failures stay distinct.
 */
export function EmptyState({
  action,
  className,
  description,
  icon,
  title,
  variant = "default",
  ...props
}: EmptyStateProps) {
  return (
    <div {...props} className={cx(styles.emptyState, className)} data-variant={variant}>
      {icon ? <span className={styles.icon}>{icon}</span> : undefined}
      <strong className={styles.title}>{title}</strong>
      {description ? <span className={styles.description}>{description}</span> : undefined}
      {action ? <div className={styles.action}>{action}</div> : undefined}
    </div>
  )
}
