import {Moon, Sun} from "lucide-react"
import type {ComponentPropsWithRef} from "react"

import {cx} from "../../lib/cx"
import {useTheme, type ThemeMode} from "../Theme/ThemeProvider"
import styles from "./ThemeSwitch.module.css"

export type ThemeSwitchProps = Readonly<
  Omit<ComponentPropsWithRef<"button">, "children" | "onClick" | "type"> & {
    readonly onToggleTheme?: () => void
    readonly theme?: ThemeMode
  }
>

export function ThemeSwitch({
  "aria-label": ariaLabel,
  className,
  onToggleTheme,
  ref,
  theme,
  ...props
}: ThemeSwitchProps) {
  const themeContext = useTheme()
  const resolvedTheme = theme ?? themeContext.theme
  const handleToggleTheme = onToggleTheme ?? themeContext.toggleTheme

  return (
    <button
      {...props}
      ref={ref}
      type="button"
      className={cx(styles.themeSwitch, className)}
      aria-label={ariaLabel ?? `Use ${resolvedTheme === "dark" ? "light" : "dark"} theme`}
      data-theme-toggle=""
      onClick={handleToggleTheme}
    >
      <Sun
        aria-hidden="true"
        fill="currentColor"
        className={cx(
          styles.themeSwitchItem,
          resolvedTheme === "light" && styles.themeSwitchItemActive,
        )}
      />
      <Moon
        aria-hidden="true"
        fill="currentColor"
        className={cx(
          styles.themeSwitchItem,
          resolvedTheme === "dark" && styles.themeSwitchItemActive,
        )}
      />
    </button>
  )
}
