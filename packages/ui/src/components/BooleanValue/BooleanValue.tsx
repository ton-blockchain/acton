import type {HTMLAttributes, ReactNode} from "react"

import {cx} from "../../lib/cx"
import styles from "./BooleanValue.module.css"

export type BooleanValueDisplay = "true-false" | "yes-no"

export interface BooleanValueProps
  extends Omit<HTMLAttributes<HTMLDataElement>, "children" | "value"> {
  readonly display?: BooleanValueDisplay
  readonly falseLabel?: string
  readonly fallback?: ReactNode
  readonly trueLabel?: string
  readonly value: boolean | null | undefined
}

export function BooleanValue({
  value,
  display = "yes-no",
  falseLabel,
  fallback = "—",
  trueLabel,
  className,
  ...props
}: BooleanValueProps) {
  if (value === null || value === undefined) return fallback

  const defaultTrueLabel = display === "yes-no" ? "Yes" : "true"
  const defaultFalseLabel = display === "yes-no" ? "No" : "false"
  return (
    <data
      data-visual-dynamic="boolean"
      data-visual-placeholder="<boolean>"
      {...props}
      className={cx(styles.value, value ? styles.trueValue : styles.falseValue, className)}
      value={String(value)}
    >
      {value ? (trueLabel ?? defaultTrueLabel) : (falseLabel ?? defaultFalseLabel)}
    </data>
  )
}
