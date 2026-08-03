type ThemeMode = "dark" | "light"
type ThemePreference = "system" | ThemeMode

interface ThemeBootstrapOptions {
  readonly defaultTheme?: ThemePreference
  readonly storageKey?: string
}

const THEME_BOOTSTRAP_STYLES = `
html {
  min-height: 100%;
  background: hsl(240 7% 97%);
  color-scheme: light;
}

html[data-theme="dark"] {
  background: hsl(240 10% 4%);
  color-scheme: dark;
}

body,
#root {
  min-height: 100%;
}

body {
  margin: 0;
  background: inherit;
}

@media (prefers-color-scheme: dark) {
  html:not([data-theme]) {
    background: hsl(240 10% 4%);
    color-scheme: dark;
  }
}
`.trim()

export function themeBootstrap({
  defaultTheme = "system",
  storageKey = "theme",
}: ThemeBootstrapOptions = {}) {
  const fallbackTheme =
    defaultTheme === "system"
      ? 'matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light"'
      : JSON.stringify(defaultTheme)
  const script = `
"use strict"

;(() => {
  let theme
  try {
    const storedTheme = localStorage.getItem(${JSON.stringify(storageKey)})
    if (storedTheme === "dark" || storedTheme === "light") theme = storedTheme
  } catch {
    // The fallback below still prevents a theme flash when storage is unavailable.
  }

  theme ??= ${fallbackTheme}
  document.documentElement.dataset.theme = theme
  document.documentElement.classList.toggle("dark-theme", theme === "dark")
})()
`.trim()

  return {
    name: "acton-theme-bootstrap",
    enforce: "pre" as const,
    transformIndexHtml: {
      order: "pre" as const,
      handler: () => [
        {
          tag: "style",
          attrs: {"data-acton-theme-bootstrap": ""},
          children: THEME_BOOTSTRAP_STYLES,
          injectTo: "head-pre" as const,
        },
        {
          tag: "script",
          attrs: {"data-acton-theme-bootstrap": ""},
          children: script,
          injectTo: "head-pre" as const,
        },
      ],
    },
  }
}
