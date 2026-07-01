import {clsx} from "clsx"
import {Moon, Sun} from "lucide-react"
import * as React from "react"

import styles from "./ThemeSwitch.module.css"

export type ThemeMode = "light" | "dark"

export type ThemeSwitchProps = Readonly<
  Omit<React.ButtonHTMLAttributes<HTMLButtonElement>, "children" | "onClick" | "type"> & {
    readonly theme: ThemeMode
    readonly onToggleTheme: () => void
  }
>

export const ThemeSwitch: React.FC<ThemeSwitchProps> = ({
  theme,
  onToggleTheme,
  className,
  "aria-label": ariaLabel = "Toggle Theme",
  ...properties
}) => {
  return (
    <button
      type="button"
      className={clsx(styles.themeSwitch, className)}
      aria-label={ariaLabel}
      data-theme-toggle=""
      onClick={onToggleTheme}
      {...properties}
    >
      <Sun
        fill="currentColor"
        className={clsx(styles.themeSwitchItem, theme === "light" && styles.themeSwitchItemActive)}
      />
      <Moon
        fill="currentColor"
        className={clsx(styles.themeSwitchItem, theme === "dark" && styles.themeSwitchItemActive)}
      />
    </button>
  )
}
