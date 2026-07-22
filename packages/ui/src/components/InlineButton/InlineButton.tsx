import {Check, Copy} from "lucide-react"
import type {ComponentPropsWithRef, MouseEvent, ReactNode} from "react"

import {cx} from "../../lib/cx"
import {useCopyValue} from "../../lib/useCopyValue"
import styles from "./InlineButton.module.css"
import type {ACTON_INLINE_BUTTON_VARIANTS} from "./constants"

export type ActonInlineButtonVariant = keyof typeof ACTON_INLINE_BUTTON_VARIANTS.variant

export type InlineButtonProps = Readonly<
  ComponentPropsWithRef<"button"> & {
    readonly variant?: ActonInlineButtonVariant
    readonly leadingIcon?: ReactNode
    readonly trailingIcon?: ReactNode
  }
>

export type CopyInlineButtonProps = Readonly<
  Omit<
    InlineButtonProps,
    "aria-label" | "children" | "leadingIcon" | "onClick" | "title" | "type"
  > & {
    readonly children: ReactNode
    readonly copiedChildren?: ReactNode
    readonly copiedLabel?: string
    readonly label?: string
    readonly onCopy?: (value: string) => Promise<void> | void
    readonly onCopyError?: (error: unknown) => void
    readonly resetDelay?: number
    readonly stopPropagation?: boolean
    readonly value: string
  }
>

const variantClassNames = {
  default: styles.variantDefault,
  utility: styles.variantUtility,
  accent: styles.variantAccent,
  danger: styles.variantDanger,
} satisfies Record<ActonInlineButtonVariant, string>

export function InlineButton({
  children,
  className,
  leadingIcon,
  ref,
  trailingIcon,
  type = "button",
  variant = "default",
  ...props
}: InlineButtonProps) {
  const hasChildren = children !== undefined && children !== null

  return (
    <button
      {...props}
      ref={ref}
      type={type}
      className={cx(styles.inlineButton, variantClassNames[variant], className)}
    >
      <span className={styles.content}>
        {leadingIcon ? (
          <span className={styles.icon} aria-hidden="true">
            {leadingIcon}
          </span>
        ) : undefined}
        {hasChildren ? <span className={styles.label}>{children}</span> : undefined}
        {trailingIcon ? (
          <span className={styles.icon} aria-hidden="true">
            {trailingIcon}
          </span>
        ) : undefined}
      </span>
    </button>
  )
}

export function CopyInlineButton({
  children,
  copiedChildren = "Copied",
  copiedLabel = "Copied",
  label = "Copy",
  onCopy,
  onCopyError,
  resetDelay = 2000,
  stopPropagation = true,
  value,
  variant = "utility",
  ...props
}: CopyInlineButtonProps) {
  const {copy, isCopied} = useCopyValue({onCopy, onCopyError, resetDelay, value})
  const currentLabel = isCopied ? copiedLabel : label

  const handleClick = async (event: MouseEvent<HTMLButtonElement>) => {
    if (stopPropagation) event.stopPropagation()
    await copy()
  }

  return (
    <InlineButton
      {...props}
      variant={variant}
      aria-label={currentLabel}
      title={currentLabel}
      leadingIcon={isCopied ? <Check /> : <Copy />}
      onClick={event => void handleClick(event)}
    >
      {isCopied ? copiedChildren : children}
    </InlineButton>
  )
}
