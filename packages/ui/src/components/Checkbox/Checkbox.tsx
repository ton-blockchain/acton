import type {ComponentPropsWithRef, ReactNode} from "react"

import {cx} from "../../lib/cx"
import styles from "./Checkbox.module.css"

export type CheckboxProps = Readonly<
  Omit<ComponentPropsWithRef<"input">, "children" | "className" | "type"> & {
    readonly className?: string
    readonly label: ReactNode
    readonly count?: ReactNode
    readonly description?: ReactNode
  }
>

export function Checkbox({
  className,
  count,
  description,
  disabled,
  label,
  ref,
  ...props
}: CheckboxProps) {
  const hasCount = count !== undefined && count !== null

  return (
    <label className={cx(styles.checkbox, disabled && styles.disabled, className)}>
      <input {...props} ref={ref} type="checkbox" disabled={disabled} className={styles.input} />
      <span className={styles.control} aria-hidden="true" />
      <span className={styles.body}>
        <span className={styles.labelLine}>
          <span className={styles.label}>{label}</span>
          {hasCount ? <span className={styles.count}>{count}</span> : undefined}
        </span>
        {description ? <span className={styles.description}>{description}</span> : undefined}
      </span>
    </label>
  )
}
