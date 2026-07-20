import type {ComponentPropsWithRef, ReactNode} from "react"

import {cx} from "../../lib/cx"
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
