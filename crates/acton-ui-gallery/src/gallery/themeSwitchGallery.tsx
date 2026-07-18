import {ThemeSwitch, type ThemeMode} from "@acton/ui"

import styles from "./themeSwitchGallery.module.css"
import type {ComponentGallery} from "./types"

const themeStates = [
  {
    theme: "light",
    title: "Light Active",
    description: "Sun segment active, same geometry as the shared-ui switch.",
  },
  {
    theme: "dark",
    title: "Dark Active",
    description: "Moon segment active with acton surface-active token.",
  },
] as const satisfies readonly {
  readonly description: string
  readonly theme: ThemeMode
  readonly title: string
}[]

function StateSamples() {
  return (
    <div className={styles.grid}>
      {themeStates.map(state => (
        <article key={state.theme} className={styles.sample}>
          <div className={styles.sampleText}>
            <h4>{state.title}</h4>
            <p>{state.description}</p>
          </div>
          <ThemeSwitch theme={state.theme} onToggleTheme={() => undefined} />
        </article>
      ))}
    </div>
  )
}

function ToolbarSample() {
  return (
    <div className={styles.toolbarPreview}>
      <span className={styles.toolbarText}>
        <strong>Toolbar placement</strong>
        <span>Use at the end of compact app chrome.</span>
      </span>
      <ThemeSwitch theme="dark" onToggleTheme={() => undefined} aria-label="Use light theme" />
    </div>
  )
}

export const themeSwitchGallery = {
  id: "theme-switch",
  title: "ThemeSwitch",
  status: "ready",
  summary:
    "ThemeSwitch toggles between light and dark modes with the same segmented pill appearance used across existing Acton UIs.",
  importStatement: 'import { ThemeSwitch } from "@acton/ui"',
  agentSummary:
    "Use ThemeSwitch inside the app-level ThemeProvider. It reads theme state from context; controlled props are reserved for visual-state previews.",
  usage: [
    "Use inside ThemeProvider in app chrome, sidebars, and settings surfaces where the whole interface theme changes.",
    "Keep the existing Sun/Moon segmented pill appearance.",
    "Set a contextual aria-label such as Use light theme or Use dark theme when possible.",
  ],
  avoid: [
    "Do not use for section-level display modes; use segmented controls for local modes.",
    "Do not wrap it in another button or toolbar control.",
    "Do not restyle the active segment with product-specific colors.",
  ],
  sections: [
    {
      id: "theme-switch-states",
      title: "States",
      description: "Light and dark active states with acton tokens.",
      content: <StateSamples />,
    },
    {
      id: "theme-switch-toolbar",
      title: "Toolbar Context",
      description: "ThemeSwitch placed where existing app chrome uses it.",
      content: <ToolbarSample />,
    },
  ],
} satisfies ComponentGallery
