import {StrictMode} from "react"
import {createRoot} from "react-dom/client"
import {ThemeProvider, ToastProvider} from "@acton/ui"

import "@acton/ui/styles/tokens.css"
import {App} from "./App"
import "./index.css"

const rootElement = document.querySelector("#root")

if (!rootElement) {
  throw new Error("Failed to find the Localton UI root element")
}

createRoot(rootElement).render(
  <StrictMode>
    <ThemeProvider storageKey="localton-observability-theme">
      <ToastProvider>
        <App />
      </ToastProvider>
    </ThemeProvider>
  </StrictMode>,
)
