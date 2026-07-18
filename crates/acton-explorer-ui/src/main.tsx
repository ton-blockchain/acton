import {StrictMode} from "react"
import {createRoot} from "react-dom/client"
import {ThemeProvider} from "@acton/ui"

import {ExplorerApp} from "./ExplorerApp"

const rootElement = document.querySelector("#root")
if (rootElement) {
  createRoot(rootElement).render(
    <StrictMode>
      <ThemeProvider storageKey="explorerTheme">
        <ExplorerApp />
      </ThemeProvider>
    </StrictMode>,
  )
}
