import {Check, Copy} from "lucide-react"
import type {ComponentPropsWithRef, MouseEvent, ReactNode} from "react"

import {cx} from "../../lib/cx"
import {useCopyValue} from "../../lib/useCopyValue"
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
    readonly copiedIcon?: ReactNode
    readonly copiedLabel?: string
    readonly icon?: ReactNode
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
  copiedIcon = <Check />,
  copiedLabel = "Copied",
  icon = <Copy />,
  label = "Copy",
  onCopy,
  onCopyError,
  resetDelay = 2000,
  stopPropagation = true,
  value,
  ...props
}: CopyInlineActionProps) {
  const {copy, isCopied} = useCopyValue({onCopy, onCopyError, resetDelay, value})
  const currentLabel = isCopied ? copiedLabel : label

  const handleClick = async (event: MouseEvent<HTMLButtonElement>) => {
    if (stopPropagation) event.stopPropagation()
    await copy()
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
