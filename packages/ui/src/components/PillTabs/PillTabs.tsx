import {ChevronDown, ChevronUp} from "lucide-react"
import type {ComponentPropsWithRef, ReactNode} from "react"

import {cx} from "../../lib/cx"
import styles from "./PillTabs.module.css"

export type PillTabVariant = "default" | "group" | "muted"

export type PillTabsProps = Readonly<
  ComponentPropsWithRef<"div"> & {
    readonly ariaLabel?: string
  }
>

export type PillTabProps = Readonly<
  Omit<ComponentPropsWithRef<"button">, "aria-current"> & {
    readonly selected?: boolean
    readonly variant?: PillTabVariant
  }
>

export type PillTabToggleProps = Readonly<
  Omit<PillTabProps, "children" | "selected" | "variant"> & {
    readonly children: ReactNode
    readonly expanded: boolean
  }
>

const variantClassNames = {
  default: styles.variantDefault,
  group: styles.variantGroup,
  muted: styles.variantMuted,
} satisfies Record<PillTabVariant, string>

export function PillTabs({
  ariaLabel,
  children,
  className,
  ref,
  role = "group",
  ...props
}: PillTabsProps) {
  return (
    <div
      {...props}
      ref={ref}
      role={role}
      aria-label={ariaLabel}
      className={cx(styles.pillTabs, className)}
    >
      {children}
    </div>
  )
}

export function PillTab({
  children,
  className,
  ref,
  selected = false,
  type = "button",
  variant = "default",
  ...props
}: PillTabProps) {
  return (
    <button
      {...props}
      ref={ref}
      type={type}
      aria-current={selected ? "true" : undefined}
      className={cx(
        styles.pillTab,
        variantClassNames[variant],
        selected && styles.selected,
        className,
      )}
    >
      {children}
    </button>
  )
}

export function PillTabToggle({children, expanded, ...props}: PillTabToggleProps) {
  return (
    <PillTab {...props} variant="group" aria-expanded={expanded}>
      <span className={styles.toggleIcon} aria-hidden="true">
        {expanded ? <ChevronUp /> : <ChevronDown />}
      </span>
      <span className={styles.label}>{children}</span>
    </PillTab>
  )
}
