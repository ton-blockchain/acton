import type {ButtonHTMLAttributes, FC, ReactNode} from "react"

import styles from "./InlineActionButton.module.css"

interface InlineActionGroupProps {
  readonly children: ReactNode
  readonly className?: string
  readonly spacing?: "default" | "loose"
}

interface InlineActionButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  readonly variant?: "default" | "danger"
}

export const InlineActionGroup: FC<InlineActionGroupProps> = ({
  children,
  className,
  spacing = "default",
}) => (
  <span className={`${styles.group} ${spacing === "loose" ? styles.loose : ""} ${className ?? ""}`}>
    {children}
  </span>
)

export const InlineActionButton: FC<InlineActionButtonProps> = ({
  variant = "default",
  className,
  children,
  ...props
}) => (
  <button
    {...props}
    className={`${styles.button} ${variant === "danger" ? styles.danger : ""} ${className ?? ""}`}
  >
    {children}
  </button>
)
