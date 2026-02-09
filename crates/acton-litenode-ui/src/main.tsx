import * as React from "react"
import {createRoot} from "react-dom/client"

import {App} from "./App"

const rootElement = document.querySelector("#root")
if (rootElement) {
  createRoot(rootElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  )
}
