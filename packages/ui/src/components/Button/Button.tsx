import {Check, Copy} from "lucide-react"
import type {ComponentPropsWithRef, MouseEvent, ReactNode} from "react"

import {cx} from "../../lib/cx"
import {useCopyValue} from "../../lib/useCopyValue"
import styles from "./Button.module.css"
import type {ACTON_BUTTON_VARIANTS} from "./constants"

export type ActonButtonVariant = keyof typeof ACTON_BUTTON_VARIANTS.variant
export type ActonButtonSize = keyof typeof ACTON_BUTTON_VARIANTS.size

export type ButtonProps = Readonly<
  ComponentPropsWithRef<"button"> & {
    readonly variant?: ActonButtonVariant
    readonly size?: ActonButtonSize
    readonly leadingIcon?: ReactNode
    readonly trailingIcon?: ReactNode
    readonly loading?: boolean
  }
>

export type CopyButtonProps = Readonly<
  Omit<ButtonProps, "aria-label" | "children" | "leadingIcon" | "onClick" | "title" | "type"> & {
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
  primary: styles.variantPrimary,
  secondary: styles.variantSecondary,
  outline: styles.variantOutline,
  ghost: styles.variantGhost,
  danger: styles.variantDanger,
} satisfies Record<ActonButtonVariant, string>

const sizeClassNames = {
  sm: styles.sizeSm,
  md: styles.sizeMd,
  lg: styles.sizeLg,
  icon: styles.sizeIcon,
} satisfies Record<ActonButtonSize, string>

export function Button({
  children,
  className,
  disabled,
  leadingIcon,
  loading = false,
  ref,
  size = "md",
  trailingIcon,
  type = "button",
  variant = "secondary",
  ...props
}: ButtonProps) {
  const isDisabled = disabled || loading
  const hasChildren = children !== undefined && children !== null
  const leadingContent = loading ? (
    <span className={styles.spinner} aria-hidden="true" />
  ) : leadingIcon ? (
    <span className={styles.icon} aria-hidden="true">
      {leadingIcon}
    </span>
  ) : undefined

  return (
    <button
      {...props}
      ref={ref}
      type={type}
      disabled={isDisabled}
      aria-busy={loading || undefined}
      className={cx(styles.button, variantClassNames[variant], sizeClassNames[size], className)}
    >
      <span className={styles.content}>
        {leadingContent}
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

export function CopyButton({
  children,
  copiedChildren = "Copied",
  copiedLabel = "Copied",
  label = "Copy",
  onCopy,
  onCopyError,
  resetDelay = 2000,
  stopPropagation = true,
  value,
  variant = "secondary",
  ...props
}: CopyButtonProps) {
  const {copy, isCopied} = useCopyValue({onCopy, onCopyError, resetDelay, value})
  const currentLabel = isCopied ? copiedLabel : label

  const handleClick = async (event: MouseEvent<HTMLButtonElement>) => {
    if (stopPropagation) event.stopPropagation()
    await copy()
  }

  return (
    <Button
      {...props}
      variant={variant}
      aria-label={currentLabel}
      title={currentLabel}
      leadingIcon={isCopied ? <Check size={16} /> : <Copy size={16} />}
      onClick={event => void handleClick(event)}
    >
      {isCopied ? copiedChildren : children}
    </Button>
  )
}
