import type {ComponentPropsWithRef, ReactNode} from "react"

import {cx} from "../../lib/cx"
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
