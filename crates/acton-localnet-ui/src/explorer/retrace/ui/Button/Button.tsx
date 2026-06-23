import React, {type ButtonHTMLAttributes} from "react"

import styles from "./Button.module.css"

export type ButtonVariant = "primary" | "ghost" | "outline"
export type ButtonSize = "sm" | "md" | "lg"

// eslint-disable-next-line functional/type-declaration-immutability
interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  readonly variant?: ButtonVariant
  readonly size?: ButtonSize
  readonly className?: string
  readonly children?: React.ReactNode
}

const Button: React.FC<ButtonProps> = ({
  variant = "primary",
  size = "md",
  className = "",
  children,
  ...rest
}) => {
  const buttonClasses = [styles.btn, styles[`btn-${variant}`], styles[`btn-${size}`], className]
    .filter(Boolean)
    .join(" ")

  return (
    <button className={buttonClasses} {...rest}>
      {children}
    </button>
  )
}

export default Button
