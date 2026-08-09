import {ChevronDown, ChevronUp} from "lucide-react"
import type {ComponentPropsWithRef, ReactNode} from "react"

import {cx} from "../../lib/cx"
import styles from "./DisclosureToggle.module.css"

export type DisclosureToggleProps = Readonly<
  Omit<ComponentPropsWithRef<"button">, "aria-expanded" | "children"> & {
    readonly contextLabel?: string
    readonly expanded: boolean
    readonly hideLabel?: ReactNode
    readonly loading?: boolean
    readonly loadingLabel?: ReactNode
    readonly showLabel?: ReactNode
  }
>

function getGeneratedAriaLabel(label: ReactNode, contextLabel: string | undefined) {
  if (!contextLabel || typeof label !== "string") return undefined
  return `${label.toLowerCase()} ${contextLabel}`
}

export function DisclosureToggle({
  "aria-label": ariaLabel,
  className,
  contextLabel,
  disabled,
  expanded,
  hideLabel = "Hide",
  loading = false,
  loadingLabel = "Loading",
  ref,
  showLabel = "Show",
  type = "button",
  ...props
}: DisclosureToggleProps) {
  const isDisabled = disabled || loading
  const stateLabel = loading ? loadingLabel : expanded ? hideLabel : showLabel
  const Icon = expanded ? ChevronUp : ChevronDown

  return (
    <button
      {...props}
      ref={ref}
      type={type}
      disabled={isDisabled}
      aria-busy={loading || undefined}
      aria-expanded={expanded}
      aria-label={ariaLabel ?? getGeneratedAriaLabel(stateLabel, contextLabel)}
      data-loading={loading || undefined}
      className={cx(styles.disclosureToggle, className)}
    >
      <span className={styles.icon} aria-hidden="true">
        <Icon />
      </span>
      <span className={styles.label}>{stateLabel}</span>
    </button>
  )
}
