import {StrictMode} from "react"
import {createRoot} from "react-dom/client"
import {ThemeProvider} from "@acton/ui"

import {App} from "./App"
import "./index.css"

const rootElement = document.querySelector("#root")
if (rootElement) {
  createRoot(rootElement).render(
    <StrictMode>
      <ThemeProvider defaultTheme="dark" storageKey="acton-dev-wallet:theme">
        <App />
      </ThemeProvider>
    </StrictMode>,
  )
}
