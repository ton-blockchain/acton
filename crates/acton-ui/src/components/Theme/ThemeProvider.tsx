import {
  createContext,
  use,
  useCallback,
  useLayoutEffect,
  useMemo,
  useState,
  type Dispatch,
  type PropsWithChildren,
  type SetStateAction,
} from "react"
import {flushSync} from "react-dom"

export type ThemeMode = "light" | "dark"
export type ThemePreference = ThemeMode | "system"

export type ThemeContextValue = Readonly<{
  readonly setTheme: Dispatch<SetStateAction<ThemeMode>>
  readonly theme: ThemeMode
  readonly toggleTheme: () => void
}>

export type ThemeProviderProps = PropsWithChildren<
  Readonly<{
    readonly defaultTheme?: ThemePreference
    readonly storageKey?: string
  }>
>

const ThemeContext = createContext<ThemeContextValue | undefined>(undefined)

export function ThemeProvider({
  children,
  defaultTheme = "system",
  storageKey = "theme",
}: ThemeProviderProps) {
  const [theme, setTheme] = useState<ThemeMode>(() => readInitialTheme(storageKey, defaultTheme))
  const toggleTheme = useCallback(() => {
    const updateTheme = () => {
      setTheme(currentTheme => (currentTheme === "light" ? "dark" : "light"))
    }

    if (!document.startViewTransition) {
      updateTheme()
      return
    }

    document.startViewTransition(() => {
      flushSync(updateTheme)
    })
  }, [])

  useLayoutEffect(() => {
    const root = document.documentElement
    const isDark = theme === "dark"

    root.dataset.theme = theme
    root.classList.toggle("dark-theme", isDark)
    document.body.classList.toggle("dark-mode", isDark)
    document.body.classList.toggle("light-mode", !isDark)

    try {
      localStorage.setItem(storageKey, theme)
    } catch {
      // Theme still works when storage is unavailable.
    }
  }, [storageKey, theme])

  const value = useMemo<ThemeContextValue>(
    () => ({setTheme, theme, toggleTheme}),
    [theme, toggleTheme],
  )

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>
}

export function useTheme(): ThemeContextValue {
  const context = use(ThemeContext)
  if (context === undefined) {
    throw new Error("useTheme must be used within ThemeProvider")
  }
  return context
}

function readInitialTheme(storageKey: string, defaultTheme: ThemePreference): ThemeMode {
  try {
    const storedTheme = globalThis.localStorage?.getItem(storageKey)
    if (storedTheme === "dark" || storedTheme === "light") return storedTheme
  } catch {
    // Fall through to the configured default when storage is unavailable.
  }

  if (defaultTheme !== "system") return defaultTheme

  return globalThis.matchMedia?.("(prefers-color-scheme: dark)").matches ? "dark" : "light"
}
