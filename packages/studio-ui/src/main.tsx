import {StrictMode} from "react"
import {createRoot} from "react-dom/client"
import {BrowserRouter} from "react-router"
import {ThemeProvider} from "@acton/ui"

import "@acton/ui/styles/tokens.css"
import {App} from "./App"
import "./index.css"

const rootElement = document.querySelector("#root")

if (!rootElement) {
  throw new Error("Failed to find the Studio root element")
}

createRoot(rootElement).render(
  <StrictMode>
    <ThemeProvider storageKey="acton-studio-theme">
      <BrowserRouter>
        <App />
      </BrowserRouter>
    </ThemeProvider>
  </StrictMode>,
)
