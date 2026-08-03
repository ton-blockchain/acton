import {ChevronRight} from "lucide-react"
import type {ComponentPropsWithRef, ReactNode} from "react"

import {cx} from "../../lib/cx"
import styles from "./Disclosure.module.css"

export type DisclosureProps = Readonly<
  Omit<ComponentPropsWithRef<"details">, "children"> & {
    readonly children: ReactNode
    readonly contentClassName?: string
    readonly description?: ReactNode
    readonly label: ReactNode
  }
>

export function Disclosure({
  children,
  className,
  contentClassName,
  description,
  label,
  ref,
  ...props
}: DisclosureProps) {
  return (
    <details {...props} ref={ref} className={cx(styles.disclosure, className)}>
      <summary className={styles.summary}>
        <ChevronRight className={styles.icon} size={16} aria-hidden="true" />
        <span className={styles.summaryBody}>
          <span className={styles.label}>{label}</span>
          {description ? <span className={styles.description}>{description}</span> : null}
        </span>
      </summary>
      <div className={cx(styles.content, contentClassName)}>{children}</div>
    </details>
  )
}
