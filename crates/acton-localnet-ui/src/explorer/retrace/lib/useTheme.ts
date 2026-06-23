import {createContext, useContext} from "react"

export type Theme = "light" | "dark"

interface ThemeContextType {
  readonly theme: Theme
  readonly toggleTheme: () => void
}

export const ThemeContext = createContext<ThemeContextType | undefined>(undefined)

export const useTheme = () => {
  const context = useContext(ThemeContext)
  if (context === undefined) {
    throw new Error("useTheme must be used within a ThemeProvider")
  }
  return context
}
