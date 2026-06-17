import * as React from "react"
import {clsx} from "clsx"

import styles from "./Button.module.css"

export type ButtonProps = Readonly<
  React.ButtonHTMLAttributes<HTMLButtonElement> & {
    readonly variant?: "default" | "outline" | "secondary" | "ghost"
    readonly size?: "default" | "sm" | "lg" | "icon"
  }
>

export const Button: React.FC<ButtonProps> = ({
  className,
  variant = "default",
  size = "default",
  type = "button",
  ...properties
}) => {
  const variantClassName =
    variant === "outline"
      ? styles.variantOutline
      : variant === "secondary"
        ? styles.variantSecondary
        : variant === "ghost"
          ? styles.variantGhost
          : styles.variantDefault

  const sizeClassName =
    size === "sm"
      ? styles.sizeSm
      : size === "lg"
        ? styles.sizeLg
        : size === "icon"
          ? styles.sizeIcon
          : styles.sizeDefault

  return (
    <button
      type={type}
      className={clsx(styles.button, variantClassName, sizeClassName, className)}
      {...properties}
    />
  )
}
