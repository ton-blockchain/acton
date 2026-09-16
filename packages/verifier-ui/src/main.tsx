import {createRoot} from "react-dom/client"
import {BrowserRouter} from "react-router"

import {App} from "./App"
import "./global.css"
import {clearLegacyVerifierStorage} from "./lib/legacy-storage"

clearLegacyVerifierStorage()

const root = document.getElementById("root")
if (!root) throw new Error("Verifier root element was not found")
createRoot(root).render(
  <BrowserRouter>
    <App />
  </BrowserRouter>,
)
