import type {ComponentPropsWithRef, ReactNode} from "react"

import {cx} from "../../lib/cx"
import {shortenMiddle} from "../../lib/formatting"
import {CopyInlineAction, InlineActions, type ActonInlineActionsVisibility} from "../InlineActions"
import {Tooltip} from "../Tooltip"
import styles from "./TechnicalValue.module.css"

export type TechnicalValueProps = Readonly<
  Omit<ComponentPropsWithRef<"span">, "children"> & {
    readonly copyable?: boolean
    readonly copyLabel?: string
    readonly copyVisibility?: ActonInlineActionsVisibility
    readonly displayValue?: string
    readonly endLength?: number
    readonly fallback?: ReactNode
    readonly shorten?: boolean
    readonly startLength?: number
    readonly tooltip?: boolean
    readonly value: string | null | undefined
  }
>

export function TechnicalValue({
  value,
  copyable = true,
  copyLabel = "technical value",
  copyVisibility = "hover",
  displayValue,
  endLength,
  fallback = "—",
  shorten = true,
  startLength = 8,
  tooltip = true,
  className,
  ref,
  ...props
}: TechnicalValueProps) {
  const normalized = value?.trim()
  if (!normalized) return fallback

  const visibleValue =
    displayValue ??
    (shorten
      ? shortenMiddle(normalized, {start: startLength, end: endLength ?? startLength})
      : normalized)
  const code = <code className={styles.code}>{visibleValue}</code>
  const content = tooltip ? (
    <Tooltip
      content={
        <span className={styles.tooltipContent}>
          <code className={styles.tooltipCode}>{normalized}</code>
          {copyable ? (
            <CopyInlineAction
              copiedLabel={`${copyLabel} copied`}
              label={`Copy ${copyLabel}`}
              size="compact"
              value={normalized}
            />
          ) : null}
        </span>
      }
      width="extra-wide"
    >
      {code}
    </Tooltip>
  ) : (
    code
  )

  return (
    <InlineActions
      data-visual-dynamic="technical-value"
      data-visual-placeholder="<value>"
      {...props}
      ref={ref}
      className={cx(styles.value, className)}
      visibility={copyVisibility}
      actions={
        copyable ? (
          <CopyInlineAction
            copiedLabel={`${copyLabel} copied`}
            label={`Copy ${copyLabel}`}
            size="compact"
            value={normalized}
          />
        ) : undefined
      }
    >
      {content}
    </InlineActions>
  )
}
