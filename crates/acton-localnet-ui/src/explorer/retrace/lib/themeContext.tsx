import React, {useState, useEffect, useCallback} from "react"

import {type Theme, ThemeContext} from "./useTheme"

export const ThemeProvider: React.FC<{children: React.ReactNode}> = ({children}) => {
  const [theme, setTheme] = useState<Theme>(() => {
    const storedTheme = localStorage.getItem("app-theme") as Theme | null
    if (storedTheme) {
      return storedTheme
    }
    const prefersDark =
      window.matchMedia && window.matchMedia("(prefers-color-scheme: dark)").matches
    if (prefersDark) {
      return "dark"
    }
    return "light"
  })

  useEffect(() => {
    if (theme === "dark") {
      document.body.classList.add("dark-theme")
    } else {
      document.body.classList.remove("dark-theme")
    }
    localStorage.setItem("app-theme", theme)
  }, [theme])

  const toggleTheme = useCallback(() => {
    if (document.startViewTransition) {
      document.startViewTransition(() => {
        setTheme(prevTheme => (prevTheme === "light" ? "dark" : "light"))
      })
    } else {
      setTheme(prevTheme => (prevTheme === "light" ? "dark" : "light"))
    }
  }, [])

  return <ThemeContext.Provider value={{theme, toggleTheme}}>{children}</ThemeContext.Provider>
}
