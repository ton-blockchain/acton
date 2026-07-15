import {
  useEffect,
  useState,
  type ComponentPropsWithRef,
  type MouseEvent,
  type ReactNode,
} from "react"

import {cx} from "../../lib/cx"
import styles from "./InlineActions.module.css"
import type {ACTON_INLINE_ACTIONS_VISIBILITY, ACTON_INLINE_ACTION_VARIANTS} from "./constants"

export type ActonInlineActionVariant = keyof typeof ACTON_INLINE_ACTION_VARIANTS.variant
export type ActonInlineActionSize = keyof typeof ACTON_INLINE_ACTION_VARIANTS.size
export type ActonInlineActionsVisibility = keyof typeof ACTON_INLINE_ACTIONS_VISIBILITY.visibility

export type InlineActionProps = Readonly<
  Omit<ComponentPropsWithRef<"button">, "children"> & {
    readonly icon: ReactNode
    readonly label: string
    readonly size?: ActonInlineActionSize
    readonly variant?: ActonInlineActionVariant
  }
>

export type CopyInlineActionProps = Readonly<
  Omit<InlineActionProps, "aria-label" | "icon" | "label" | "onClick" | "title" | "type"> & {
    readonly copiedIcon: ReactNode
    readonly copiedLabel?: string
    readonly icon: ReactNode
    readonly label?: string
    readonly onCopy?: (value: string) => Promise<void> | void
    readonly onCopyError?: (error: unknown) => void
    readonly resetDelay?: number
    readonly stopPropagation?: boolean
    readonly value: string
  }
>

export type InlineActionsProps = Readonly<
  ComponentPropsWithRef<"span"> & {
    readonly actions: ReactNode
    readonly visibility?: ActonInlineActionsVisibility
  }
>

const actionVariantClassNames = {
  default: styles.actionVariantDefault,
  accent: styles.actionVariantAccent,
} satisfies Record<ActonInlineActionVariant, string>

const actionSizeClassNames = {
  default: styles.actionSizeDefault,
  compact: styles.actionSizeCompact,
} satisfies Record<ActonInlineActionSize, string>

const visibilityClassNames = {
  hover: styles.visibilityHover,
  always: styles.visibilityAlways,
} satisfies Record<ActonInlineActionsVisibility, string>

export function InlineAction({
  "aria-label": ariaLabel,
  className,
  icon,
  label,
  ref,
  size = "default",
  title,
  type = "button",
  variant = "default",
  ...props
}: InlineActionProps) {
  return (
    <button
      {...props}
      ref={ref}
      type={type}
      aria-label={ariaLabel ?? label}
      title={title ?? label}
      className={cx(
        styles.inlineAction,
        actionVariantClassNames[variant],
        actionSizeClassNames[size],
        className,
      )}
    >
      <span className={styles.actionIcon} aria-hidden="true">
        {icon}
      </span>
    </button>
  )
}

export function CopyInlineAction({
  copiedIcon,
  copiedLabel = "Copied",
  icon,
  label = "Copy",
  onCopy,
  onCopyError,
  resetDelay = 2000,
  stopPropagation = true,
  value,
  ...props
}: CopyInlineActionProps) {
  const [isCopied, setIsCopied] = useState(false)
  const currentLabel = isCopied ? copiedLabel : label

  // react-doctor-disable-next-line react-doctor/no-reset-all-state-on-prop-change -- avoids showing copied state for a new value
  useEffect(() => {
    setIsCopied(false)
  }, [value])

  useEffect(() => {
    if (!isCopied || resetDelay <= 0) return

    const timer = globalThis.setTimeout(() => setIsCopied(false), resetDelay)
    return () => globalThis.clearTimeout(timer)
  }, [isCopied, resetDelay])

  const handleClick = async (event: MouseEvent<HTMLButtonElement>) => {
    if (stopPropagation) event.stopPropagation()

    try {
      if (onCopy) {
        await onCopy(value)
      } else {
        await navigator.clipboard.writeText(value)
      }

      setIsCopied(true)
    } catch (error) {
      onCopyError?.(error)
    }
  }

  return (
    <InlineAction
      {...props}
      type="button"
      label={currentLabel}
      title={currentLabel}
      icon={isCopied ? copiedIcon : icon}
      onClick={event => void handleClick(event)}
    />
  )
}

export function InlineActions({
  actions,
  children,
  className,
  ref,
  visibility = "hover",
  ...props
}: InlineActionsProps) {
  return (
    <span
      {...props}
      ref={ref}
      className={cx(styles.inlineActions, visibilityClassNames[visibility], className)}
    >
      <span className={styles.content}>{children}</span>
      <span className={styles.actions}>{actions}</span>
    </span>
  )
}
